//! Recorrido paralelo del disco. Solo lee metadatos (nunca el contenido de los ficheros).
//!
//! - No entra en enlaces simbólicos ni uniones (junctions): evita bucles y contar dos veces.
//! - Los ficheros de OneDrive "solo en la nube" cuentan 0 bytes en disco.
//! - Las carpetas sin permiso se marcan como inaccesibles en lugar de fallar.
//! - Los enlaces duros (frecuentes en `WinSxS`) se cuentan una vez por enlace.

use crate::file_types::lower_extension;
use crate::rules::markers;
use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Ficheros que se guardan por carpeta para dibujarlos; el resto se agrupa.
const FILES_KEPT_PER_DIRECTORY: usize = 40;
pub const TOP_FILES_LIMIT: usize = 200;
const SCAN_THREAD_STACK_BYTES: usize = 32 * 1024 * 1024;

const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
pub const CLOUD_ONLY_ATTRIBUTES: u32 = FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
/// Segundos entre 1601 (FILETIME) y 1970 (Unix).
const FILETIME_UNIX_OFFSET_SECONDS: u64 = 11_644_473_600;

pub struct ScannedFile {
  pub name: String,
  pub size: u64,
  pub modified: u64,
  pub cloud_only: bool,
}

pub struct ScannedDirectory {
  pub name: String,
  pub size: u64,
  pub file_count: u64,
  pub newest_modified: u64,
  /// Fecha más reciente de los ficheros que están directamente en esta carpeta.
  pub own_files_modified: u64,
  pub inaccessible: bool,
  pub markers: u16,
  pub files: Vec<ScannedFile>,
  pub other_files_size: u64,
  pub other_files_count: u64,
  pub subdirectories: Vec<ScannedDirectory>,
}

impl ScannedDirectory {
  fn inaccessible(name: String) -> Self {
    Self {
      name,
      size: 0,
      file_count: 0,
      newest_modified: 0,
      own_files_modified: 0,
      inaccessible: true,
      markers: 0,
      files: Vec::new(),
      other_files_size: 0,
      other_files_count: 0,
      subdirectories: Vec::new(),
    }
  }
}

pub struct TopFile {
  pub size: u64,
  pub parent_path: String,
  pub name: String,
  pub modified: u64,
}

#[derive(Default, Clone, Copy)]
pub struct ExtensionTotals {
  pub size: u64,
  pub count: u64,
}

pub struct ScanCounters {
  pub cancelled: Arc<AtomicBool>,
  pub files: AtomicU64,
  pub directories: AtomicU64,
  pub bytes: AtomicU64,
  pub inaccessible: AtomicU64,
  pub current_path: Mutex<String>,
}

impl ScanCounters {
  pub fn new(cancelled: Arc<AtomicBool>) -> Self {
    Self {
      cancelled,
      files: AtomicU64::new(0),
      directories: AtomicU64::new(0),
      bytes: AtomicU64::new(0),
      inaccessible: AtomicU64::new(0),
      current_path: Mutex::new(String::new()),
    }
  }

  pub fn is_cancelled(&self) -> bool {
    self.cancelled.load(Ordering::Relaxed)
  }
}

struct ScanCollectors<'a> {
  counters: &'a ScanCounters,
  top_files: Mutex<BinaryHeap<Reverse<(u64, String, String, u64)>>>,
  top_files_threshold: AtomicU64,
  extensions: Mutex<HashMap<String, ExtensionTotals>>,
}

pub struct ScanOutput {
  pub root: ScannedDirectory,
  pub top_files: Vec<TopFile>,
  pub extensions: HashMap<String, ExtensionTotals>,
}

pub fn scan(root_path: &str, counters: &ScanCounters) -> ScanOutput {
  let collectors = ScanCollectors {
    counters,
    top_files: Mutex::new(BinaryHeap::with_capacity(TOP_FILES_LIMIT + 1)),
    top_files_threshold: AtomicU64::new(0),
    extensions: Mutex::new(HashMap::new()),
  };
  let thread_count = std::thread::available_parallelism().map_or(8, |count| count.get() * 2);
  let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(thread_count)
    .stack_size(SCAN_THREAD_STACK_BYTES)
    .build()
    .expect("no se pudo crear el grupo de hilos del escáner");
  let root = pool.install(|| scan_directory(Path::new(root_path), root_path.to_string(), &collectors));
  let mut top_files: Vec<TopFile> = collectors
    .top_files
    .into_inner()
    .unwrap_or_default()
    .into_iter()
    .map(|Reverse((size, parent_path, name, modified))| TopFile { size, parent_path, name, modified })
    .collect();
  top_files.sort_by(|a, b| b.size.cmp(&a.size));
  ScanOutput { root, top_files, extensions: collectors.extensions.into_inner().unwrap_or_default() }
}

fn scan_directory(path: &Path, name: String, collectors: &ScanCollectors) -> ScannedDirectory {
  let counters = collectors.counters;
  if counters.is_cancelled() {
    return ScannedDirectory::inaccessible(name);
  }
  let Ok(entries) = fs::read_dir(path) else {
    counters.inaccessible.fetch_add(1, Ordering::Relaxed);
    return ScannedDirectory::inaccessible(name);
  };
  counters.directories.fetch_add(1, Ordering::Relaxed);
  if let Ok(mut current_path) = counters.current_path.try_lock() {
    *current_path = path.to_string_lossy().into_owned();
  }

  let path_text = path.to_string_lossy();
  let mut subdirectory_entries = Vec::new();
  let mut files = Vec::new();
  let mut directory_markers = 0;
  let mut local_extensions: HashMap<String, ExtensionTotals> = HashMap::new();

  for entry in entries.flatten() {
    // En Windows estos metadatos vienen del propio listado: no se abre cada fichero.
    let Ok(metadata) = entry.metadata() else { continue };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
      continue;
    }
    let entry_name = entry.file_name().to_string_lossy().into_owned();
    if file_type.is_dir() {
      subdirectory_entries.push((entry.path(), entry_name));
      continue;
    }
    let cloud_only = metadata.file_attributes() & CLOUD_ONLY_ATTRIBUTES != 0;
    let size = if cloud_only { 0 } else { metadata.len() };
    let modified = (metadata.last_write_time() / 10_000_000).saturating_sub(FILETIME_UNIX_OFFSET_SECONDS);
    directory_markers |= markers::from_file_name(&entry_name.to_lowercase());
    let extension_totals = local_extensions.entry(lower_extension(&entry_name)).or_default();
    extension_totals.size += size;
    extension_totals.count += 1;
    offer_top_file(collectors, size, &path_text, &entry_name, modified);
    files.push(ScannedFile { name: entry_name, size, modified, cloud_only });
  }

  let local_file_count = files.len() as u64;
  let local_files_size: u64 = files.iter().map(|file| file.size).sum();
  counters.files.fetch_add(local_file_count, Ordering::Relaxed);
  counters.bytes.fetch_add(local_files_size, Ordering::Relaxed);
  merge_extensions(collectors, local_extensions);

  let subdirectories: Vec<ScannedDirectory> = subdirectory_entries
    .into_par_iter()
    .map(|(subdirectory_path, subdirectory_name)| scan_directory(&subdirectory_path, subdirectory_name, collectors))
    .collect();

  let newest_file_modified = files.iter().map(|file| file.modified).max().unwrap_or(0);
  files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
  let other_files: Vec<ScannedFile> = if files.len() > FILES_KEPT_PER_DIRECTORY { files.split_off(FILES_KEPT_PER_DIRECTORY) } else { Vec::new() };

  ScannedDirectory {
    name,
    size: local_files_size + subdirectories.iter().map(|directory| directory.size).sum::<u64>(),
    file_count: local_file_count + subdirectories.iter().map(|directory| directory.file_count).sum::<u64>(),
    newest_modified: subdirectories.iter().map(|directory| directory.newest_modified).fold(newest_file_modified, u64::max),
    own_files_modified: newest_file_modified,
    inaccessible: false,
    markers: directory_markers,
    other_files_size: other_files.iter().map(|file| file.size).sum(),
    other_files_count: other_files.len() as u64,
    files,
    subdirectories,
  }
}

fn offer_top_file(collectors: &ScanCollectors, size: u64, parent_path: &str, name: &str, modified: u64) {
  if size <= collectors.top_files_threshold.load(Ordering::Relaxed) {
    return;
  }
  let Ok(mut top_files) = collectors.top_files.lock() else { return };
  top_files.push(Reverse((size, parent_path.to_string(), name.to_string(), modified)));
  if top_files.len() > TOP_FILES_LIMIT {
    top_files.pop();
    if let Some(Reverse((smallest_size, ..))) = top_files.peek() {
      collectors.top_files_threshold.store(*smallest_size, Ordering::Relaxed);
    }
  }
}

fn merge_extensions(collectors: &ScanCollectors, local_extensions: HashMap<String, ExtensionTotals>) {
  if local_extensions.is_empty() {
    return;
  }
  let Ok(mut extensions) = collectors.extensions.lock() else { return };
  for (extension, totals) in local_extensions {
    let global_totals = extensions.entry(extension).or_default();
    global_totals.size += totals.size;
    global_totals.count += totals.count;
  }
}
