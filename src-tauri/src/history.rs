//! Historial de análisis: guarda lo que ocupaba cada carpeta para ver qué ha crecido desde la vez anterior.
//!
//! Por cada carpeta analizada hay una subcarpeta (con nombre derivado de su ruta) con dos ficheros:
//! - `analisis.json`: fecha, tamaño y número de ficheros de los últimos análisis.
//! - `carpetas.json`: tamaño de cada carpeta del último análisis (solo las grandes y poco profundas).

use crate::rules::{QUARANTINE_FOLDER_NAME, normalize};
use crate::tree::{DirectoryId, ScanTree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCANS_FILE_NAME: &str = "analisis.json";
const SNAPSHOT_FILE_NAME: &str = "carpetas.json";
const MAX_SCANS_PER_ROOT: usize = 50;
const SNAPSHOT_MAX_DEPTH: usize = 6;
const SNAPSHOT_MIN_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const GROWTH_MIN_BYTES: u64 = 50 * 1024 * 1024;
const MAX_GROWTH_ENTRIES: usize = 40;
/// Una carpeta no se lista si una de sus subcarpetas explica al menos esta parte de lo que ha crecido.
const EXPLAINED_GROWTH_FRACTION: f64 = 0.8;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PastScan {
  pub scanned_at: u64,
  pub total_size: u64,
  pub file_count: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RootScans {
  root_path: String,
  scans: Vec<PastScan>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
  scanned_at: u64,
  /// Ruta en minúsculas y tamaño.
  directories: HashMap<String, u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrowthEntry {
  pub name: String,
  pub path: String,
  pub directory_id: DirectoryId,
  pub previous_size: u64,
  pub current_size: u64,
  /// No existía (o era pequeña) en el análisis anterior.
  pub is_new: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanComparison {
  pub root_path: String,
  pub previous_scan: Option<PastScan>,
  pub grown: Vec<GrowthEntry>,
  /// Del más reciente al más antiguo, incluido el actual.
  pub scans: Vec<PastScan>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedRoot {
  pub root_path: String,
  pub last_scan: PastScan,
  pub scan_count: usize,
}

/// Compara el análisis con el anterior de la misma carpeta y lo guarda como el último.
pub fn record(history_dir: &Path, tree: &ScanTree) -> Result<ScanComparison, String> {
  let root = &tree.records[0];
  let root_dir = history_dir.join(root_folder_name(&root.path));
  fs::create_dir_all(&root_dir).map_err(|error| error.to_string())?;

  let mut root_scans: RootScans = read_json(&root_dir.join(SCANS_FILE_NAME)).unwrap_or_default();
  let previous_snapshot: Option<Snapshot> = read_json(&root_dir.join(SNAPSHOT_FILE_NAME));
  let current_scan = PastScan { scanned_at: unix_now(), total_size: root.size, file_count: root.file_count };
  let snapshot_ids = snapshot_directory_ids(tree);
  let grown = previous_snapshot.as_ref().map(|previous| growth(tree, &snapshot_ids, previous)).unwrap_or_default();

  root_scans.root_path = root.path.clone();
  let previous_scan = root_scans.scans.first().cloned();
  root_scans.scans.insert(0, current_scan.clone());
  root_scans.scans.truncate(MAX_SCANS_PER_ROOT);
  let directories = snapshot_ids.iter().map(|&id| (normalize(&tree.records[id as usize].path), tree.records[id as usize].size)).collect();
  write_json(&root_dir.join(SNAPSHOT_FILE_NAME), &Snapshot { scanned_at: current_scan.scanned_at, directories })?;
  write_json(&root_dir.join(SCANS_FILE_NAME), &root_scans)?;

  Ok(ScanComparison { root_path: root.path.clone(), previous_scan, grown, scans: root_scans.scans })
}

/// Carpetas analizadas alguna vez, de la más reciente a la más antigua.
pub fn list_roots(history_dir: &Path) -> Vec<ScannedRoot> {
  let Ok(entries) = fs::read_dir(history_dir) else { return Vec::new() };
  let mut roots: Vec<ScannedRoot> = entries
    .flatten()
    .filter_map(|entry| read_json::<RootScans>(&entry.path().join(SCANS_FILE_NAME)))
    .filter_map(|root_scans| {
      let last_scan = root_scans.scans.first()?.clone();
      Some(ScannedRoot { root_path: root_scans.root_path, last_scan, scan_count: root_scans.scans.len() })
    })
    .collect();
  roots.sort_by(|a, b| b.last_scan.scanned_at.cmp(&a.last_scan.scanned_at));
  roots
}

fn root_folder_name(root_path: &str) -> String {
  blake3::hash(normalize(root_path).as_bytes()).to_hex()[..16].to_string()
}

fn unix_now() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or_default()
}

/// Carpetas que se guardan: poco profundas, grandes y fuera de la cuarentena (lo apartado no es crecimiento).
fn snapshot_directory_ids(tree: &ScanTree) -> Vec<DirectoryId> {
  let quarantine_name = QUARANTINE_FOLDER_NAME.to_lowercase();
  let mut depths: Vec<usize> = vec![0; tree.records.len()];
  let mut excluded: Vec<bool> = vec![false; tree.records.len()];
  let mut ids = Vec::new();
  // Los padres siempre tienen un id menor que sus hijos.
  for (index, record) in tree.records.iter().enumerate() {
    if let Some(parent) = record.parent {
      depths[index] = depths[parent as usize] + 1;
      excluded[index] = excluded[parent as usize] || record.name.to_lowercase() == quarantine_name;
    }
    if !excluded[index] && depths[index] <= SNAPSHOT_MAX_DEPTH && record.size >= SNAPSHOT_MIN_SIZE_BYTES {
      ids.push(index as DirectoryId);
    }
  }
  ids
}

fn growth(tree: &ScanTree, snapshot_ids: &[DirectoryId], previous: &Snapshot) -> Vec<GrowthEntry> {
  let mut deltas: HashMap<DirectoryId, (u64, Option<u64>)> = HashMap::new();
  for &id in snapshot_ids {
    let record = &tree.records[id as usize];
    let previous_size = previous.directories.get(&normalize(&record.path)).copied();
    // Si no estaba ni ella ni su padre, no se sabe cuánto ocupaba: se descarta.
    let parent_known = record.parent.is_some_and(|parent| previous.directories.contains_key(&normalize(&tree.records[parent as usize].path)));
    if previous_size.is_none() && !parent_known {
      continue;
    }
    let delta = record.size.saturating_sub(previous_size.unwrap_or(0));
    if delta >= GROWTH_MIN_BYTES {
      deltas.insert(id, (delta, previous_size));
    }
  }
  let explained: Vec<DirectoryId> = deltas
    .iter()
    .filter_map(|(&id, &(delta, _))| {
      let parent = tree.records[id as usize].parent?;
      let (parent_delta, _) = deltas.get(&parent)?;
      (delta as f64 >= *parent_delta as f64 * EXPLAINED_GROWTH_FRACTION).then_some(parent)
    })
    .collect();
  for parent in explained {
    deltas.remove(&parent);
  }
  let mut grown: Vec<GrowthEntry> = deltas
    .into_iter()
    .map(|(id, (_, previous_size))| {
      let record = &tree.records[id as usize];
      GrowthEntry {
        name: record.name.clone(),
        path: record.path.clone(),
        directory_id: id,
        previous_size: previous_size.unwrap_or(0),
        current_size: record.size,
        is_new: previous_size.is_none(),
      }
    })
    .collect();
  grown.sort_by(|a, b| (b.current_size - b.previous_size).cmp(&(a.current_size - a.previous_size)));
  grown.truncate(MAX_GROWTH_ENTRIES);
  grown
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Option<T> {
  serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
  let content = serde_json::to_vec(value).map_err(|error| error.to_string())?;
  // Primero a un temporal: si se corta a medias, el fichero anterior sigue intacto.
  let temporary_path = path.with_extension("tmp");
  fs::write(&temporary_path, content).and_then(|_| fs::rename(&temporary_path, path)).map_err(|error| error.to_string())
}
