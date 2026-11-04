pub mod file_types;
pub mod quarantine;
pub mod rules;
pub mod scanner;
pub mod tree;

use quarantine::{CleanupReport, CleanupRequest, PurgeReport, QuarantineBatch};
use scanner::ScanCounters;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use tauri::ipc::Channel;
use tree::{ScanSummary, ScanTree, SummaryInput, TreeResponse};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
const NO_SCAN_MESSAGE: &str = "No hay ningún análisis abierto";

#[derive(Default)]
struct AppState {
  tree: Mutex<Option<ScanTree>>,
  cancelled: Arc<AtomicBool>,
  scanning: AtomicBool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DriveInfo {
  mount_point: String,
  label: String,
  file_system: String,
  total_space: u64,
  available_space: u64,
  removable: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
  files: u64,
  directories: u64,
  bytes: u64,
  current_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupResult {
  report: CleanupReport,
  summary: ScanSummary,
}

#[tauri::command]
fn list_drives() -> Result<Vec<DriveInfo>, String> {
  let disks = sysinfo::Disks::new_with_refreshed_list().map_err(|error| error.to_string())?;
  let mut drives: Vec<DriveInfo> = disks
    .iter()
    .map(|disk| DriveInfo {
      mount_point: disk.mount_point().to_string_lossy().into_owned(),
      label: disk.name().to_string_lossy().into_owned(),
      file_system: disk.file_system().to_string_lossy().into_owned(),
      total_space: disk.total_space(),
      available_space: disk.available_space(),
      removable: disk.is_removable(),
    })
    .collect();
  drives.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
  Ok(drives)
}

/// Discos fijos del equipo, como `c:`. Son los únicos donde se permite limpiar.
fn fixed_drives() -> Vec<String> {
  sysinfo::Disks::new_with_refreshed_list()
    .map(|disks| {
      disks
        .iter()
        .filter(|disk| !disk.is_removable())
        .map(|disk| rules::normalize(&disk.mount_point().to_string_lossy()))
        .filter(|mount_point| mount_point.len() == 2)
        .collect()
    })
    .unwrap_or_default()
}

#[tauri::command]
async fn scan_path(path: String, on_progress: Channel<ScanProgress>, state: State<'_, AppState>) -> Result<ScanSummary, String> {
  let root_path = normalize_root_path(&path);
  if !Path::new(&root_path).is_dir() {
    return Err(format!("No existe la carpeta {root_path}"));
  }
  if state.scanning.swap(true, Ordering::SeqCst) {
    return Err("Ya hay un análisis en marcha".into());
  }
  state.cancelled.store(false, Ordering::SeqCst);
  // Libera el análisis anterior antes de empezar otro.
  *state.tree.lock().map_err(|error| error.to_string())? = None;

  let counters = Arc::new(ScanCounters::new(state.cancelled.clone()));
  let finished = Arc::new(AtomicBool::new(false));
  spawn_progress_reporter(counters.clone(), finished.clone(), on_progress);

  let started = Instant::now();
  let scan_counters = counters.clone();
  let result = tauri::async_runtime::spawn_blocking(move || {
    let output = scanner::scan(&root_path, &scan_counters);
    let summary_input = SummaryInput {
      duration_ms: started.elapsed().as_millis() as u64,
      cancelled: scan_counters.is_cancelled(),
      inaccessible_count: scan_counters.inaccessible.load(Ordering::Relaxed),
    };
    let tree = ScanTree::build(output, &root_path, summary_input);
    let summary = tree.summary();
    (tree, summary)
  })
  .await;
  finished.store(true, Ordering::SeqCst);
  state.scanning.store(false, Ordering::SeqCst);

  let (tree, summary) = result.map_err(|error| error.to_string())?;
  *state.tree.lock().map_err(|error| error.to_string())? = Some(tree);
  Ok(summary)
}

fn spawn_progress_reporter(counters: Arc<ScanCounters>, finished: Arc<AtomicBool>, on_progress: Channel<ScanProgress>) {
  std::thread::spawn(move || {
    while !finished.load(Ordering::SeqCst) {
      let current_path = counters.current_path.lock().map(|path| path.clone()).unwrap_or_default();
      let progress = ScanProgress {
        files: counters.files.load(Ordering::Relaxed),
        directories: counters.directories.load(Ordering::Relaxed),
        bytes: counters.bytes.load(Ordering::Relaxed),
        current_path,
      };
      if on_progress.send(progress).is_err() {
        return;
      }
      std::thread::sleep(PROGRESS_INTERVAL);
    }
  });
}

/// `C:` pasa a `C:\`; el resto de rutas, sin barra final.
fn normalize_root_path(path: &str) -> String {
  let trimmed = path.trim().trim_end_matches(['\\', '/']);
  if trimmed.len() == 2 && trimmed.ends_with(':') { format!("{trimmed}\\") } else { trimmed.replace('/', "\\") }
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
  state.cancelled.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn get_tree(node_id: u32, depth: u8, state: State<'_, AppState>) -> Result<TreeResponse, String> {
  let tree = state.tree.lock().map_err(|error| error.to_string())?;
  tree.as_ref().and_then(|tree| tree.view(node_id, depth)).ok_or_else(|| NO_SCAN_MESSAGE.to_string())
}

/// Aparta a la cuarentena lo pedido y devuelve el resumen ya descontado.
#[tauri::command]
async fn quarantine_items(requests: Vec<CleanupRequest>, state: State<'_, AppState>) -> Result<CleanupResult, String> {
  let mut tree_guard = state.tree.lock().map_err(|error| error.to_string())?;
  let tree = tree_guard.as_mut().ok_or(NO_SCAN_MESSAGE)?;
  let report = quarantine::quarantine(&requests, &tree.rules, &fixed_drives(), |path| tree.directory_size(path));
  let moved_paths: Vec<(String, u64)> = report.moved.iter().map(|item| (item.original_path.clone(), item.size)).collect();
  tree.remove_moved(&moved_paths);
  Ok(CleanupResult { report, summary: tree.summary() })
}

#[tauri::command]
fn list_quarantine() -> Vec<QuarantineBatch> {
  quarantine::list(&fixed_drives())
}

#[tauri::command]
async fn restore_quarantine_batch(batch_path: String) -> Result<CleanupReport, String> {
  quarantine::restore(&batch_path)
}

#[tauri::command]
async fn purge_quarantine_batch(batch_path: String) -> Result<PurgeReport, String> {
  quarantine::purge(&batch_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_opener::init())
    .manage(AppState::default())
    .invoke_handler(tauri::generate_handler![
      list_drives,
      scan_path,
      cancel_scan,
      get_tree,
      quarantine_items,
      list_quarantine,
      restore_quarantine_batch,
      purge_quarantine_batch
    ])
    .run(tauri::generate_context!())
    .expect("error al arrancar discos-mapa");
}
