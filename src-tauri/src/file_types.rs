//! Agrupa las extensiones de fichero en categorías para colorear el mapa.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileCategory {
  Video,
  Image,
  Audio,
  Document,
  Archive,
  DiskImage,
  Code,
  Executable,
  System,
  Other,
}

pub fn category_for_extension(lower_extension: &str) -> FileCategory {
  use FileCategory::*;
  match lower_extension {
    "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "flv" | "mpg" | "mpeg" | "m2ts" => Video,
    "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "tif" | "tiff" | "raw" | "cr2" | "nef" | "arw" | "psd" | "svg" | "ico" => Image,
    "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" | "opus" => Audio,
    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "txt" | "md" | "csv" | "epub" | "rtf" => Document,
    "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cab" | "tgz" => Archive,
    "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "vdi" | "qcow2" | "wim" | "esd" => DiskImage,
    "js" | "ts" | "mjs" | "cjs" | "jsx" | "tsx" | "json" | "html" | "css" | "scss" | "rs" | "cs" | "java" | "kt" | "py" | "c" | "cpp" | "h" | "hpp" | "go" | "rb" | "php" | "xml" | "yml" | "yaml" | "toml" | "map" | "sql" => Code,
    "exe" | "msi" | "dll" | "sys" | "appx" | "msix" | "apk" | "jar" | "bat" | "cmd" | "ps1" | "so" | "lib" | "pdb" | "node" => Executable,
    "dat" | "log" | "etl" | "evtx" | "db" | "sqlite" | "tmp" | "cache" | "pak" | "bin" | "blob" | "mui" | "cat" | "manifest" => System,
    _ => Other,
  }
}

/// Extensión en minúsculas, o cadena vacía si no tiene.
pub fn lower_extension(file_name: &str) -> String {
  match file_name.rfind('.') {
    Some(index) if index > 0 && index < file_name.len() - 1 => file_name[index + 1..].to_lowercase(),
    _ => String::new(),
  }
}
