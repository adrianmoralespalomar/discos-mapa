//! Reglas que deciden si una carpeta o un fichero se puede borrar.
//!
//! Principios:
//! - Cada regla explica el porqué y, si existe, la forma oficial de limpiarlo.
//! - Una carpeta hereda el nivel de su antepasado más cercano con regla propia.
//! - Dentro de una zona protegida solo cuentan las reglas explícitas de ruta,
//!   nunca las genéricas por nombre (para no sugerir nada dentro de `Windows`).
//! - Ante la duda, el nivel más prudente.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
  /// Se regenera solo o es basura.
  Safe,
  /// Se puede regenerar, pero con un coste (descargas, compilaciones).
  Probable,
  /// Depende del usuario.
  Review,
  /// No tocar a mano.
  Protected,
}

/// Qué hace el botón de limpiar sobre lo que marca la regla.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupAction {
  /// Se aparta la carpeta o el fichero entero.
  Item,
  /// Se aparta lo de dentro y la carpeta se queda (Windows o el programa la necesitan).
  Contents,
  /// Lo decide el usuario mirando uno a uno (Descargas, AppData): no se marca en bloque,
  /// pero sí se puede apartar un fichero o una carpeta concretos.
  Manual,
  /// Solo con la herramienta oficial que indica el consejo (Docker, emuladores, Windows.old…):
  /// la app no aparta nada de dentro, ni siquiera ficheros sueltos.
  Tool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskRule {
  pub level: RiskLevel,
  pub category: &'static str,
  pub reason: &'static str,
  pub tip: Option<&'static str>,
  pub action: CleanupAction,
  /// Aparece en la lista de sugerencias aunque sea de nivel `Review`.
  #[serde(skip)]
  pub suggest: bool,
}

impl RiskRule {
  const fn item(self) -> Self {
    Self { action: CleanupAction::Item, ..self }
  }

  const fn contents(self) -> Self {
    Self { action: CleanupAction::Contents, ..self }
  }

  const fn tool(self) -> Self {
    Self { action: CleanupAction::Tool, ..self }
  }
}

const fn rule(level: RiskLevel, category: &'static str, reason: &'static str, tip: Option<&'static str>) -> RiskRule {
  RiskRule { level, category, reason, tip, action: CleanupAction::Manual, suggest: false }
}

const fn suggested(level: RiskLevel, category: &'static str, reason: &'static str, tip: Option<&'static str>) -> RiskRule {
  RiskRule { level, category, reason, tip, action: CleanupAction::Manual, suggest: true }
}

/// Ficheros de marca que indican qué tipo de proyecto hay en una carpeta.
pub mod markers {
  pub const PACKAGE_JSON: u16 = 1 << 0;
  pub const ANGULAR_JSON: u16 = 1 << 1;
  pub const CARGO_TOML: u16 = 1 << 2;
  pub const DOTNET_PROJECT: u16 = 1 << 3;
  pub const GRADLE_PROJECT: u16 = 1 << 4;
  pub const PYTHON_PROJECT: u16 = 1 << 5;
  pub const MAVEN_PROJECT: u16 = 1 << 6;

  pub fn from_file_name(lower_name: &str) -> u16 {
    match lower_name {
      "package.json" => PACKAGE_JSON,
      "angular.json" => ANGULAR_JSON,
      "cargo.toml" => CARGO_TOML,
      "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => GRADLE_PROJECT,
      "pyproject.toml" | "requirements.txt" | "setup.py" => PYTHON_PROJECT,
      "pom.xml" => MAVEN_PROJECT,
      name if name.ends_with(".csproj") || name.ends_with(".sln") || name.ends_with(".fsproj") || name.ends_with(".vbproj") => DOTNET_PROJECT,
      _ => 0,
    }
  }
}

/// Carpeta, en la raíz de cada disco, donde se aparta lo que se limpia.
pub const QUARANTINE_FOLDER_NAME: &str = "DiscosMapa-Cuarentena";
const QUARANTINE_FOLDER_LOWER_NAME: &str = "discosmapa-cuarentena";

const TIP_WINDOWS_TEMP_FILES: &str = "Configuración > Sistema > Almacenamiento > Archivos temporales.";
const REASON_BUILD_OUTPUT: &str = "Salida de compilación de un proyecto; se vuelve a generar al compilar.";

use RiskLevel::{Probable, Protected, Review, Safe};

/// Reglas por ruta absoluta. `{VARIABLE}` se sustituye por la variable de entorno.
const PATH_RULES: &[(&str, RiskRule)] = &[
  // Sistema
  ("{SystemRoot}", rule(Protected, "Sistema", "Carpeta de Windows. Borrar algo aquí a mano puede dejar el equipo sin arrancar.", Some(TIP_WINDOWS_TEMP_FILES))),
  ("{SystemRoot}\\WinSxS", rule(Protected, "Sistema", "Almacén de componentes de Windows. Muchos ficheros son enlaces a otros del sistema, así que ocupa menos de lo que parece.", Some("Limpia versiones antiguas con: DISM /Online /Cleanup-Image /StartComponentCleanup (como administrador)."))),
  ("{SystemRoot}\\Installer", rule(Protected, "Sistema", "Windows lo necesita para reparar y desinstalar programas.", None)),
  ("{ProgramFiles}", rule(Protected, "Programas", "Programas instalados.", Some("Desinstala lo que no uses desde Configuración > Aplicaciones."))),
  ("{ProgramFiles(x86)}", rule(Protected, "Programas", "Programas instalados (32 bits).", Some("Desinstala lo que no uses desde Configuración > Aplicaciones."))),
  ("{ProgramData}", rule(Protected, "Datos de programas", "Datos compartidos de los programas instalados.", None)),
  ("{APPDATA}", rule(Review, "Configuración de programas", "Configuración de tus programas. Solo borra carpetas de programas que ya hayas desinstalado.", None)),
  ("{LOCALAPPDATA}", rule(Review, "Datos de programas", "Datos locales de tus programas. Solo borra carpetas de programas que ya hayas desinstalado.", None)),
  ("{USERPROFILE}\\Documents", rule(Review, "Personal", "Tus documentos. Revísalos uno a uno.", None)),
  ("{USERPROFILE}\\Desktop", rule(Review, "Personal", "Tu escritorio. Revísalo uno a uno.", None)),
  ("{USERPROFILE}\\Pictures", rule(Review, "Personal", "Tus imágenes. Revísalas una a una.", None)),
  ("{USERPROFILE}\\Videos", rule(Review, "Personal", "Tus vídeos. Revísalos uno a uno.", None)),
  ("{USERPROFILE}\\Music", rule(Review, "Personal", "Tu música. Revísala una a una.", None)),
  ("{USERPROFILE}\\Downloads", suggested(Review, "Descargas", "Descargas acumuladas: suelen sobrar instaladores y ficheros ya usados.", None)),
  // Temporales y cachés del sistema
  ("{TEMP}", rule(Safe, "Temporales", "Ficheros temporales de tus programas. Los que estén en uso no se dejarán borrar.", Some(TIP_WINDOWS_TEMP_FILES)).contents()),
  ("{SystemRoot}\\Temp", rule(Safe, "Temporales", "Ficheros temporales de Windows.", Some(TIP_WINDOWS_TEMP_FILES)).contents()),
  ("{SystemRoot}\\SoftwareDistribution\\Download", rule(Safe, "Actualizaciones de Windows", "Descargas de Windows Update ya instaladas.", Some("Archivos temporales > Limpieza de Windows Update.")).contents()),
  ("{SystemRoot}\\ServiceProfiles\\NetworkService\\AppData\\Local\\Microsoft\\Windows\\DeliveryOptimization", rule(Safe, "Actualizaciones de Windows", "Caché de Optimización de distribución (actualizaciones compartidas).", Some(TIP_WINDOWS_TEMP_FILES)).tool()),
  ("{SystemRoot}\\Logs\\CBS", rule(Safe, "Registros", "Registros antiguos del servicio de componentes de Windows.", None).contents()),
  ("{SystemRoot}\\LiveKernelReports", rule(Safe, "Volcados de errores", "Informes de bloqueos del sistema.", None).contents()),
  ("{SystemRoot}\\Minidump", rule(Safe, "Volcados de errores", "Volcados de pantallazos azules antiguos.", None).contents()),
  ("{LOCALAPPDATA}\\CrashDumps", rule(Safe, "Volcados de errores", "Volcados de programas que se cerraron con error.", None).contents()),
  ("{LOCALAPPDATA}\\Microsoft\\Windows\\WER", rule(Safe, "Informes de errores", "Informes de errores de Windows ya enviados.", Some(TIP_WINDOWS_TEMP_FILES)).contents()),
  ("{ProgramData}\\Microsoft\\Windows\\WER", rule(Safe, "Informes de errores", "Informes de errores de Windows ya enviados.", Some(TIP_WINDOWS_TEMP_FILES)).contents()),
  ("{LOCALAPPDATA}\\Microsoft\\Windows\\INetCache", rule(Safe, "Caché de Internet", "Caché de Internet de Windows; se regenera.", None).contents()),
  ("{LOCALAPPDATA}\\Microsoft\\Windows\\Explorer", rule(Safe, "Miniaturas", "Caché de miniaturas e iconos del Explorador; se regenera.", Some(TIP_WINDOWS_TEMP_FILES)).contents()),
  ("{LOCALAPPDATA}\\D3DSCache", rule(Safe, "Caché de gráficos", "Caché de sombreadores de DirectX; se regenera.", None).contents()),
  ("{LOCALAPPDATA}\\NVIDIA\\DXCache", rule(Safe, "Caché de gráficos", "Caché de sombreadores de NVIDIA; se regenera.", None).contents()),
  ("{LOCALAPPDATA}\\NVIDIA\\GLCache", rule(Safe, "Caché de gráficos", "Caché de sombreadores de NVIDIA; se regenera.", None).contents()),
  ("{LOCALAPPDATA}\\AMD\\DxCache", rule(Safe, "Caché de gráficos", "Caché de sombreadores de AMD; se regenera.", None).contents()),
  ("{SystemDrive}\\Windows.old", rule(Safe, "Windows anterior", "Copia de la instalación anterior de Windows; solo sirve para volver atrás.", Some("Bórrala con Archivos temporales > Instalación anterior de Windows, no a mano.")).tool()),
  ("{SystemDrive}\\$WINDOWS.~BT", rule(Safe, "Windows anterior", "Restos de una actualización de Windows.", Some(TIP_WINDOWS_TEMP_FILES)).tool()),
  ("{SystemDrive}\\$Windows.~WS", rule(Safe, "Windows anterior", "Restos de una actualización de Windows.", Some(TIP_WINDOWS_TEMP_FILES)).tool()),
  // Herramientas de desarrollo
  ("{LOCALAPPDATA}\\npm-cache", rule(Probable, "Caché de paquetes", "Paquetes descargados por npm; se vuelven a bajar al instalar.", Some("npm cache clean --force")).contents()),
  ("{LOCALAPPDATA}\\Yarn\\Cache", rule(Probable, "Caché de paquetes", "Paquetes descargados por Yarn.", Some("yarn cache clean")).contents()),
  ("{LOCALAPPDATA}\\pnpm\\store", rule(Probable, "Caché de paquetes", "Almacén de paquetes de pnpm.", Some("pnpm store prune")).tool()),
  ("{LOCALAPPDATA}\\pip\\cache", rule(Probable, "Caché de paquetes", "Paquetes descargados por pip.", Some("pip cache purge")).contents()),
  ("{USERPROFILE}\\.gradle\\caches", rule(Probable, "Caché de paquetes", "Caché de Gradle; se regenera (la siguiente compilación tardará más).", None).contents()),
  ("{USERPROFILE}\\.gradle\\wrapper\\dists", rule(Probable, "Caché de paquetes", "Versiones de Gradle descargadas; borra las que ya no uses.", None).contents()),
  ("{USERPROFILE}\\.m2\\repository", rule(Probable, "Caché de paquetes", "Dependencias de Maven descargadas.", None).contents()),
  ("{USERPROFILE}\\.nuget\\packages", rule(Probable, "Caché de paquetes", "Paquetes de NuGet descargados.", Some("dotnet nuget locals all --clear")).contents()),
  ("{USERPROFILE}\\.cargo\\registry", rule(Probable, "Caché de paquetes", "Crates de Rust descargados.", None).contents()),
  ("{APPDATA}\\Code\\Cache", rule(Probable, "Caché de programa", "Caché de VS Code; se regenera.", None).contents()),
  ("{APPDATA}\\Code\\CachedData", rule(Probable, "Caché de programa", "Caché de VS Code; se regenera.", None).contents()),
  ("{APPDATA}\\Code\\CachedExtensionVSIXs", rule(Probable, "Caché de programa", "Instaladores de extensiones de VS Code ya instaladas.", None).contents()),
  ("{LOCALAPPDATA}\\Android\\Sdk\\system-images", suggested(Review, "Emuladores", "Imágenes de emulador Android.", Some("Borra las versiones que no uses desde el SDK Manager de Android Studio.")).tool()),
  ("{USERPROFILE}\\.android\\avd", suggested(Review, "Emuladores", "Emuladores Android creados.", Some("Bórralos desde el Device Manager de Android Studio.")).tool()),
  ("{LOCALAPPDATA}\\Docker\\wsl", suggested(Review, "Docker", "Disco virtual de Docker con imágenes y volúmenes.", Some("docker system prune -a y después compacta el disco virtual.")).tool()),
];

#[derive(Clone, Copy)]
enum NameCondition {
  Anywhere,
  DriveRoot,
  ParentHas(u16),
  PathContains(&'static str),
}

/// Reglas por nombre de carpeta, en orden de prioridad.
const DIRECTORY_NAME_RULES: &[(&[&str], NameCondition, RiskRule)] = &[
  (&["$recycle.bin"], NameCondition::DriveRoot, rule(Safe, "Papelera", "Papelera de reciclaje.", Some("Vacíala desde el escritorio.")).tool()),
  (&[QUARANTINE_FOLDER_LOWER_NAME], NameCondition::DriveRoot, suggested(Review, "Cuarentena", "Lo que has apartado con Discos Mapa. No libera espacio hasta que la vacíes.", Some("Vacíala o restáurala desde la pestaña Cuarentena.")).tool()),
  (&["system volume information"], NameCondition::DriveRoot, rule(Protected, "Sistema", "Puntos de restauración e información del sistema.", Some("Gestiona los puntos de restauración en Protección del sistema."))),
  (&[".git"], NameCondition::Anywhere, rule(Protected, "Repositorio", "Historial de un repositorio git. Si lo borras pierdes el historial local.", None)),
  (&[".ssh", ".gnupg"], NameCondition::Anywhere, rule(Protected, "Claves", "Claves de acceso. No se pueden recuperar si se borran.", None)),
  (&["node_modules"], NameCondition::ParentHas(markers::PACKAGE_JSON), rule(Probable, "Dependencias de proyecto", "Dependencias de un proyecto Node; se reinstalan con npm install.", None).item()),
  (&[".angular"], NameCondition::ParentHas(markers::ANGULAR_JSON), rule(Safe, "Caché de compilación", "Caché de Angular CLI; se regenera al compilar.", None).item()),
  (&["dist", ".next", ".nuxt", ".output", ".parcel-cache", ".turbo", ".svelte-kit", ".vite"], NameCondition::ParentHas(markers::PACKAGE_JSON), rule(Probable, "Compilación de proyecto", REASON_BUILD_OUTPUT, None).item()),
  (&["target"], NameCondition::ParentHas(markers::CARGO_TOML), rule(Probable, "Compilación de proyecto", "Compilación de Rust; se regenera con cargo build (tarda).", Some("cargo clean")).item()),
  (&["target"], NameCondition::ParentHas(markers::MAVEN_PROJECT), rule(Probable, "Compilación de proyecto", REASON_BUILD_OUTPUT, Some("mvn clean")).item()),
  (&["bin", "obj"], NameCondition::ParentHas(markers::DOTNET_PROJECT), rule(Probable, "Compilación de proyecto", REASON_BUILD_OUTPUT, Some("dotnet clean")).item()),
  (&["build", ".gradle"], NameCondition::ParentHas(markers::GRADLE_PROJECT), rule(Probable, "Compilación de proyecto", REASON_BUILD_OUTPUT, Some("gradlew clean")).item()),
  (&[".venv", "venv"], NameCondition::ParentHas(markers::PYTHON_PROJECT), rule(Probable, "Dependencias de proyecto", "Entorno virtual de Python; se recrea con pip install.", None).item()),
  (&["__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache"], NameCondition::Anywhere, rule(Safe, "Caché de compilación", "Caché de Python; se regenera.", None).item()),
  (&["cache", "cache_data", "code cache", "gpucache", "shadercache", "grshadercache", "dawncache", "dawngraphitecache", "dawnwebgpucache"], NameCondition::PathContains("\\appdata\\"), rule(Probable, "Caché de programa", "Caché de un programa; se regenera al usarlo. Cierra el programa antes de borrarla.", None).contents()),
];

const RULE_PAGEFILE: RiskRule = rule(Protected, "Sistema", "Memoria virtual de Windows.", Some("Se ajusta en Rendimiento > Opciones avanzadas > Memoria virtual."));
const RULE_HIBERFIL: RiskRule = suggested(Review, "Hibernación", "Fichero de hibernación. Si no usas hibernar, se puede desactivar.", Some("powercfg -h off (como administrador).")).tool();
const RULE_MEMORY_DUMP: RiskRule = rule(Safe, "Volcados de errores", "Volcado de memoria de un pantallazo azul antiguo.", None).item();

pub struct RuleSet {
  path_rules: HashMap<String, RiskRule>,
  system_root: String,
}

impl RuleSet {
  pub fn load() -> Self {
    let path_rules = PATH_RULES
      .iter()
      .filter_map(|(template, rule)| resolve_template(template).map(|path| (path, *rule)))
      .collect();
    let system_root = std::env::var("SystemRoot").map(|path| normalize(&path)).unwrap_or_default();
    Self { path_rules, system_root }
  }

  /// Regla explícita de ruta (`lower_path` en minúsculas y sin barra final).
  pub fn path_rule(&self, lower_path: &str) -> Option<RiskRule> {
    self.path_rules.get(lower_path).copied()
  }

  pub fn classify_directory(&self, lower_path: &str, lower_name: &str, parent_markers: u16, inherited_level: Option<RiskLevel>) -> Option<RiskRule> {
    if let Some(rule) = self.path_rule(lower_path) {
      return Some(rule);
    }
    if inherited_level == Some(Protected) {
      return None;
    }
    DIRECTORY_NAME_RULES
      .iter()
      .find(|(names, condition, _)| names.contains(&lower_name) && matches_condition(*condition, lower_path, parent_markers))
      .map(|(_, _, rule)| *rule)
  }

  pub fn classify_file(&self, lower_parent_path: &str, lower_name: &str) -> Option<RiskRule> {
    let is_drive_root = is_drive_root(lower_parent_path);
    match lower_name {
      "pagefile.sys" | "swapfile.sys" if is_drive_root => Some(RULE_PAGEFILE),
      "hiberfil.sys" if is_drive_root => Some(RULE_HIBERFIL),
      "memory.dmp" if lower_parent_path == self.system_root => Some(RULE_MEMORY_DUMP),
      _ => None,
    }
  }

  /// Regla efectiva de una ruta cualquiera, recorriéndola desde la raíz del disco.
  /// Sin el contexto del escaneo (ficheros de marca), así que las reglas de proyecto no se aplican:
  /// solo sirve para comprobar que no se toca nada protegido.
  pub fn effective_rule_for_path(&self, lower_path: &str, is_file: bool) -> Option<RiskRule> {
    let path = std::path::Path::new(lower_path);
    let mut prefixes: Vec<&std::path::Path> = path.ancestors().collect();
    prefixes.reverse();
    let mut effective: Option<RiskRule> = None;
    for (index, prefix) in prefixes.iter().enumerate() {
      let prefix_text = normalize(&prefix.to_string_lossy());
      let name = prefix.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
      let is_last = index == prefixes.len() - 1;
      let own_rule = if is_last && is_file {
        let parent = prefix.parent().map(|parent| normalize(&parent.to_string_lossy())).unwrap_or_default();
        self.classify_file(&parent, &name)
      } else {
        self.classify_directory(&prefix_text, &name, 0, effective.map(|rule| rule.level))
      };
      if own_rule.is_some() {
        effective = own_rule;
      }
    }
    effective
  }

  /// Nivel heredado por la raíz del escaneo, mirando solo reglas de ruta de sus antepasados.
  pub fn inherited_rule_for(&self, lower_path: &str) -> Option<RiskRule> {
    std::path::Path::new(lower_path)
      .ancestors()
      .skip(1)
      .find_map(|ancestor| self.path_rule(&normalize(&ancestor.to_string_lossy())))
  }
}

fn matches_condition(condition: NameCondition, lower_path: &str, parent_markers: u16) -> bool {
  match condition {
    NameCondition::Anywhere => true,
    NameCondition::DriveRoot => lower_path.rfind('\\').is_some_and(|index| is_drive_root(&lower_path[..index])),
    NameCondition::ParentHas(marker) => parent_markers & marker != 0,
    NameCondition::PathContains(fragment) => lower_path.contains(fragment),
  }
}

fn is_drive_root(lower_path: &str) -> bool {
  let bytes = lower_path.as_bytes();
  bytes.len() == 2 && bytes[1] == b':'
}

/// Minúsculas y sin barra final (`C:\` pasa a `c:`).
pub fn normalize(path: &str) -> String {
  path.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

fn resolve_template(template: &str) -> Option<String> {
  let start = template.find('{')?;
  let end = template.find('}')?;
  let value = std::env::var(&template[start + 1..end]).ok()?;
  Some(normalize(&format!("{}{}{}", &template[..start], value, &template[end + 1..])))
}
