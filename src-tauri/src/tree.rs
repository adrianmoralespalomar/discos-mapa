//! Árbol del escaneo guardado en memoria y las vistas reducidas que se envían a la interfaz.
//!
//! Nunca se manda el árbol entero: la interfaz pide un nodo y una profundidad, y los
//! elementos pequeños se agrupan para que el mapa no tenga cientos de miles de piezas.

use crate::file_types::{FileCategory, category_for_extension};
use crate::rules::{RiskLevel, RiskRule, RuleSet, normalize};
use crate::scanner::{ExtensionTotals, ScanOutput, ScannedDirectory, ScannedFile, TopFile};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Fracción mínima del nodo mostrado para dibujar un elemento suelto.
const MINIMUM_VISIBLE_FRACTION: u64 = 3000;
const MAX_CHILDREN_PER_NODE: usize = 120;
const MAX_VIEW_DEPTH: u8 = 4;
const CANDIDATES_LIMIT: usize = 300;
const EXTENSIONS_LIMIT: usize = 40;

pub type DirectoryId = u32;

pub struct DirectoryRecord {
  pub name: String,
  pub path: String,
  pub parent: Option<DirectoryId>,
  pub size: u64,
  pub file_count: u64,
  pub newest_modified: u64,
  pub inaccessible: bool,
  pub files: Vec<ScannedFile>,
  pub other_files_size: u64,
  pub other_files_count: u64,
  pub children: Vec<DirectoryId>,
  pub risk: Option<RiskRule>,
}

pub struct ScanTree {
  pub records: Vec<DirectoryRecord>,
  pub root_inherited_risk: Option<RiskRule>,
  pub rules: RuleSet,
  top_files: Vec<TopFile>,
  extensions: HashMap<String, ExtensionTotals>,
  summary_input: SummaryInput,
  /// Rutas (en minúsculas) apartadas a la cuarentena después del escaneo.
  removed_paths: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RiskView {
  #[serde(flatten)]
  pub rule: RiskRule,
  pub inherited: bool,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum TreeNodeKind {
  Directory,
  File,
  Group,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNodeView {
  pub id: Option<DirectoryId>,
  pub kind: TreeNodeKind,
  pub name: String,
  pub path: String,
  pub size: u64,
  pub file_count: u64,
  pub modified: u64,
  pub inaccessible: bool,
  pub cloud_only: bool,
  pub has_hidden_children: bool,
  pub category: Option<FileCategory>,
  pub risk: Option<RiskView>,
  pub children: Vec<TreeNodeView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AncestorView {
  pub id: DirectoryId,
  pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeResponse {
  pub node: TreeNodeView,
  /// Desde la raíz del escaneo hasta el nodo pedido, ambos incluidos.
  pub ancestors: Vec<AncestorView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
  pub name: String,
  pub path: String,
  pub directory_id: Option<DirectoryId>,
  pub size: u64,
  pub modified: u64,
  pub category: FileCategory,
  pub risk: Option<RiskView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCandidate {
  pub directory_id: Option<DirectoryId>,
  pub name: String,
  pub path: String,
  pub size: u64,
  pub file_count: u64,
  pub modified: u64,
  pub risk: RiskView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
  pub extension: String,
  pub category: FileCategory,
  pub size: u64,
  pub count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
  pub category: FileCategory,
  pub size: u64,
  pub count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
  pub root_id: DirectoryId,
  pub root_path: String,
  pub total_size: u64,
  pub file_count: u64,
  pub directory_count: u64,
  pub inaccessible_count: u64,
  pub duration_ms: u64,
  pub cancelled: bool,
  pub top_files: Vec<FileView>,
  pub extensions: Vec<ExtensionStat>,
  pub categories: Vec<CategoryStat>,
  pub candidates: Vec<CleanupCandidate>,
  pub recoverable_safe: u64,
  pub recoverable_probable: u64,
  pub review_size: u64,
}

pub struct SummaryInput {
  pub duration_ms: u64,
  pub cancelled: bool,
  pub inaccessible_count: u64,
}

impl ScanTree {
  pub fn build(output: ScanOutput, root_path: &str, summary_input: SummaryInput) -> Self {
    let root = output.root;
    let rules = RuleSet::load();
    let lower_root_path = normalize(root_path);
    let root_inherited_risk = rules.inherited_rule_for(&lower_root_path);
    let mut records: Vec<DirectoryRecord> = Vec::new();
    // Pila explícita: (carpeta, padre, ruta, marcas del padre, nivel heredado).
    let mut pending: Vec<(ScannedDirectory, Option<DirectoryId>, String, u16, Option<RiskLevel>)> =
      vec![(root, None, root_path.to_string(), 0, root_inherited_risk.map(|rule| rule.level))];

    while let Some((mut directory, parent, path, parent_markers, inherited_level)) = pending.pop() {
      let lower_path = normalize(&path);
      let lower_name = directory.name.to_lowercase();
      let risk = if parent.is_none() {
        rules.path_rule(&lower_path)
      } else {
        rules.classify_directory(&lower_path, &lower_name, parent_markers, inherited_level)
      };
      let effective_level = risk.map(|rule| rule.level).or(inherited_level);
      let id = records.len() as DirectoryId;
      for subdirectory in directory.subdirectories.drain(..) {
        let subdirectory_path = join_path(&path, &subdirectory.name);
        pending.push((subdirectory, Some(id), subdirectory_path, directory.markers, effective_level));
      }
      if let Some(parent_id) = parent {
        records[parent_id as usize].children.push(id);
      }
      records.push(DirectoryRecord {
        name: directory.name,
        path,
        parent,
        size: directory.size,
        file_count: directory.file_count,
        newest_modified: directory.newest_modified,
        inaccessible: directory.inaccessible,
        files: directory.files,
        other_files_size: directory.other_files_size,
        other_files_count: directory.other_files_count,
        children: Vec::new(),
        risk,
      });
    }

    let sizes: Vec<u64> = records.iter().map(|record| record.size).collect();
    for record in &mut records {
      record.children.sort_unstable_by(|a, b| sizes[*b as usize].cmp(&sizes[*a as usize]));
    }
    Self {
      records,
      root_inherited_risk,
      rules,
      top_files: output.top_files,
      extensions: output.extensions,
      summary_input,
      removed_paths: Vec::new(),
    }
  }

  /// Regla propia o heredada del antepasado más cercano.
  fn effective_risk(&self, id: DirectoryId) -> Option<RiskView> {
    let mut current = Some(id);
    let mut inherited = false;
    while let Some(current_id) = current {
      let record = &self.records[current_id as usize];
      if let Some(rule) = record.risk {
        return Some(RiskView { rule, inherited });
      }
      inherited = true;
      current = record.parent;
    }
    self.root_inherited_risk.map(|rule| RiskView { rule, inherited: true })
  }

  fn file_risk(&self, directory_id: Option<DirectoryId>, parent_path: &str, file_name: &str) -> Option<RiskView> {
    if let Some(rule) = self.rules.classify_file(&normalize(parent_path), &file_name.to_lowercase()) {
      return Some(RiskView { rule, inherited: false });
    }
    directory_id.and_then(|id| self.effective_risk(id)).map(|risk| RiskView { inherited: true, ..risk })
  }

  pub fn view(&self, id: DirectoryId, depth: u8) -> Option<TreeResponse> {
    self.records.get(id as usize)?;
    let minimum_visible_size = self.records[id as usize].size / MINIMUM_VISIBLE_FRACTION;
    let mut node = self.directory_view(id, depth.clamp(1, MAX_VIEW_DEPTH), minimum_visible_size);
    node.risk = self.effective_risk(id);
    let mut ancestors = Vec::new();
    let mut current = Some(id);
    while let Some(current_id) = current {
      let record = &self.records[current_id as usize];
      ancestors.push(AncestorView { id: current_id, name: record.name.clone() });
      current = record.parent;
    }
    ancestors.reverse();
    Some(TreeResponse { node, ancestors })
  }

  fn directory_view(&self, id: DirectoryId, depth: u8, minimum_visible_size: u64) -> TreeNodeView {
    let record = &self.records[id as usize];
    let mut view = TreeNodeView {
      id: Some(id),
      kind: TreeNodeKind::Directory,
      name: record.name.clone(),
      path: record.path.clone(),
      size: record.size,
      file_count: record.file_count,
      modified: record.newest_modified,
      inaccessible: record.inaccessible,
      cloud_only: false,
      has_hidden_children: false,
      category: None,
      risk: record.risk.map(|rule| RiskView { rule, inherited: false }),
      children: Vec::new(),
    };
    let has_content = !record.children.is_empty() || !record.files.is_empty() || record.other_files_count > 0;
    if depth == 0 || !has_content {
      view.has_hidden_children = has_content;
      return view;
    }

    enum Item<'a> {
      Directory(DirectoryId),
      File(&'a ScannedFile),
    }
    let mut items: Vec<(u64, Item)> = record
      .children
      .iter()
      .map(|child| (self.records[*child as usize].size, Item::Directory(*child)))
      .chain(record.files.iter().map(|file| (file.size, Item::File(file))))
      .collect();
    items.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut grouped_size = record.other_files_size;
    let mut grouped_count = record.other_files_count;
    for (index, (size, item)) in items.into_iter().enumerate() {
      if size == 0 {
        continue;
      }
      let visible = index < MAX_CHILDREN_PER_NODE && size >= minimum_visible_size;
      match item {
        Item::Directory(child) if visible => view.children.push(self.directory_view(child, depth - 1, minimum_visible_size)),
        Item::Directory(child) => {
          grouped_size += size;
          grouped_count += self.records[child as usize].file_count;
        }
        Item::File(file) if visible => view.children.push(self.file_view(record, file)),
        Item::File(_) => {
          grouped_size += size;
          grouped_count += 1;
        }
      }
    }
    if grouped_size > 0 {
      view.children.push(TreeNodeView {
        id: None,
        kind: TreeNodeKind::Group,
        name: format!("{grouped_count} elementos pequeños"),
        path: record.path.clone(),
        size: grouped_size,
        file_count: grouped_count,
        modified: 0,
        inaccessible: false,
        cloud_only: false,
        has_hidden_children: false,
        category: None,
        risk: None,
        children: Vec::new(),
      });
    }
    view
  }

  /// Los ficheros sin regla propia heredan en la interfaz la de su carpeta.
  fn file_view(&self, record: &DirectoryRecord, file: &ScannedFile) -> TreeNodeView {
    let own_risk = self.rules.classify_file(&normalize(&record.path), &file.name.to_lowercase());
    TreeNodeView {
      id: None,
      kind: TreeNodeKind::File,
      name: file.name.clone(),
      path: join_path(&record.path, &file.name),
      size: file.size,
      file_count: 1,
      modified: file.modified,
      inaccessible: false,
      cloud_only: file.cloud_only,
      has_hidden_children: false,
      category: Some(category_for_extension(&crate::file_types::lower_extension(&file.name))),
      risk: own_risk.map(|rule| RiskView { rule, inherited: false }),
      children: Vec::new(),
    }
  }

  /// Quita del árbol lo que se ha apartado a la cuarentena y descuenta su tamaño de las carpetas superiores.
  /// Los totales por extensión no se recalculan: siguen siendo los del escaneo.
  pub fn remove_moved(&mut self, moved_paths: &[(String, u64)]) {
    let parent_paths: HashSet<String> = moved_paths
      .iter()
      .filter_map(|(path, _)| std::path::Path::new(path).parent().map(|parent| normalize(&parent.to_string_lossy())))
      .collect();
    let directory_ids: HashMap<String, DirectoryId> = self
      .records
      .iter()
      .enumerate()
      .filter_map(|(index, record)| {
        let lower_path = normalize(&record.path);
        parent_paths.contains(&lower_path).then_some((lower_path, index as DirectoryId))
      })
      .collect();

    for (path, size_hint) in moved_paths {
      let moved_path = std::path::Path::new(path);
      let lower_name = moved_path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
      let parent_lower_path = moved_path.parent().map(|parent| normalize(&parent.to_string_lossy())).unwrap_or_default();
      self.removed_paths.push(normalize(path));
      let Some(&parent_id) = directory_ids.get(&parent_lower_path) else { continue };
      let (removed_size, removed_files) = self.detach_entry(parent_id, &lower_name, *size_hint);
      let mut current = Some(parent_id);
      while let Some(current_id) = current {
        let record = &mut self.records[current_id as usize];
        record.size = record.size.saturating_sub(removed_size);
        record.file_count = record.file_count.saturating_sub(removed_files);
        current = record.parent;
      }
    }
  }

  /// Saca una carpeta o un fichero de su carpeta padre y devuelve (bytes, ficheros) quitados.
  fn detach_entry(&mut self, parent_id: DirectoryId, lower_name: &str, size_hint: u64) -> (u64, u64) {
    let child_position = self.records[parent_id as usize].children.iter().position(|child| self.records[*child as usize].name.to_lowercase() == lower_name);
    if let Some(position) = child_position {
      let child_id = self.records[parent_id as usize].children.remove(position);
      let child = &mut self.records[child_id as usize];
      let removed = (child.size, child.file_count);
      child.size = 0;
      child.file_count = 0;
      child.children.clear();
      child.files.clear();
      child.other_files_size = 0;
      child.other_files_count = 0;
      return removed;
    }
    let parent = &mut self.records[parent_id as usize];
    if let Some(position) = parent.files.iter().position(|file| file.name.to_lowercase() == lower_name) {
      return (parent.files.remove(position).size, 1);
    }
    // Fichero pequeño que se había agrupado en "otros".
    let removed_size = size_hint.min(parent.other_files_size);
    parent.other_files_size -= removed_size;
    parent.other_files_count = parent.other_files_count.saturating_sub(1);
    (removed_size, 1)
  }

  pub fn directory_size(&self, path: &str) -> Option<u64> {
    let lower_path = normalize(path);
    self.records.iter().find(|record| normalize(&record.path) == lower_path).map(|record| record.size)
  }

  pub fn summary(&self) -> ScanSummary {
    let input = &self.summary_input;
    let root = &self.records[0];
    let (candidates, recoverable_safe, recoverable_probable, review_size) = self.cleanup_candidates();
    ScanSummary {
      root_id: 0,
      root_path: root.path.clone(),
      total_size: root.size,
      file_count: root.file_count,
      directory_count: self.records.len() as u64,
      inaccessible_count: input.inaccessible_count,
      duration_ms: input.duration_ms,
      cancelled: input.cancelled,
      top_files: self.top_file_views(),
      categories: category_stats(&self.extensions),
      extensions: extension_stats(&self.extensions),
      candidates,
      recoverable_safe,
      recoverable_probable,
      review_size,
    }
  }

  fn top_file_views(&self) -> Vec<FileView> {
    let top_files: Vec<&TopFile> = self
      .top_files
      .iter()
      .filter(|file| {
        let lower_path = normalize(&join_path(&file.parent_path, &file.name));
        !self.removed_paths.iter().any(|removed| lower_path == *removed || lower_path.starts_with(&format!("{removed}\\")))
      })
      .collect();
    let parent_paths: HashSet<&str> = top_files.iter().map(|file| file.parent_path.as_str()).collect();
    let directory_ids: HashMap<&str, DirectoryId> = self
      .records
      .iter()
      .enumerate()
      .filter(|(_, record)| parent_paths.contains(record.path.as_str()))
      .map(|(index, record)| (record.path.as_str(), index as DirectoryId))
      .collect();
    top_files
      .iter()
      .map(|file| {
        let directory_id = directory_ids.get(file.parent_path.as_str()).copied();
        FileView {
          name: file.name.clone(),
          path: join_path(&file.parent_path, &file.name),
          directory_id,
          size: file.size,
          modified: file.modified,
          category: category_for_extension(&crate::file_types::lower_extension(&file.name)),
          risk: self.file_risk(directory_id, &file.parent_path, &file.name),
        }
      })
      .collect()
  }

  /// Carpetas más altas con regla de limpieza. No se baja dentro de una carpeta ya sugerida,
  /// así que ningún byte se cuenta dos veces.
  fn cleanup_candidates(&self) -> (Vec<CleanupCandidate>, u64, u64, u64) {
    let mut candidates = Vec::new();
    let (mut safe, mut probable, mut review) = (0, 0, 0);
    let mut pending: Vec<DirectoryId> = vec![0];
    while let Some(id) = pending.pop() {
      let record = &self.records[id as usize];
      if record.size == 0 {
        continue;
      }
      let rule = record.risk.filter(|rule| matches!(rule.level, RiskLevel::Safe | RiskLevel::Probable) || rule.suggest);
      if let Some(rule) = rule {
        match rule.level {
          RiskLevel::Safe => safe += record.size,
          RiskLevel::Probable => probable += record.size,
          _ => review += record.size,
        }
        candidates.push(CleanupCandidate {
          directory_id: Some(id),
          name: record.name.clone(),
          path: record.path.clone(),
          size: record.size,
          file_count: record.file_count,
          modified: record.newest_modified,
          risk: RiskView { rule, inherited: false },
        });
        continue;
      }
      for file in &record.files {
        let Some(rule) = self.rules.classify_file(&normalize(&record.path), &file.name.to_lowercase()) else { continue };
        if !(matches!(rule.level, RiskLevel::Safe | RiskLevel::Probable) || rule.suggest) || file.size == 0 {
          continue;
        }
        match rule.level {
          RiskLevel::Safe => safe += file.size,
          RiskLevel::Probable => probable += file.size,
          _ => review += file.size,
        }
        candidates.push(CleanupCandidate {
          directory_id: None,
          name: file.name.clone(),
          path: join_path(&record.path, &file.name),
          size: file.size,
          file_count: 1,
          modified: file.modified,
          risk: RiskView { rule, inherited: false },
        });
      }
      pending.extend(record.children.iter().copied());
    }
    candidates.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    candidates.truncate(CANDIDATES_LIMIT);
    (candidates, safe, probable, review)
  }
}

fn extension_stats(extensions: &HashMap<String, ExtensionTotals>) -> Vec<ExtensionStat> {
  let mut stats: Vec<ExtensionStat> = extensions
    .iter()
    .map(|(extension, totals)| ExtensionStat { category: category_for_extension(extension), extension: extension.clone(), size: totals.size, count: totals.count })
    .collect();
  stats.sort_unstable_by(|a, b| b.size.cmp(&a.size));
  stats.truncate(EXTENSIONS_LIMIT);
  stats
}

fn category_stats(extensions: &HashMap<String, ExtensionTotals>) -> Vec<CategoryStat> {
  let mut totals: HashMap<FileCategory, (u64, u64)> = HashMap::new();
  for (extension, extension_totals) in extensions {
    let entry = totals.entry(category_for_extension(extension)).or_default();
    entry.0 += extension_totals.size;
    entry.1 += extension_totals.count;
  }
  let mut stats: Vec<CategoryStat> = totals.into_iter().map(|(category, (size, count))| CategoryStat { category, size, count }).collect();
  stats.sort_unstable_by(|a, b| b.size.cmp(&a.size));
  stats
}

pub fn join_path(parent: &str, name: &str) -> String {
  if parent.ends_with('\\') || parent.ends_with('/') { format!("{parent}{name}") } else { format!("{parent}\\{name}") }
}
