pub mod duplicates;
pub mod file_types;
pub mod history;
pub mod projects;
pub mod quarantine;
pub mod rules;
pub mod scanner;
pub mod tree;

use duplicates::{DuplicateCounters, DuplicatesResult};
use history::{ScanComparison, ScannedRoot};
use projects::ProjectView;
use quarantine::{CleanupReport, CleanupRequest, PurgeReport, QuarantineBatch};
use scanner::ScanCounters;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use tauri::ipc::Channel;
use tree::{ScanSummary, ScanTree, SummaryInput, TreeResponse};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
const NO_SCAN_MESSAGE: &str = "No hay ningún análisis abierto";
const LOCAL_FILE_SYSTEMS: &[&str] = &["NTFS", "REFS"];
const HISTORY_FOLDER_NAME: &str = "historial";
/// Valor de `GetDriveTypeW` para un disco fijo (los de red son `DRIVE_REMOTE`).
const DRIVE_FIXED: u32 = 3;

#[derive(Default)]
struct AppState {
  tree: Mutex<Option<ScanTree>>,
  cancelled: Arc<AtomicBool>,
  scanning: AtomicBool,
  duplicate_counters: Mutex<Option<Arc<DuplicateCounters>>>,
  comparison: Mutex<Option<ScanComparison>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DuplicateProgress {
  listed_files: u64,
  hashed_bytes: u64,
  bytes_to_hash: u64,
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

/// Discos fijos NTFS o ReFS del equipo, como `c:`. Son los únicos donde se permite limpiar.
/// Las unidades de Google Drive y similares se montan con otros sistemas de ficheros (como FAT32), y las
/// unidades de red mapeadas pueden decir que son NTFS (Samba lo hace), por eso se mira también el tipo.
fn fixed_drives() -> Vec<String> {
  sysinfo::Disks::new_with_refreshed_list()
    .map(|disks| {
      disks
        .iter()
        .filter(|disk| !disk.is_removable())
        .filter(|disk| LOCAL_FILE_SYSTEMS.contains(&disk.file_system().to_string_lossy().to_uppercase().as_str()))
        .map(|disk| rules::normalize(&disk.mount_point().to_string_lossy()))
        .filter(|mount_point| mount_point.len() == 2 && is_fixed_drive(mount_point))
        .collect()
    })
    .unwrap_or_default()
}

fn is_fixed_drive(mount_point: &str) -> bool {
  let wide_root: Vec<u16> = format!("{mount_point}\\").encode_utf16().chain(std::iter::once(0)).collect();
  // SAFETY: la cadena termina en 0 y vive durante la llamada.
  unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(wide_root.as_ptr()) == DRIVE_FIXED }
}

#[tauri::command]
async fn scan_path(path: String, on_progress: Channel<ScanProgress>, app: AppHandle, state: State<'_, AppState>) -> Result<ScanSummary, String> {
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
  *state.comparison.lock().map_err(|error| error.to_string())? = None;
  let can_clean = quarantine::drive_of(&rules::normalize(&root_path)).is_some_and(|drive| fixed_drives().contains(&drive));
  let history_dir = history_dir(&app);

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
      can_clean,
    };
    let tree = ScanTree::build(output, &root_path, summary_input);
    let summary = tree.summary();
    // Un análisis cortado no vale para comparar. Si el historial falla, el análisis sigue valiendo.
    let comparison = match (&history_dir, summary.cancelled) {
      (Some(history_dir), false) => history::record(history_dir, &tree).inspect_err(|error| eprintln!("historial: {error}")).ok(),
      _ => None,
    };
    (tree, summary, comparison)
  })
  .await;
  finished.store(true, Ordering::SeqCst);
  state.scanning.store(false, Ordering::SeqCst);

  let (tree, summary, comparison) = result.map_err(|error| error.to_string())?;
  *state.tree.lock().map_err(|error| error.to_string())? = Some(tree);
  *state.comparison.lock().map_err(|error| error.to_string())? = comparison;
  Ok(summary)
}

fn history_dir(app: &AppHandle) -> Option<PathBuf> {
  app.path().app_data_dir().ok().map(|directory| directory.join(HISTORY_FOLDER_NAME))
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
  let directory_sizes = tree.directory_sizes();
  let report = quarantine::quarantine(&requests, &tree.rules, &fixed_drives(), |path| directory_sizes.get(&rules::normalize(path)).copied());
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

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectView>, String> {
  let tree = state.tree.lock().map_err(|error| error.to_string())?;
  Ok(projects::find_projects(tree.as_ref().ok_or(NO_SCAN_MESSAGE)?))
}

#[tauri::command]
async fn find_duplicates(min_size: u64, on_progress: Channel<DuplicateProgress>, state: State<'_, AppState>) -> Result<DuplicatesResult, String> {
  let directories = {
    let tree = state.tree.lock().map_err(|error| error.to_string())?;
    duplicates::searchable_directories(tree.as_ref().ok_or(NO_SCAN_MESSAGE)?)
  };
  let counters = Arc::new(DuplicateCounters::default());
  *state.duplicate_counters.lock().map_err(|error| error.to_string())? = Some(counters.clone());

  let finished = Arc::new(AtomicBool::new(false));
  let progress_counters = counters.clone();
  let progress_finished = finished.clone();
  std::thread::spawn(move || {
    while !progress_finished.load(Ordering::SeqCst) {
      let progress = DuplicateProgress {
        listed_files: progress_counters.listed_files.load(Ordering::Relaxed),
        hashed_bytes: progress_counters.hashed_bytes.load(Ordering::Relaxed),
        bytes_to_hash: progress_counters.bytes_to_hash.load(Ordering::Relaxed),
      };
      if on_progress.send(progress).is_err() {
        return;
      }
      std::thread::sleep(PROGRESS_INTERVAL);
    }
  });

  let search_counters = counters.clone();
  let result = tauri::async_runtime::spawn_blocking(move || duplicates::find(directories, min_size, &search_counters)).await;
  finished.store(true, Ordering::SeqCst);
  result.map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_duplicates(state: State<'_, AppState>) {
  if let Ok(Some(counters)) = state.duplicate_counters.lock().as_deref() {
    counters.cancelled.store(true, Ordering::SeqCst);
  }
}

/// Qué ha crecido desde el análisis anterior de la misma carpeta.
#[tauri::command]
fn get_scan_comparison(state: State<'_, AppState>) -> Result<Option<ScanComparison>, String> {
  Ok(state.comparison.lock().map_err(|error| error.to_string())?.clone())

}

#[tauri::command]
fn list_scanned_roots(app: AppHandle) -> Vec<ScannedRoot> {
  history_dir(&app).map(|directory| history::list_roots(&directory)).unwrap_or_default()
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
      purge_quarantine_batch,
      list_projects,
      find_duplicates,
      cancel_duplicates,
      get_scan_comparison,
      list_scanned_roots
    ])
    .run(tauri::generate_context!())
    .expect("error al arrancar discos-mapa");
}
