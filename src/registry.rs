use crate::{
    config::Product,
    msi::{
        Database, Result,
        Value::{Int as I, Null as N, Text as T},
    },
    payload,
};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub root: String,
    pub key: String,
    pub name: String,
    pub kind: String,
    pub view: String,
    pub text: String,
    pub strings: Vec<String>,
}
fn literal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '[' => "[\\[]".into(),
            ']' => "[\\]]".into(),
            '{' => "[\\{]".into(),
            '}' => "[\\}]".into(),
            _ => c.to_string(),
        })
        .collect()
}
// Resolve constants once. Runtime directory references remain MSI formatted fields.
pub fn template(s: &str, p: &Product, runtime: bool) -> Result<String> {
    if s.contains('\0') {
        return Err("registry text contains NUL".into());
    }
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                out.push_str(&literal("{"));
                continue;
            }
            let mut name = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                name.push(c);
            }
            if !closed {
                return Err("unclosed registry variable".into());
            }
            match name.as_str() {
                "ProductName" => out.push_str(&literal(&p.name)),
                "Manufacturer" => out.push_str(&literal(&p.manufacturer)),
                "ProductVersion" => out.push_str(&literal(&p.version)),
                "InstallDir" | "ProgramFiles" | "ProgramData" | "WindowsDir" if runtime => {
                    let property = match name.as_str() {
                        "InstallDir" => "INSTALLDIR",
                        "ProgramData" => "CommonAppDataFolder",
                        "WindowsDir" => "WindowsFolder",
                        _ => {
                            if p.architecture == "x64" {
                                "ProgramFiles64Folder"
                            } else {
                                "ProgramFilesFolder"
                            }
                        }
                    };
                    out.push_str(&format!("[{property}]"));
                    // MSI directory properties already end with a backslash.
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                }
                _ => {
                    return Err(
                        format!("unsupported registry variable {{{name}}} in this field").into(),
                    );
                }
            }
        } else if c == '}' {
            if chars.next() != Some('}') {
                return Err("unescaped closing brace in registry text".into());
            }
            out.push_str(&literal("}"));
        } else {
            out.push_str(&literal(&c.to_string()));
        }
    }
    Ok(out)
}
fn dword(s: &str) -> Result<u32> {
    let s = s.trim();
    Ok(if let Some(h) = s.strip_prefix("0x") {
        u32::from_str_radix(h, 16)?
    } else {
        s.parse()?
    })
}
fn binary(s: &str) -> Result<String> {
    let hex: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("REG_BINARY requires nonempty even-length hexadecimal bytes".into());
    }
    Ok(hex.to_ascii_uppercase())
}
impl Entry {
    pub fn is64(&self, p: &Product) -> bool {
        self.view == "registry64" || (self.view == "auto" && p.architecture == "x64")
    }
    pub fn value(&self, p: &Product) -> Result<String> {
        let value = match self.kind.as_str() {
            "REG_SZ" => {
                let v = template(&self.text, p, true)?;
                if v.is_empty() {
                    // A non-null formatted field that resolves to an empty REG_SZ.
                    "[EMB_REG_EMPTY]".into()
                } else if v.starts_with('#') {
                    format!("#{v}")
                } else {
                    v
                }
            }
            "REG_DWORD" => format!("#{}", dword(&self.text)? as i32),
            "REG_BINARY" => format!("#x{}", binary(&self.text)?),
            "REG_MULTI_SZ" => {
                if self.strings.is_empty() || self.strings.iter().any(String::is_empty) {
                    return Err("REG_MULTI_SZ requires at least one nonempty String".into());
                }
                let items = self
                    .strings
                    .iter()
                    .map(|s| template(s, p, true))
                    .collect::<Result<Vec<_>>>()?;
                format!("[~]{}[~]", items.join("[~]"))
            }
            _ => {
                return Err(
                    "registry type must be REG_SZ, REG_DWORD, REG_BINARY, or REG_MULTI_SZ".into(),
                );
            }
        };
        if value.encode_utf16().count() > 32767 {
            return Err("encoded registry value exceeds 32767 characters".into());
        }
        Ok(value)
    }
}
pub fn validate(entries: &[Entry], p: &Product) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut targets = BTreeSet::new();
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
            return Err(
                "invalid or duplicate registry id (use letters, digits, underscore; max 64)".into(),
            );
        }
        if !matches!(e.root.as_str(), "HKLM" | "HKCU") {
            return Err("registry root must be HKLM or HKCU".into());
        }
        if !matches!(e.view.as_str(), "auto" | "registry32" | "registry64")
            || (e.is64(p) && p.architecture == "x86")
        {
            return Err("unsupported registry view for package architecture".into());
        }
        let key = template(&e.key, p, false)?;
        let name = template(&e.name, p, false)?;
        if key.is_empty()
            || key.starts_with('\\')
            || key.ends_with('\\')
            || key.contains("\\\\")
            || key.encode_utf16().count() > 255
            || name.encode_utf16().count() > 255
        {
            return Err("invalid registry key/name or exceeds 255 encoded characters".into());
        }
        if !targets.insert((
            e.root.clone(),
            e.is64(p),
            key.to_uppercase(),
            name.to_uppercase(),
        )) {
            return Err("duplicate registry destination".into());
        }
        if (e.kind == "REG_MULTI_SZ" && !e.text.trim().is_empty())
            || (e.kind != "REG_MULTI_SZ" && !e.strings.is_empty())
        {
            return Err("registry Data shape does not match type".into());
        }
        e.value(p)?;
    }
    Ok(())
}
pub fn author(db: &Database, p: &Product) -> Result<()> {
    validate(&p.registry, p)?;
    for e in &p.registry {
        let key = template(&e.key, p, false)?;
        let name = template(&e.name, p, false)?;
        let value = e.value(p)?;
        let id = format!("R_{}", e.id);
        let component = format!("RC_{}", e.id);
        let canonical_key = key.to_uppercase();
        let canonical_name = name.to_uppercase();
        let identity: String = format!(
            "{}:{}{}:{}",
            canonical_key.len(),
            canonical_key,
            canonical_name.len(),
            canonical_name
        )
        .bytes()
        .map(|b| format!("{b:02X}"))
        .collect();
        let seed = format!(
            "registry|{}|{}|{}|{}",
            crate::config::guid(&p.upgrade_code)?,
            e.root,
            e.is64(p),
            identity
        );
        let guid = payload::component_guid(Path::new(&seed))?;
        db.insert(
            "Component",
            &[
                "Component",
                "ComponentId",
                "Directory_",
                "Attributes",
                "KeyPath",
            ],
            &[
                T(&component),
                T(&guid),
                T("INSTALLDIR"),
                I(4 + if e.is64(p) { 256 } else { 0 }),
                T(&id),
            ],
        )?;
        db.insert(
            "FeatureComponents",
            &["Feature_", "Component_"],
            &[T("Main"), T(&component)],
        )?;
        db.insert(
            "Registry",
            &["Registry", "Root", "Key", "Name", "Value", "Component_"],
            &[
                T(&id),
                I(if e.root == "HKLM" { 2 } else { 1 }),
                T(&key),
                if name.is_empty() { N } else { T(&name) },
                T(&value),
                T(&component),
            ],
        )?;
    }
    Ok(())
}
