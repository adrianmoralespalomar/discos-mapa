//! Ficheros con el mismo contenido dentro del análisis abierto.
//!
//! 1. Se agrupan por tamaño (solo metadatos).
//! 2. Los que coinciden se comparan por una huella del principio y el final.
//! 3. Los que siguen coincidiendo se comparan por la huella BLAKE3 de todo el contenido.
//!
//! Es lo único de la app que lee el contenido de los ficheros. No entra en zonas protegidas ni
//! en las que tienen herramienta propia, salta lo que está solo en la nube y descarta los enlaces
//! duros (dos rutas al mismo fichero: apartar una no libera nada).

use crate::rules::{CleanupAction, RiskLevel, normalize};
use crate::scanner::CLOUD_ONLY_ATTRIBUTES;
use crate::tree::{DirectoryId, ScanTree, join_path};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const PARTIAL_HASH_BYTES: u64 = 64 * 1024;
const READ_BUFFER_BYTES: usize = 1024 * 1024;
const GROUPS_LIMIT: usize = 300;
/// Segundos entre 1601 (FILETIME) y 1970 (Unix).
const FILETIME_UNIX_OFFSET_SECONDS: u64 = 11_644_473_600;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
  pub path: String,
  pub directory_id: DirectoryId,
  pub modified: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
  pub size: u64,
  /// Lo que se libera dejando una sola copia.
  pub wasted_size: u64,
  pub files: Vec<DuplicateFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesResult {
  pub groups: Vec<DuplicateGroup>,
  pub wasted_size: u64,
  pub compared_files: u64,
  pub cancelled: bool,
}

pub struct DuplicateCounters {
  pub cancelled: AtomicBool,
  pub listed_files: AtomicU64,
  pub hashed_bytes: AtomicU64,
  pub bytes_to_hash: AtomicU64,
}

impl Default for DuplicateCounters {
  fn default() -> Self {
    Self { cancelled: AtomicBool::new(false), listed_files: AtomicU64::new(0), hashed_bytes: AtomicU64::new(0), bytes_to_hash: AtomicU64::new(0) }
  }
}

struct Candidate {
  file: DuplicateFile,
  size: u64,
}

/// Carpetas del análisis en las que se puede buscar (rutas e id): las tuyas, sin regla o de nivel "Revisar".
/// Se salta lo regenerable (node_modules, dist, cachés, temporales), que ya se limpia entero desde otras pestañas.
pub fn searchable_directories(tree: &ScanTree) -> Vec<(DirectoryId, String)> {
  tree
    .effective_rules()
    .iter()
    .enumerate()
    .filter(|(index, rule)| {
      let record = &tree.records[*index];
      let blocked = rule.is_some_and(|rule| rule.level != RiskLevel::Review || rule.action == CleanupAction::Tool);
      !blocked && record.size > 0 && !record.inaccessible && !is_program_data_path(&normalize(&record.path))
    })
    .map(|(index, _)| (index as DirectoryId, tree.records[index].path.clone()))
    .collect()
}

/// Datos internos de programas: `AppData` y carpetas ocultas que empiezan por punto (`.local`, `.gradle`…).
/// Ahí un fichero repetido suele ser parte de un programa y apartarlo lo rompe.
fn is_program_data_path(lower_path: &str) -> bool {
  lower_path.split('\\').any(|segment| segment == "appdata" || (segment.starts_with('.') && segment.len() > 1))
}

pub fn find(directories: Vec<(DirectoryId, String)>, min_size: u64, counters: &DuplicateCounters) -> DuplicatesResult {
  let candidates: Vec<Candidate> = directories
    .into_par_iter()
    .flat_map_iter(|(directory_id, path)| list_files(directory_id, &path, min_size, counters))
    .collect();
  let compared_files = candidates.len() as u64;

  let mut by_size: HashMap<u64, Vec<Candidate>> = HashMap::new();
  for candidate in candidates {
    by_size.entry(candidate.size).or_default().push(candidate);
  }
  let same_size: Vec<Vec<Candidate>> = by_size.into_values().filter(|group| group.len() > 1).collect();
  let bytes_to_hash: u64 = same_size.iter().flatten().map(|candidate| candidate.size).sum();
  counters.bytes_to_hash.store(bytes_to_hash, Ordering::Relaxed);

  let mut groups: Vec<DuplicateGroup> = same_size
    .into_par_iter()
    .flat_map_iter(|group| split_by_content(group, counters))
    .collect();
  groups.sort_unstable_by(|a, b| b.wasted_size.cmp(&a.wasted_size));
  let wasted_size = groups.iter().map(|group| group.wasted_size).sum();
  groups.truncate(GROUPS_LIMIT);
  DuplicatesResult { groups, wasted_size, compared_files, cancelled: counters.cancelled.load(Ordering::Relaxed) }
}

fn list_files(directory_id: DirectoryId, path: &str, min_size: u64, counters: &DuplicateCounters) -> Vec<Candidate> {
  if counters.cancelled.load(Ordering::Relaxed) {
    return Vec::new();
  }
  let Ok(entries) = fs::read_dir(path) else { return Vec::new() };
  let files: Vec<Candidate> = entries
    .flatten()
    .filter_map(|entry| {
      let metadata = entry.metadata().ok()?;
      let file_type = metadata.file_type();
      let skip = file_type.is_dir() || file_type.is_symlink() || metadata.len() < min_size || metadata.file_attributes() & CLOUD_ONLY_ATTRIBUTES != 0;
      if skip {
        return None;
      }
      let modified = (metadata.last_write_time() / 10_000_000).saturating_sub(FILETIME_UNIX_OFFSET_SECONDS);
      let file = DuplicateFile { path: join_path(path, &entry.file_name().to_string_lossy()), directory_id, modified };
      Some(Candidate { file, size: metadata.len() })
    })
    .collect();
  counters.listed_files.fetch_add(files.len() as u64, Ordering::Relaxed);
  files
}

/// Divide un grupo del mismo tamaño en grupos de contenido idéntico.
fn split_by_content(group: Vec<Candidate>, counters: &DuplicateCounters) -> Vec<DuplicateGroup> {
  let size = group[0].size;
  let mut by_partial: HashMap<[u8; 32], Vec<Candidate>> = HashMap::new();
  for candidate in group {
    if counters.cancelled.load(Ordering::Relaxed) {
      return Vec::new();
    }
    if let Some(hash) = partial_hash(&candidate.file.path, size) {
      by_partial.entry(hash).or_default().push(candidate);
    }
  }
  let mut result = Vec::new();
  for partial_group in by_partial.into_values().filter(|group| group.len() > 1) {
    // Huella completa e identidad del fichero en disco (para descartar enlaces duros).
    let mut by_full: HashMap<[u8; 32], Vec<(Candidate, Option<(u32, u64)>)>> = HashMap::new();
    for candidate in partial_group {
      if counters.cancelled.load(Ordering::Relaxed) {
        return Vec::new();
      }
      let already_complete = size <= PARTIAL_HASH_BYTES * 2;
      if let Some((hash, identity)) = full_hash(&candidate.file.path, already_complete, counters) {
        by_full.entry(hash).or_default().push((candidate, identity));
      }
    }
    for mut same_content in by_full.into_values() {
      same_content.sort_by_key(|(candidate, _)| candidate.file.modified);
      let mut seen_identities = std::collections::HashSet::new();
      same_content.retain(|(_, identity)| identity.is_none_or(|identity| seen_identities.insert(identity)));
      if same_content.len() > 1 {
        let files: Vec<DuplicateFile> = same_content.into_iter().map(|(candidate, _)| candidate.file).collect();
        result.push(DuplicateGroup { size, wasted_size: size * (files.len() as u64 - 1), files });
      }
    }
  }
  result
}

fn partial_hash(path: &str, size: u64) -> Option<[u8; 32]> {
  let mut file = File::open(path).ok()?;
  let mut hasher = blake3::Hasher::new();
  let mut buffer = vec![0; PARTIAL_HASH_BYTES as usize];
  let read = file.read(&mut buffer).ok()?;
  hasher.update(&buffer[..read]);
  if size > PARTIAL_HASH_BYTES * 2 {
    file.seek(SeekFrom::End(-(PARTIAL_HASH_BYTES as i64))).ok()?;
    let read = file.read(&mut buffer).ok()?;
    hasher.update(&buffer[..read]);
  }
  Some(*hasher.finalize().as_bytes())
}

/// Huella de todo el fichero y su identidad (volumen, índice). `already_complete` evita releer los pequeños.
fn full_hash(path: &str, already_complete: bool, counters: &DuplicateCounters) -> Option<([u8; 32], Option<(u32, u64)>)> {
  let mut file = File::open(path).ok()?;
  let identity = file_identity(&file);
  let mut hasher = blake3::Hasher::new();
  let mut buffer = vec![0; READ_BUFFER_BYTES];
  if already_complete {
    let read = file.read(&mut buffer).ok()?;
    hasher.update(&buffer[..read]);
    counters.hashed_bytes.fetch_add(read as u64, Ordering::Relaxed);
    return Some((*hasher.finalize().as_bytes(), identity));
  }
  loop {
    if counters.cancelled.load(Ordering::Relaxed) {
      return None;
    }
    let read = file.read(&mut buffer).ok()?;
    if read == 0 {
      break;
    }
    hasher.update(&buffer[..read]);
    counters.hashed_bytes.fetch_add(read as u64, Ordering::Relaxed);
  }
  Some((*hasher.finalize().as_bytes(), identity))
}

/// Algunos servidores SMB devuelven índice 0 para todo: entonces no se sabe y no se usa para descartar copias.
fn file_identity(file: &File) -> Option<(u32, u64)> {
  use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
  let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
  // SAFETY: el handle es válido mientras `file` esté abierto y la estructura es de solo salida.
  let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
  let index = (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
  (ok != 0 && index != 0).then_some((information.dwVolumeSerialNumber, index))
}
