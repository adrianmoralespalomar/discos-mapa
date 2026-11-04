//! Proyectos de desarrollo encontrados en el análisis, con su última actividad real y lo que se puede regenerar.
//!
//! La actividad no cuenta las carpetas regenerables (un `npm install` cambia `node_modules` aunque no toques el código).

use crate::rules::{CleanupAction, RiskLevel, markers};
use crate::tree::{DirectoryId, ScanTree};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerableFolder {
  pub directory_id: DirectoryId,
  pub name: String,
  pub path: String,
  pub size: u64,
  pub category: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
  pub directory_id: DirectoryId,
  pub name: String,
  pub path: String,
  pub kind: &'static str,
  /// Segundos Unix del último cambio fuera de las carpetas regenerables.
  pub last_activity: u64,
  pub regenerable_size: u64,
  pub folders: Vec<RegenerableFolder>,
}

pub fn find_projects(tree: &ScanTree) -> Vec<ProjectView> {
  let effective_levels = tree.effective_levels();
  let mut projects = Vec::new();
  let mut pending: Vec<DirectoryId> = vec![0];
  while let Some(id) = pending.pop() {
    let record = &tree.records[id as usize];
    if effective_levels[id as usize] == Some(RiskLevel::Protected) {
      continue;
    }
    let (regenerable, other): (Vec<DirectoryId>, Vec<DirectoryId>) = record.children.iter().partition(|child| is_regenerable(tree, **child));
    if record.markers != 0 {
      let folders: Vec<RegenerableFolder> = regenerable
        .iter()
        .map(|child_id| (*child_id, &tree.records[*child_id as usize]))
        .filter(|(_, child)| child.size > 0)
        .map(|(child_id, child)| RegenerableFolder {
          directory_id: child_id,
          name: child.name.clone(),
          path: child.path.clone(),
          size: child.size,
          category: child.risk.map_or("", |rule| rule.category),
        })
        .collect();
      if !folders.is_empty() {
        let last_activity = other.iter().map(|child| tree.records[*child as usize].newest_modified).fold(record.own_files_modified, u64::max);
        projects.push(ProjectView {
          directory_id: id,
          name: record.name.clone(),
          path: record.path.clone(),
          kind: project_kind(record.markers),
          last_activity,
          regenerable_size: folders.iter().map(|folder| folder.size).sum(),
          folders,
        });
      }
    }
    // Dentro de node_modules, target… hay miles de package.json que no son proyectos tuyos.
    pending.extend(other);
  }
  projects.sort_unstable_by(|a, b| b.regenerable_size.cmp(&a.regenerable_size));
  projects
}

fn is_regenerable(tree: &ScanTree, id: DirectoryId) -> bool {
  tree.records[id as usize]
    .risk
    .is_some_and(|rule| rule.action == CleanupAction::Item && matches!(rule.level, RiskLevel::Safe | RiskLevel::Probable))
}

fn project_kind(project_markers: u16) -> &'static str {
  if project_markers & markers::ANGULAR_JSON != 0 {
    "Angular"
  } else if project_markers & markers::PACKAGE_JSON != 0 {
    "Node"
  } else if project_markers & markers::CARGO_TOML != 0 {
    "Rust"
  } else if project_markers & markers::DOTNET_PROJECT != 0 {
    ".NET"
  } else if project_markers & markers::GRADLE_PROJECT != 0 {
    "Gradle"
  } else if project_markers & markers::MAVEN_PROJECT != 0 {
    "Maven"
  } else {
    "Python"
  }
}
