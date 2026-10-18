use crate::{
    action_protocol::SEP,
    config::Product,
    msi::{
        Database, Result,
        Value::{Int as I, Text as T},
    },
    payload, registry,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Actions {
    pub install: Vec<Entry>,
    pub uninstall: Vec<Entry>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: String,
    pub path: String,
    pub arguments: Vec<String>,
    pub directory: String,
    pub hide: bool,
    pub policy: String,
    pub timeout: u64,
}
impl Actions {
    pub fn any(&self) -> bool {
        self.install
            .iter()
            .chain(&self.uninstall)
            .any(|e| e.enabled)
    }
    pub fn validate(&self, p: &Product) -> Result<()> {
        for entries in [&self.install, &self.uninstall] {
            if entries.len() > 100 {
                return Err("at most 100 actions per phase are supported".into());
            }
            let mut ids = BTreeSet::new();
            for e in entries {
                if e.id.is_empty()
                    || e.id.len() > 60
                    || !e.id.bytes().enumerate().all(|(i, b)| {
                        b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit())
                    })
                    || !ids.insert(&e.id)
                {
                    return Err(
                        "invalid or duplicate action id (maximum 60 letters, digits, underscores)"
                            .into(),
                    );
                }
                if !matches!(e.policy.as_str(), "ignore" | "notify" | "stop") || e.timeout > 86400 {
                    return Err("invalid action failurePolicy or timeoutSeconds".into());
                }
                if e.name.trim().is_empty() {
                    return Err("action name must not be empty".into());
                }
                for s in [&e.name, &e.path, &e.directory]
                    .into_iter()
                    .chain(e.arguments.iter())
                {
                    if s.contains(['\0', SEP]) {
                        return Err("action text contains NUL or reserved separator".into());
                    }
                }
                if !Path::new(&e.path)
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
                    || Path::new(&e.path)
                        .file_name()
                        .is_some_and(|s| s.eq_ignore_ascii_case("msiexec.exe"))
                {
                    return Err(
                        "actions require a direct EXE; nested MSI execution is unsupported".into(),
                    );
                }
                match e.kind.as_str() {
                    "installedFile" => {
                        if !relative(Path::new(&e.path)) {
                            return Err(
                                "installedFile action must use a relative payload path".into()
                            );
                        }
                    }
                    "targetPath" => {
                        registry::template(&e.path, p, true)?;
                        if !absolute_template(&e.path) {
                            return Err("targetPath action must be an absolute path or start with a directory variable".into());
                        }
                    }
                    _ => return Err("Executable.kind must be installedFile or targetPath".into()),
                }
                if !e.directory.is_empty() {
                    registry::template(&e.directory, p, true)?;
                    if !absolute_template(&e.directory) {
                        return Err("WorkingDirectory must be an absolute path template".into());
                    }
                }
                for a in &e.arguments {
                    registry::template(a, p, true)?;
                }
            }
        }
        Ok(())
    }
}
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}
fn absolute_template(s: &str) -> bool {
    Path::new(s).is_absolute()
        || [
            "{InstallDir}",
            "{ProgramFiles}",
            "{ProgramData}",
            "{WindowsDir}",
        ]
        .iter()
        .any(|v| s.starts_with(v))
}
fn executable(e: &Entry, p: &Product, files: &[payload::File]) -> Result<String> {
    if e.kind == "targetPath" {
        return registry::template(&e.path, p, true);
    }
    let key = payload::key(Path::new(&e.path))?;
    let file = files
        .iter()
        .find(|f| payload::key(&f.relative).is_ok_and(|s| s == key))
        .ok_or_else(|| format!("action {} references a file outside the payload", e.id))?;
    Ok(format!("[#{}]", file.id))
}
pub fn validate_payload(p: &Product, files: &[payload::File]) -> Result<()> {
    for e in p
        .actions
        .install
        .iter()
        .chain(&p.actions.uninstall)
        .filter(|e| e.enabled)
    {
        executable(e, p, files)?;
    }
    Ok(())
}
fn data(e: &Entry, p: &Product, files: &[payload::File], phase: &str) -> Result<String> {
    let mut fields = vec![
        "EMB1".into(),
        format!("{phase}/{}", e.id),
        e.policy.clone(),
        e.hide.to_string(),
        "[UILevel]".into(),
        e.timeout.to_string(),
        executable(e, p, files)?,
        registry::template(&e.directory, p, true)?,
        e.arguments.len().to_string(),
    ];
    fields.extend(
        e.arguments
            .iter()
            .map(|a| registry::template(a, p, true))
            .collect::<Result<Vec<_>>>()?,
    );
    let text = fields.join(&SEP.to_string());
    if text.encode_utf16().count() > 30000 {
        return Err("action data exceeds 30000 characters".into());
    }
    Ok(text)
}
pub fn author(db: &Database, p: &Product, files: &[payload::File], work: &Path) -> Result<()> {
    if !p.actions.any() {
        return Ok(());
    }
    let bytes: &[u8] = if p.architecture == "x64" {
        include_bytes!(concat!(env!("OUT_DIR"), "/emb-actions-x64.dll"))
    } else {
        include_bytes!(concat!(env!("OUT_DIR"), "/emb-actions-x86.dll"))
    };
    let dll = work.join("emb-actions.dll");
    fs::write(&dll, bytes)?;
    db.binary("EmbActionRuntime", &dll)?;
    let mut hidden = vec!["CustomActionData".to_owned()];
    for (phase, entries, base, condition) in [
        (
            "Install",
            &p.actions.install,
            5200,
            "NOT Installed AND NOT REMOVE AND NOT REINSTALL AND NOT UPGRADINGPRODUCTCODE AND NOT EMB_OLD_PRODUCTS",
        ),
        (
            "Uninstall",
            &p.actions.uninstall,
            2000,
            "Installed AND REMOVE ~= \"ALL\" AND NOT UPGRADINGPRODUCTCODE",
        ),
    ] {
        for (index, e) in entries.iter().filter(|e| e.enabled).enumerate() {
            let id = format!(
                "EMB_{}_{}",
                if phase == "Install" { "I" } else { "U" },
                e.id
            );
            let setter = format!("Set_{id}");
            let clear = format!("Clr_{id}");
            hidden.push(id.clone());
            let target = data(e, p, files, phase)?;
            for (action, kind, source, target) in [
                (&setter, 8243, id.as_str(), target.as_str()),
                (&id, 11265, "EmbActionRuntime", "EmbRun"),
                (&clear, 8243, id.as_str(), ""),
            ] {
                db.insert(
                    "CustomAction",
                    &["Action", "Type", "Source", "Target"],
                    &[T(action), I(kind), T(source), T(target)],
                )?;
            }
            for (offset, action) in [&setter, &id, &clear].iter().enumerate() {
                db.insert(
                    "InstallExecuteSequence",
                    &["Action", "Condition", "Sequence"],
                    &[
                        T(action),
                        T(condition),
                        I(base + index as i32 * 3 + offset as i32),
                    ],
                )?;
            }
        }
    }
    db.insert(
        "Property",
        &["Property", "Value"],
        &[T("MsiHiddenProperties"), T(&hidden.join(";"))],
    )?;
    error_dialog(db)?;
    Ok(())
}
pub(crate) fn error_dialog(db: &Database) -> Result<()> {
    use crate::msi::Value::Null as N;
    db.insert(
        "Control",
        &[
            "Dialog_",
            "Control",
            "Type",
            "X",
            "Y",
            "Width",
            "Height",
            "Attributes",
            "Text",
        ],
        &[
            T("ActionError"),
            T("ErrorIcon"),
            T("Icon"),
            I(15),
            I(18),
            I(24),
            I(24),
            I(1048577),
            T("32513"),
        ],
    )?;
    db.insert(
        "Property",
        &["Property", "Value"],
        &[T("ErrorDialog"), T("ActionError")],
    )?;
    db.insert(
        "Dialog",
        &[
            "Dialog",
            "HCentering",
            "VCentering",
            "Width",
            "Height",
            "Attributes",
            "Title",
            "Control_First",
        ],
        &[
            T("ActionError"),
            I(50),
            I(50),
            I(340),
            I(140),
            I(65539),
            T("[ProductName]"),
            T("ErrorText"),
        ],
    )?;
    db.insert(
        "Control",
        &[
            "Dialog_",
            "Control",
            "Type",
            "X",
            "Y",
            "Width",
            "Height",
            "Attributes",
            "Text",
            "Control_Next",
        ],
        &[
            T("ActionError"),
            T("ErrorText"),
            T("Text"),
            I(50),
            I(15),
            I(275),
            I(80),
            I(3),
            T(""),
            N,
        ],
    )?;
    for (name, text, result) in [
        ("O", "확인", "ErrorOk"),
        ("C", "취소", "ErrorCancel"),
        ("A", "중단", "ErrorAbort"),
        ("R", "다시 시도", "ErrorRetry"),
        ("I", "무시", "ErrorIgnore"),
        ("Y", "예", "ErrorYes"),
        ("N", "아니요", "ErrorNo"),
    ] {
        db.insert(
            "Control",
            &[
                "Dialog_",
                "Control",
                "Type",
                "X",
                "Y",
                "Width",
                "Height",
                "Attributes",
                "Text",
                "Control_Next",
            ],
            &[
                T("ActionError"),
                T(name),
                T("PushButton"),
                I(230),
                I(110),
                I(70),
                I(18),
                I(3),
                T(text),
                N,
            ],
        )?;
        db.insert(
            "ControlEvent",
            &[
                "Dialog_",
                "Control_",
                "Event",
                "Argument",
                "Condition",
                "Ordering",
            ],
            &[
                T("ActionError"),
                T(name),
                T("EndDialog"),
                T(result),
                T("1"),
                I(1),
            ],
        )?;
    }
    Ok(())
}
