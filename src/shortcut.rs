use crate::{
    config::Product,
    finish::Launch,
    msi::{
        Database, Result,
        Value::{Int as I, Null as N, Text as T},
    },
    payload,
};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub location: String,
    pub name: String,
    pub target: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub description: String,
}
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}
impl Entry {
    pub fn arguments(&self) -> Result<String> {
        Launch {
            file: self.target.clone(),
            arguments: self.arguments.clone(),
        }
        .target()
    }
    fn resolve<'a>(&self, files: &'a [payload::File]) -> Result<&'a payload::File> {
        let target = payload::key(&self.target)?;
        files
            .iter()
            .find(|f| payload::key(&f.relative).is_ok_and(|s| s == target))
            .ok_or_else(|| {
                format!(
                    "shortcut {} target is not in payload: {}",
                    self.id,
                    self.target.display()
                )
                .into()
            })
    }
    fn working_directory_id(&self, files: &[payload::File]) -> Result<String> {
        if self.working_directory == Path::new(".") {
            return Ok("INSTALLDIR".into());
        }
        let wanted = payload::key(&self.working_directory)?;
        for file in files {
            for parent in file.relative.ancestors().skip(1) {
                if !parent.as_os_str().is_empty() && payload::key(parent)? == wanted {
                    return payload::identifier("D_", parent);
                }
            }
        }
        Err(format!(
            "shortcut {} working directory must contain installed files",
            self.id
        )
        .into())
    }
}
pub fn validate(entries: &[Entry], p: &Product) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for e in entries {
        if e.id.is_empty()
            || e.id.len() > 64
            || !e
                .id
                .bytes()
                .enumerate()
                .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
            || !ids.insert(&e.id)
        {
            return Err("invalid or duplicate shortcut id".into());
        }
        if !matches!(e.location.as_str(), "startMenu" | "desktop") {
            return Err("shortcut location must be startMenu or desktop".into());
        }
        if !relative(Path::new(&e.name))
            || Path::new(&e.name).components().count() != 1
            || e.name.contains(['\\', '/'])
            || e.name.to_ascii_lowercase().ends_with(".lnk")
        {
            return Err("shortcut name must be a plain filename without .lnk extension".into());
        }
        payload::msi_name(Path::new(&e.name))?;
        if !destinations.insert((e.location.clone(), e.name.to_uppercase())) {
            return Err("duplicate shortcut destination".into());
        }
        if !relative(&e.target)
            || !(e.working_directory == Path::new(".") || relative(&e.working_directory))
        {
            return Err(
                "shortcut target/workingDirectory must be relative installed paths without . or .."
                    .into(),
            );
        }
        if e.description.contains('\0') || e.description.encode_utf16().count() > 255 {
            return Err("invalid shortcut description (maximum 255 characters)".into());
        }
        e.arguments()?;
    }
    if entries.iter().any(|e| e.location == "startMenu") {
        payload::msi_name(Path::new(&p.name))?;
        if p.name.contains(['\\', '/']) {
            return Err("Product.name must be a plain folder name for start-menu shortcuts".into());
        }
    }
    Ok(())
}
pub fn validate_payload(entries: &[Entry], files: &[payload::File]) -> Result<()> {
    for e in entries {
        e.resolve(files)?;
        e.working_directory_id(files)?;
    }
    Ok(())
}
pub fn author(db: &Database, p: &Product, files: &[payload::File]) -> Result<()> {
    validate(&p.shortcuts, p)?;
    validate_payload(&p.shortcuts, files)?;
    if p.shortcuts.iter().any(|e| e.location == "desktop") {
        db.insert(
            "Directory",
            &["Directory", "Directory_Parent", "DefaultDir"],
            &[T("DesktopFolder"), T("TARGETDIR"), T(".")],
        )?;
    }
    let menu = p.shortcuts.iter().find(|e| e.location == "startMenu");
    if let Some(e) = menu {
        let name = payload::msi_name(Path::new(&p.name))?;
        db.insert(
            "Directory",
            &["Directory", "Directory_Parent", "DefaultDir"],
            &[T("ProgramMenuFolder"), T("TARGETDIR"), T(".")],
        )?;
        db.insert(
            "Directory",
            &["Directory", "Directory_Parent", "DefaultDir"],
            &[T("EMB_MENU"), T("ProgramMenuFolder"), T(&name)],
        )?;
        let file = e.resolve(files)?;
        db.insert(
            "RemoveFile",
            &[
                "FileKey",
                "Component_",
                "FileName",
                "DirProperty",
                "InstallMode",
            ],
            &[
                T("RemoveMenuFolder"),
                T(&file.component),
                N,
                T("EMB_MENU"),
                I(2),
            ],
        )?;
    }
    for e in &p.shortcuts {
        let file = e.resolve(files)?;
        let args = e.arguments()?;
        let name = payload::msi_name(Path::new(&e.name))?;
        let target = format!("[#{}]", file.id);
        let wd = e.working_directory_id(files)?;
        let id = format!("S_{}", e.id);
        db.insert(
            "Shortcut",
            &[
                "Shortcut",
                "Directory_",
                "Name",
                "Component_",
                "Target",
                "Arguments",
                "Description",
                "ShowCmd",
                "WkDir",
            ],
            &[
                T(&id),
                T(if e.location == "desktop" {
                    "DesktopFolder"
                } else {
                    "EMB_MENU"
                }),
                T(&name),
                T(&file.component),
                T(&target),
                if args.is_empty() { N } else { T(&args) },
                if e.description.is_empty() {
                    N
                } else {
                    T(&e.description)
                },
                I(1),
                T(&wd),
            ],
        )?;
    }
    Ok(())
}
