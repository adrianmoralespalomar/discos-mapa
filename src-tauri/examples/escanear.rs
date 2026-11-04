//! Escanea una carpeta desde la terminal y muestra el resumen, sin abrir la ventana.
//!
//! cargo run --release --example escanear -- "C:\Users"

use discos_mapa_lib::scanner::{self, ScanCounters};
use discos_mapa_lib::tree::{ScanTree, SummaryInput};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn main() {
  let root_path = std::env::args().nth(1).unwrap_or_else(|| "C:\\".to_string());
  let counters = ScanCounters::new(Arc::new(AtomicBool::new(false)));
  let started = Instant::now();
  let output = scanner::scan(&root_path, &counters);
  let scan_seconds = started.elapsed().as_secs_f64();
  let summary_input = SummaryInput {
    duration_ms: started.elapsed().as_millis() as u64,
    cancelled: false,
    inaccessible_count: counters.inaccessible.load(Ordering::Relaxed),
  };
  let tree = ScanTree::build(output, &root_path, summary_input);
  let summary = tree.summary();
  let summary_json = serde_json::to_string(&summary).unwrap_or_default();
  let view_json = tree.view(0, 3).map(|view| serde_json::to_string(&view).unwrap_or_default()).unwrap_or_default();

  println!("Carpeta: {root_path}");
  println!("Escaneo: {scan_seconds:.1} s, total {:.1} s", started.elapsed().as_secs_f64());
  println!("Tamaño: {:.2} GB en {} ficheros y {} carpetas ({} sin permiso)", gigabytes(summary.total_size), summary.file_count, summary.directory_count, summary.inaccessible_count);
  println!("Recuperable: seguro {:.2} GB, probable {:.2} GB, a revisar {:.2} GB", gigabytes(summary.recoverable_safe), gigabytes(summary.recoverable_probable), gigabytes(summary.review_size));
  println!("JSON del resumen: {} KB, vista del mapa: {} KB", summary_json.len() / 1024, view_json.len() / 1024);
  println!("\nSugerencias principales:");
  for candidate in summary.candidates.iter().take(15) {
    println!("  {:>8.2} GB  {:?}  {}", gigabytes(candidate.size), candidate.risk.rule.level, candidate.path);
  }

  let projects = discos_mapa_lib::projects::find_projects(&tree);
  println!("\nProyectos con carpetas regenerables: {}", projects.len());
  for project in projects.iter().take(10) {
    let folders: Vec<&str> = project.folders.iter().map(|folder| folder.name.as_str()).collect();
    let days = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().saturating_sub(project.last_activity)) / 86_400;
    println!("  {:>8.2} GB  {:<7} hace {:>4} días  {}  {:?}", gigabytes(project.regenerable_size), project.kind, days, project.path, folders);
  }

  let started_duplicates = Instant::now();
  let counters = discos_mapa_lib::duplicates::DuplicateCounters::default();
  let duplicates = discos_mapa_lib::duplicates::find(discos_mapa_lib::duplicates::searchable_directories(&tree), 1024 * 1024, &counters);
  println!(
    "\nDuplicados (>1 MB): {} grupos, {:.2} GB repetidos, {} ficheros comparados, {:.1} s",
    duplicates.groups.len(),
    gigabytes(duplicates.wasted_size),
    duplicates.compared_files,
    started_duplicates.elapsed().as_secs_f64()
  );
  for group in duplicates.groups.iter().take(5) {
    println!("  {:>8.2} MB x{}  {}", group.size as f64 / 1024f64.powi(2), group.files.len(), group.files[0].path);
  }
}

fn gigabytes(bytes: u64) -> f64 {
  bytes as f64 / 1024f64.powi(3)
}
