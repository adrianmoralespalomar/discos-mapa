//! Cuarentena: lo que se limpia se aparta a `X:\DiscosMapa-Cuarentena\<lote>` en el mismo disco.
//!
//! - Mover dentro del mismo disco es un renombrado: instantáneo y siempre reversible.
//! - No hay límite de tamaño (la Papelera de Windows borra para siempre lo que no le cabe).
//! - Solo se borra de verdad cuando el usuario vacía un lote.
//! - Se rechaza todo lo que esté en una zona protegida, en discos extraíbles o de red, y la raíz de un disco.

use crate::rules::{CleanupAction, QUARANTINE_FOLDER_NAME, RiskLevel, RuleSet, normalize};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST_FILE_NAME: &str = "manifiesto.json";
const ITEMS_FOLDER_NAME: &str = "elementos";
/// Cada cuántos elementos movidos se guarda el manifiesto (por si se cierra la app a mitad).
const MANIFEST_SAVE_INTERVAL: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRequest {
  pub path: String,
  pub action: CleanupAction,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinedItem {
  pub original_path: String,
  pub stored_path: String,
  pub size: u64,
  pub is_directory: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Manifest {
  created: u64,
  items: Vec<QuarantinedItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineBatch {
  pub path: String,
  pub created: u64,
  pub total_size: u64,
  pub items: Vec<QuarantinedItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupFailure {
  pub path: String,
  pub reason: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
  pub moved: Vec<QuarantinedItem>,
  pub failures: Vec<CleanupFailure>,
  pub moved_size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
  pub freed_size: u64,
  pub error: Option<String>,
}

/// Lote abierto en un disco durante una limpieza.
struct OpenBatch {
  path: PathBuf,
  manifest: Manifest,
}

impl OpenBatch {
  fn save(&self) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(&self.manifest).map_err(std::io::Error::other)?;
    fs::write(self.path.join(MANIFEST_FILE_NAME), json)
  }
}

/// `size_of` da el tamaño de una carpeta según el último escaneo (los ficheros se miden en el momento).
pub fn quarantine(requests: &[CleanupRequest], rules: &RuleSet, allowed_drives: &[String], size_of: impl Fn(&str) -> Option<u64>) -> CleanupReport {
  let mut report = CleanupReport::default();
  let mut open_batches: Vec<OpenBatch> = Vec::new();

  for request in requests {
    let path = PathBuf::from(request.path.trim_end_matches(['\\', '/']));
    if let Err(reason) = validate(&path, rules, allowed_drives) {
      report.failures.push(CleanupFailure { path: request.path.clone(), reason });
      continue;
    }
    // Carpetas que Windows o un programa necesitan (como Temp) nunca se apartan enteras.
    let own_action = rules.path_rule(&normalize(&request.path)).map(|rule| rule.action);
    let action = if own_action == Some(CleanupAction::Contents) { CleanupAction::Contents } else { request.action };
    match action {
      CleanupAction::Item => move_entry(&path, &mut open_batches, &mut report, &size_of),
      CleanupAction::Contents => {
        let Ok(entries) = fs::read_dir(&path) else {
          report.failures.push(CleanupFailure { path: request.path.clone(), reason: "No se puede leer la carpeta".into() });
          continue;
        };
        for entry in entries.flatten() {
          let child = entry.path();
          match validate(&child, rules, allowed_drives) {
            Ok(()) => move_entry(&child, &mut open_batches, &mut report, &size_of),
            Err(reason) => report.failures.push(CleanupFailure { path: child.to_string_lossy().into_owned(), reason }),
          }
        }
      }
      CleanupAction::Manual => report.failures.push(CleanupFailure {
        path: request.path.clone(),
        reason: "Se limpia con la herramienta que se indica, no desde la app".into(),
      }),
    }
  }

  for batch in &open_batches {
    if batch.manifest.items.is_empty() {
      let _ = fs::remove_dir_all(&batch.path);
    } else if let Err(error) = batch.save() {
      report.failures.push(CleanupFailure { path: batch.path.to_string_lossy().into_owned(), reason: format!("No se pudo guardar el manifiesto: {error}") });
    }
  }
  report.moved_size = report.moved.iter().map(|item| item.size).sum();
  report
}

fn validate(path: &Path, rules: &RuleSet, allowed_drives: &[String]) -> Result<(), String> {
  let lower_path = normalize(&path.to_string_lossy());
  let drive = drive_of(&lower_path).ok_or("Solo se pueden limpiar rutas de un disco local (C:, D:…)")?;
  if lower_path.len() <= 2 {
    return Err("No se puede apartar un disco entero".into());
  }
  if !allowed_drives.contains(&drive) {
    return Err("Solo se limpia en discos fijos del equipo, no en extraíbles ni de red".into());
  }
  if lower_path.starts_with(&normalize(&quarantine_root(&drive).to_string_lossy())) {
    return Err("Ya está en la cuarentena".into());
  }
  let metadata = fs::symlink_metadata(path).map_err(|_| "Ya no existe".to_string())?;
  let rule = rules.effective_rule_for_path(&lower_path, !metadata.is_dir());
  if let Some(rule) = rule.filter(|rule| rule.level == RiskLevel::Protected) {
    return Err(format!("Zona protegida ({}): {}", rule.category, rule.reason));
  }
  Ok(())
}

fn move_entry(path: &Path, open_batches: &mut Vec<OpenBatch>, report: &mut CleanupReport, size_of: &impl Fn(&str) -> Option<u64>) {
  let original_path = path.to_string_lossy().into_owned();
  let Some(drive) = drive_of(&normalize(&original_path)) else { return };
  let batch_index = match open_batches.iter().position(|batch| batch.path.starts_with(quarantine_root(&drive))) {
    Some(index) => index,
    None => match open_batch(&drive) {
      Ok(batch) => {
        open_batches.push(batch);
        open_batches.len() - 1
      }
      Err(error) => {
        report.failures.push(CleanupFailure { path: original_path, reason: format!("No se pudo crear la cuarentena en {drive}: {error}") });
        return;
      }
    },
  };
  let batch = &mut open_batches[batch_index];
  let Ok(metadata) = fs::symlink_metadata(path) else {
    report.failures.push(CleanupFailure { path: original_path, reason: "Ya no existe".into() });
    return;
  };
  let is_directory = metadata.is_dir();
  let size = if is_directory { size_of(&original_path).unwrap_or(0) } else { metadata.len() };
  let slot = batch.path.join(ITEMS_FOLDER_NAME).join(batch.manifest.items.len().to_string());
  let stored_path = slot.join(path.file_name().unwrap_or_default());
  if let Err(error) = fs::create_dir_all(&slot) {
    report.failures.push(CleanupFailure { path: original_path, reason: format!("No se pudo preparar la cuarentena: {error}") });
    return;
  }
  if let Err(error) = fs::rename(path, &stored_path) {
    let _ = fs::remove_dir(&slot);
    report.failures.push(CleanupFailure { path: original_path, reason: describe_move_error(&error) });
    return;
  }
  let item = QuarantinedItem { original_path, stored_path: stored_path.to_string_lossy().into_owned(), size, is_directory };
  batch.manifest.items.push(item.clone());
  report.moved.push(item);
  if batch.manifest.items.len() % MANIFEST_SAVE_INTERVAL == 0 {
    let _ = batch.save();
  }
}

fn describe_move_error(error: &std::io::Error) -> String {
  match error.raw_os_error() {
    Some(5) => "Sin permiso (puede que haga falta abrir la app como administrador)".into(),
    Some(32) | Some(33) => "Está en uso por otro programa".into(),
    _ => format!("No se pudo mover: {error}"),
  }
}

fn open_batch(drive: &str) -> std::io::Result<OpenBatch> {
  let created = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0);
  let root = quarantine_root(drive);
  let mut path = root.join(created.to_string());
  let mut suffix = 1;
  while path.exists() {
    path = root.join(format!("{created}-{suffix}"));
    suffix += 1;
  }
  fs::create_dir_all(path.join(ITEMS_FOLDER_NAME))?;
  let batch = OpenBatch { path, manifest: Manifest { created, items: Vec::new() } };
  batch.save()?;
  Ok(batch)
}

pub fn list(drives: &[String]) -> Vec<QuarantineBatch> {
  let mut batches: Vec<QuarantineBatch> = drives
    .iter()
    .filter_map(|drive| fs::read_dir(quarantine_root(drive)).ok())
    .flat_map(|entries| entries.flatten())
    .filter_map(|entry| {
      let manifest = read_manifest(&entry.path())?;
      Some(QuarantineBatch {
        path: entry.path().to_string_lossy().into_owned(),
        created: manifest.created,
        total_size: manifest.items.iter().map(|item| item.size).sum(),
        items: manifest.items,
      })
    })
    .collect();
  batches.sort_by(|a, b| b.created.cmp(&a.created));
  batches
}

pub fn restore(batch_path: &str) -> Result<CleanupReport, String> {
  let batch_path = validated_batch_path(batch_path)?;
  let manifest = read_manifest(&batch_path).ok_or("El lote no tiene manifiesto")?;
  let mut report = CleanupReport::default();
  let mut remaining = Vec::new();
  for item in manifest.items {
    let original = PathBuf::from(&item.original_path);
    if original.exists() {
      report.failures.push(CleanupFailure { path: item.original_path.clone(), reason: "Ya hay algo en esa ruta; no se sobrescribe".into() });
      remaining.push(item);
      continue;
    }
    let restored = original.parent().map_or(Ok(()), fs::create_dir_all).and_then(|_| fs::rename(&item.stored_path, &original));
    match restored {
      Ok(()) => report.moved.push(item),
      Err(error) => {
        report.failures.push(CleanupFailure { path: item.original_path.clone(), reason: describe_move_error(&error) });
        remaining.push(item);
      }
    }
  }
  report.moved_size = report.moved.iter().map(|item| item.size).sum();
  if remaining.is_empty() {
    // Solo quedan carpetas vacías y el manifiesto.
    let _ = fs::remove_dir_all(&batch_path);
    remove_root_if_empty(&batch_path);
  } else {
    let batch = OpenBatch { path: batch_path, manifest: Manifest { created: manifest.created, items: remaining } };
    batch.save().map_err(|error| error.to_string())?;
  }
  Ok(report)
}

/// Borra el lote para siempre. Es lo único de la app que borra de verdad.
pub fn purge(batch_path: &str) -> Result<PurgeReport, String> {
  let batch_path = validated_batch_path(batch_path)?;
  let freed_size = read_manifest(&batch_path).map_or(0, |manifest| manifest.items.iter().map(|item| item.size).sum());
  let error = fs::remove_dir_all(&batch_path).err().map(|error| format!("No se pudo borrar todo: {error}"));
  remove_root_if_empty(&batch_path);
  Ok(PurgeReport { freed_size, error })
}

/// Quita `X:\DiscosMapa-Cuarentena` cuando ya no le queda ningún lote (`remove_dir` falla si no está vacía).
fn remove_root_if_empty(batch_path: &Path) {
  if let Some(root) = batch_path.parent() {
    let _ = fs::remove_dir(root);
  }
}

/// Solo se aceptan carpetas que estén directamente dentro de una cuarentena y tengan manifiesto.
fn validated_batch_path(batch_path: &str) -> Result<PathBuf, String> {
  let path = PathBuf::from(batch_path);
  let parent_name = path.parent().and_then(Path::file_name).map(|name| name.to_string_lossy().to_lowercase());
  if parent_name.as_deref() != Some(&QUARANTINE_FOLDER_NAME.to_lowercase()) || !path.join(MANIFEST_FILE_NAME).is_file() {
    return Err("Esa carpeta no es un lote de la cuarentena".into());
  }
  Ok(path)
}

fn read_manifest(batch_path: &Path) -> Option<Manifest> {
  serde_json::from_str(&fs::read_to_string(batch_path.join(MANIFEST_FILE_NAME)).ok()?).ok()
}

fn quarantine_root(drive: &str) -> PathBuf {
  PathBuf::from(format!("{}\\{QUARANTINE_FOLDER_NAME}", drive.to_uppercase()))
}

/// `c:` para cualquier ruta `C:\...`; nada para rutas de red.
fn drive_of(lower_path: &str) -> Option<String> {
  let bytes = lower_path.as_bytes();
  (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()).then(|| lower_path[..2].to_string())
}
