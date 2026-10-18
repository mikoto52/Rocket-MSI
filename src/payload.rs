use crate::msi::{self, Result};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub struct File {
    pub source: PathBuf,
    pub relative: PathBuf,
    pub id: String,
    pub component: String,
    pub guid: String,
}
pub fn key(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or("file path is not valid Unicode")?
        .replace('/', "\\")
        .to_uppercase())
}
pub fn identifier(prefix: &str, path: &Path) -> Result<String> {
    let hash = msi::sha256(&format!("emb-v1|x64|{}", key(path)?))?;
    Ok(format!(
        "{prefix}{}",
        hash.iter().map(|b| format!("{b:02X}")).collect::<String>()
    ))
}
pub(crate) fn component_guid(path: &Path) -> Result<String> {
    let mut b = msi::sha256(&format!(
        "emb-component-v1|{{9DB58DC8-21CE-45E7-9311-8932DC040002}}|x64|{}",
        key(path)?
    ))?;
    b[6] = (b[6] & 15) | 0x80; // UUID v8: application-defined, SHA-256-derived.
    b[8] = (b[8] & 63) | 0x80;
    let h: String = b[..16].iter().map(|v| format!("{v:02X}")).collect();
    Ok(format!(
        "{{{}-{}-{}-{}-{}}}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}
pub fn namespace(files: &mut [File], product: &crate::config::Product) -> Result<()> {
    for file in files {
        let seed = format!(
            "{}|{}|{}|{}",
            crate::config::guid(&product.upgrade_code)?,
            product.architecture,
            product.install_parts()?.join("\\"),
            key(&file.relative)?
        );
        file.guid = component_guid(Path::new(&seed))?;
    }
    Ok(())
}
pub fn reject_reparse(path: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    if fs::symlink_metadata(path)?.file_attributes() & 0x400 != 0 {
        return Err(format!("reparse points are not supported: {}", path.display()).into());
    }
    Ok(())
}
pub fn collect(source: &Path, output_parent: &Path) -> Result<Vec<File>> {
    reject_reparse(source)?;
    if source.is_file() {
        return Ok(vec![File {
            source: source.to_owned(),
            relative: PathBuf::from("hello.txt"),
            id: "Payload".into(),
            component: "PayloadComponent".into(),
            guid: "{9DB58DC8-21CE-45E7-9311-8932DC040003}".into(),
        }]);
    }
    if !source.is_dir() {
        return Err("source must be a file or directory".into());
    }
    let root = fs::canonicalize(source)?;
    let root_key = key(&root)?;
    let output_key = key(output_parent)?;
    if output_key == root_key || output_key.starts_with(&format!("{root_key}\\")) {
        return Err("output directory must be outside the source directory".into());
    }
    let mut paths = Vec::new();
    walk(&root, &root, &mut paths, 0)?;
    paths.sort_by_cached_key(|p| key(p).unwrap_or_default());
    if paths.is_empty() {
        return Err("source folder contains no files".into());
    }
    if paths.len() > 32767 {
        return Err("prototype supports at most 32767 files".into());
    }
    let mut seen = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut files = Vec::new();
    for relative in paths {
        if !seen.insert(key(&relative)?) {
            return Err("case-insensitive target path collision".into());
        }
        let id = identifier("F_", &relative)?;
        if !ids.insert(id.clone()) {
            return Err("file identifier collision".into());
        }
        files.push(File {
            source: root.join(&relative),
            component: identifier("C_", &relative)?,
            guid: component_guid(&relative)?,
            relative,
            id,
        });
    }
    Ok(files)
}
fn walk(root: &Path, dir: &Path, paths: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 64 {
        return Err("source folder depth exceeds 64".into());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        reject_reparse(&path)?;
        if path.is_dir() {
            walk(root, &path, paths, depth + 1)?;
        } else if path.is_file() {
            paths.push(path.strip_prefix(root)?.to_owned());
        } else {
            return Err(format!("unsupported source: {}", path.display()).into());
        }
    }
    Ok(())
}
/// Generate 8.3 aliases independent of enumeration order; reject collisions.
pub fn msi_name(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid file name")?;
    if name.ends_with(['.', ' '])
        || name.contains(['|', ':', '*', '?', '"', '<', '>'])
        || name.chars().any(char::is_control)
    {
        return Err(format!("unsupported MSI file name: {name}").into());
    }
    let id = identifier("", path)?;
    let result = format!("E{}|{name}", &id[..7]);
    if result.encode_utf16().count() > 255 {
        return Err(format!("MSI name exceeds 255 characters: {name}").into());
    }
    Ok(result)
}
