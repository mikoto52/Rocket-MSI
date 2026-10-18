use crate::{
    config::Product,
    finish::Launch,
    msi::{self, Result},
    package::{self, CabStorage},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use xml::reader::{EventReader, XmlEvent};

pub struct Project {
    pub product: Product,
    pub source: String,
    pub output_directory: String,
    pub output_file: String,
    pub license: Option<String>,
    pub branding: String,
    pub storage: CabStorage,
    pub cab_limit: Option<u64>,
    pub launch: Option<Launch>,
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn temp() -> Result<Temp> {
        let p = std::env::current_dir()?
            .join("target")
            .join(format!("project-test-{}", msi::guid()?));
        fs::create_dir_all(&p)?;
        Ok(Temp(p))
    }
    #[test]
    fn xml_roundtrip_preserves_guids_unicode_and_exact_arguments() -> Result<()> {
        let mut p = Project::new()?;
        p.product.name = "샘플 & 도구".into();
        p.branding = "Emb <Builder> & 친구".into();
        p.launch = Some(Launch {
            file: "bin/app.exe".into(),
            arguments: vec![
                "".into(),
                "  공백 & <인수>  ".into(),
                "[literal]".into(),
                "a\"b\\".into(),
            ],
        });
        p.storage = CabStorage::External;
        p.cab_limit = Some(1024 * 1024);
        let bytes = p.xml()?;
        let q = Project::parse(std::str::from_utf8(&bytes)?)?;
        assert_eq!(p.product, q.product);
        assert_eq!(p.branding, q.branding);
        assert_eq!(
            p.launch.as_ref().unwrap().arguments,
            q.launch.as_ref().unwrap().arguments
        );
        assert_eq!(bytes, q.xml()?);
        let t = temp()?;
        let path = t.0.join("sample.embproj");
        p.save_new(&path)?;
        assert!(q.save_new(&path).is_err());
        assert_eq!(Project::load(&path)?.product, p.product);
        assert_ne!(Project::new()?.product.product_code, p.product.product_code);
        Ok(())
    }
    #[test]
    fn unsupported_and_invalid_xml_is_rejected() -> Result<()> {
        let xml = String::from_utf8(Project::new()?.xml()?)?;
        for bad in [
            xml.replace("schemaVersion=\"1\"", "schemaVersion=\"2\""),
            xml.replace("<UI>", "<UI><Registry/>"),
            xml.replace(
                "allowDirectoryChange=\"true\"",
                "allowDirectoryChange=\"yes\"",
            ),
            xml.replace("version=\"1.0.0\"", "version=\"256.0.0\""),
            xml.replace("<UI>", "<Source directory=\"x\"/><UI>"),
            xml.replace("{ProgramFiles}", "{Unknown}"),
            xml.replace("existingFiles=\"default\"", "existingFiles=\"unknown\""),
            xml.replace("split=\"none\"", "split=\"size\" targetSizeMiB=\"0\""),
            "<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///C:/secret'>]><x/>".into(),
            "<broken>".into(),
        ] {
            assert!(Project::parse(&bad).is_err(), "accepted invalid XML: {bad}");
        }
        Ok(())
    }
    #[test]
    fn project_build_resolves_relative_paths_and_authors_product_and_architecture() -> Result<()> {
        let t = temp()?;
        fs::create_dir(t.0.join("payload"))?;
        fs::write(t.0.join("payload/app.exe"), b"test payload; never executed")?;
        fs::write(t.0.join("terms.txt"), "시험 약관")?;
        let mut p = Project::new()?;
        p.license = Some("./terms.txt".into());
        p.launch = Some(Launch {
            file: "app.exe".into(),
            arguments: vec!["two words".into()],
        });
        p.product.allow_directory_change = false;
        let mut components = Vec::new();
        for arch in ["x86", "x64"] {
            p.product.architecture = arch.into();
            p.output_directory = format!("./out-{arch}");
            let path = t.0.join(format!("{arch}.embproj"));
            p.save_new(&path)?;
            let output = Project::load(&path)?.build(&path)?;
            msi::verify_package(&output)?;
            let db = msi::Database::read(&output)?;
            assert_eq!(
                db.query(
                    "SELECT `Value` FROM `Property` WHERE `Property` = 'ProductCode'",
                    1
                )?[0][0],
                p.product.product_code
            );
            assert_eq!(
                db.query(
                    "SELECT `Value` FROM `Property` WHERE `Property` = 'ProductName'",
                    1
                )?[0][0],
                "MyApp"
            );
            assert_eq!(
                db.query("SELECT `Attributes` FROM `Component`", 1)?[0][0],
                if arch == "x64" { "256" } else { "0" }
            );
            assert_eq!(db.query("SELECT `Attributes` FROM `Control` WHERE `Dialog_` = 'Path' AND `Control` = 'Folder'",1)?[0][0],"1");
            components.push(db.query("SELECT `ComponentId` FROM `Component`", 1)?[0][0].clone());
            assert_eq!(
                db.query(
                    "SELECT `Action` FROM `CustomAction` WHERE `Action` = 'LaunchAfterInstall'",
                    1
                )?
                .len(),
                1
            );
            let mut files = crate::payload::collect(&t.0.join("payload"), &t.0)?;
            crate::payload::namespace(&mut files, &p.product)?;
            assert_eq!(components.last().unwrap(), &files[0].guid);
        }
        assert_ne!(components[0], components[1]);
        Ok(())
    }
    #[test]
    fn file_policies_roundtrip_and_control_install_and_repair() -> Result<()> {
        let t = temp()?;
        fs::create_dir(t.0.join("payload"))?;
        fs::write(t.0.join("payload/settings.txt"), "package contents")?;
        for policy in [
            crate::config::ExistingFiles::Default,
            crate::config::ExistingFiles::Skip,
            crate::config::ExistingFiles::Overwrite,
        ] {
            let mut p = Project::new()?;
            p.product.existing_files = policy;
            p.output_directory = format!("out-{}", policy.name());
            let bytes = p.xml()?;
            let q = Project::parse(std::str::from_utf8(&bytes)?)?;
            assert_eq!(q.product.existing_files, policy);
            let path = t.0.join(format!("{}.embproj", policy.name()));
            q.save_new(&path)?;
            let output = q.build(&path)?;
            let db = msi::Database::read(&output)?;
            assert_eq!(
                db.query(
                    "SELECT `Value` FROM `Property` WHERE `Property` = 'REINSTALLMODE'",
                    1
                )?[0][0],
                policy.mode()
            );
            assert_eq!(db.query("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Maintenance' AND `Event` = 'ReinstallMode'",1)?[0][0],policy.mode());
            assert!(db.query("SELECT `Version` FROM `File`", 1)?[0][0].is_empty());
        }
        let xml =
            String::from_utf8(Project::new()?.xml()?)?.replace(" existingFiles=\"default\"", "");
        assert_eq!(
            Project::parse(&xml)?.product.existing_files,
            crate::config::ExistingFiles::Default
        );
        let system = std::env::var("SystemRoot")?;
        let version = msi::file_version(&PathBuf::from(system).join("System32/msi.dll"))?
            .ok_or("msi.dll has no version")?;
        assert!(!version.0.is_empty());
        Ok(())
    }
    #[test]
    fn registry_project_roundtrips_and_authors_native_components() -> Result<()> {
        let mut p = Project::load(Path::new("examples/RegistryDemo.embproj"))?;
        let q = Project::parse(std::str::from_utf8(&p.xml()?)?)?;
        assert_eq!(p.product, q.product);
        let t = temp()?;
        p.source = std::env::current_dir()?
            .join("examples/payload")
            .to_str()
            .unwrap()
            .into();
        p.output_directory = "out".into();
        let path = t.0.join("registry.embproj");
        p.save_new(&path)?;
        let output = p.build(&path)?;
        msi::verify_package(&output)?;
        let db = msi::Database::read(&output)?;
        assert_eq!(db.query("SELECT `Registry` FROM `Registry`", 1)?.len(), 14);
        for (id, value) in [
            ("R_enabled", "#1"),
            ("R_max_dword", "#-1"),
            ("R_blob", "#x00017F80FEFF"),
            ("R_install_path", "[INSTALLDIR]"),
            ("R_single", "[~]#one[~]"),
            (
                "R_paths",
                "[~][INSTALLDIR]hello.txt[~]  공백 유지  [~][\\[]literal[\\]][~]",
            ),
        ] {
            assert_eq!(
                db.query(
                    &format!("SELECT `Value` FROM `Registry` WHERE `Registry` = '{id}'"),
                    1
                )?[0][0],
                value
            );
        }
        assert_eq!(
            db.query(
                "SELECT `Attributes`,`KeyPath` FROM `Component` WHERE `Component` = 'RC_blob'",
                2
            )?[0],
            vec!["260", "R_blob"]
        );
        assert_eq!(
            db.query(
                "SELECT `Attributes` FROM `Component` WHERE `Component` = 'RC_view32'",
                1
            )?[0][0],
            "4"
        );
        for (action, seq) in [
            ("RemoveRegistryValues", "2600"),
            ("WriteRegistryValues", "5000"),
        ] {
            assert_eq!(db.query(&format!("SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action` = '{action}'"),1)?[0][0],seq);
        }
        // Same destinations retain component identities even when entry ids or data change.
        let before = db.query(
            "SELECT `ComponentId` FROM `Component` WHERE `Component` = 'RC_blob'",
            1,
        )?[0][0]
            .clone();
        drop(db);
        p.product
            .registry
            .iter_mut()
            .find(|e| e.id == "blob")
            .unwrap()
            .id = "renamed_blob".into();
        p.output_directory = "out2".into();
        let output = p.build(&path)?;
        let db = msi::Database::read(&output)?;
        assert_eq!(
            db.query(
                "SELECT `ComponentId` FROM `Component` WHERE `Component` = 'RC_renamed_blob'",
                1
            )?[0][0],
            before
        );
        Ok(())
    }
    #[test]
    fn invalid_registry_xml_and_ambiguous_targets_are_rejected() -> Result<()> {
        let text = fs::read_to_string("examples/RegistryDemo.embproj")?;
        for bad in [
            text.replace("REG_BINARY", "REG_EXPAND_SZ"),
            text.replace("00 01 7F 80 FE FF", "00 GG"),
            text.replace("0xFFFFFFFF", "4294967296"),
            text.replace("{WindowsDir}", "{Unknown}"),
            text.replace("name=\"Blob\"", "name=\"Enabled\""),
            text.replace("<String>#one</String>", "<String></String>"),
            text.replace(
                "type=\"REG_BINARY\"",
                "type=\"REG_BINARY\" uninstall=\"keep\"",
            ),
            text.replace(
                "architecture=\"x64\"",
                "architecture=\"x86\" view=\"registry64\"",
            ),
            text.replace("<Data>1</Data>", "<Data><String>1</String></Data>"),
        ] {
            assert!(
                Project::parse(&bad).is_err(),
                "accepted invalid registry XML: {bad}"
            );
        }
        Ok(())
    }
    #[test]
    fn shortcuts_roundtrip_and_reference_installed_files_and_directories() -> Result<()> {
        let mut p = Project::load(Path::new("examples/ShortcutDemo.embproj"))?;
        assert_eq!(
            p.product,
            Project::parse(std::str::from_utf8(&p.xml()?)?)?.product
        );
        let t = temp()?;
        fs::create_dir_all(t.0.join("payload/bin"))?;
        fs::create_dir_all(t.0.join("payload/data"))?;
        fs::write(
            t.0.join("payload/bin/ShortcutProbe.exe"),
            b"non-executable test fixture",
        )?;
        fs::write(
            t.0.join("payload/data/readme.txt"),
            b"working directory fixture",
        )?;
        p.source = "payload".into();
        p.output_directory = "out".into();
        let path = t.0.join("shortcuts.embproj");
        p.save_new(&path)?;
        let output = p.build(&path)?;
        let db = msi::Database::read(&output)?;
        let files = crate::payload::collect(&t.0.join("payload"), &t.0)?;
        let target = files
            .iter()
            .find(|f| f.relative == Path::new("bin/ShortcutProbe.exe"))
            .unwrap();
        let rows=db.query("SELECT `Directory_`,`Component_`,`Target`,`Arguments`,`WkDir` FROM `Shortcut` WHERE `Shortcut` = 'S_menu'",5)?;
        assert_eq!(
            rows[0],
            vec![
                "EMB_MENU".to_owned(),
                target.component.clone(),
                format!("[#{}]", target.id),
                "\"start menu\" \"[\\[]literal[\\]]\" \"\"".into(),
                crate::payload::identifier("D_", Path::new("data"))?
            ]
        );
        assert_eq!(
            db.query(
                "SELECT `WkDir` FROM `Shortcut` WHERE `Shortcut` = 'S_desktop'",
                1
            )?[0][0],
            "INSTALLDIR"
        );
        assert_eq!(
            db.query(
                "SELECT `Directory_Parent` FROM `Directory` WHERE `Directory` = 'EMB_MENU'",
                1
            )?[0][0],
            "ProgramMenuFolder"
        );
        assert_eq!(
            db.query("SELECT `InstallMode`,`FileName` FROM `RemoveFile`", 2)?[0],
            vec!["2", ""]
        );
        for (action, seq) in [("RemoveShortcuts", "3200"), ("CreateShortcuts", "4500")] {
            assert_eq!(db.query(&format!("SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action` = '{action}'"),1)?[0][0],seq);
        }
        drop(db);
        // Reject broken references before CAB generation or publishing output.
        p.output_directory = "bad-out".into();
        p.product.shortcuts[0].target = "missing.exe".into();
        assert!(p.build(&path).is_err());
        assert!(!t.0.join("bad-out/Setup.msi").exists());
        p.product.shortcuts[0].target = "bin/ShortcutProbe.exe".into();
        p.product.shortcuts[0].working_directory = "missing-dir".into();
        assert!(p.build(&path).is_err());
        Ok(())
    }
    #[test]
    fn shortcut_invalid_destinations_and_paths_are_rejected() -> Result<()> {
        let xml = fs::read_to_string("examples/ShortcutDemo.embproj")?;
        for bad in [
            xml.replace("location=\"desktop\"", "location=\"startMenu\""),
            xml.replace("location=\"desktop\"", "location=\"startup\""),
            xml.replace("id=\"desktop\"", "id=\"menu\""),
            xml.replace(
                "target=\"bin/ShortcutProbe.exe\"",
                "target=\"../outside.exe\"",
            ),
            xml.replace(
                "workingDirectory=\"data\"",
                "workingDirectory=\"C:\\Windows\"",
            ),
            xml.replace("name=\"InstallEmbly 바로가기 시험\"", "name=\"bad/name\""),
            xml.replace("<Shortcuts>", "<Shortcuts><Unknown/>"),
            xml.replace(
                "workingDirectory=\"data\"",
                "workingDirectory=\"data\" unsupported=\"true\"",
            ),
        ] {
            assert!(
                Project::parse(&bad).is_err(),
                "accepted invalid shortcut XML: {bad}"
            );
        }
        Ok(())
    }
    #[test]
    fn actions_roundtrip_and_schedule_deferred_runtime_with_failure_policies() -> Result<()> {
        let mut p = Project::load(Path::new("examples/ActionsDemo.embproj"))?;
        assert_eq!(
            p.product,
            Project::parse(std::str::from_utf8(&p.xml()?)?)?.product
        );
        let t = temp()?;
        fs::create_dir(t.0.join("payload"))?;
        fs::write(
            t.0.join("payload/ActionProbe.exe"),
            b"test fixture; never executed",
        )?;
        p.source = "payload".into();
        p.license = Some(
            std::env::current_dir()?
                .join("examples/demo-terms.txt")
                .to_str()
                .unwrap()
                .into(),
        );
        for arch in ["x86", "x64"] {
            p.product.architecture = arch.into();
            p.output_directory = format!("out-{arch}");
            let path = t.0.join(format!("{arch}.embproj"));
            p.save_new(&path)?;
            let output = p.build(&path)?;
            let db = msi::Database::read(&output)?;
            let rows = db.query(
                "SELECT `Action`,`Type`,`Source`,`Target` FROM `CustomAction`",
                4,
            )?;
            let deferred: Vec<_> = rows.iter().filter(|r| r[1] == "11265").collect();
            assert_eq!(deferred.len(), 8);
            assert!(
                deferred
                    .iter()
                    .all(|r| r[2] == "EmbActionRuntime" && r[3] == "EmbRun")
            );
            assert!(rows.iter().all(|r| !r[0].contains("disabled")));
            let setters: Vec<_> = rows.iter().filter(|r| r[0].starts_with("Set_")).collect();
            assert!(setters.iter().any(|r| r[3].contains("\u{1f}ignore\u{1f}")));
            assert!(setters.iter().any(|r| r[3].contains("\u{1f}notify\u{1f}")));
            assert!(setters.iter().any(|r| r[3].contains("\u{1f}stop\u{1f}")));
            let install=db.query("SELECT `Action`,`Condition`,`Sequence` FROM `InstallExecuteSequence` WHERE `Action` = 'EMB_I_first'",3)?;
            assert_eq!(install[0][2], "5201");
            assert!(install[0][1].contains("NOT Installed"));
            let uninstall=db.query("SELECT `Condition`,`Sequence` FROM `InstallExecuteSequence` WHERE `Action` = 'EMB_U_first'",2)?;
            assert_eq!(uninstall[0][1], "2001");
            assert!(uninstall[0][0].contains("REMOVE ~= \"ALL\""));
            assert!(uninstall[0][0].contains("NOT UPGRADINGPRODUCTCODE"));
            assert_eq!(db.query("SELECT `Attributes`,`Control_First` FROM `Dialog` WHERE `Dialog` = 'ActionError'",2)?[0],vec!["65539","ErrorText"]);
            assert!(
                db.query(
                    "SELECT `Control_Next` FROM `Control` WHERE `Dialog_` = 'ActionError'",
                    1
                )?
                .iter()
                .all(|r| r[0].is_empty())
            );
        }
        Ok(())
    }
    #[test]
    fn invalid_actions_are_rejected_and_protocol_preserves_empty_arguments() -> Result<()> {
        let xml = fs::read_to_string("examples/ActionsDemo.embproj")?;
        for bad in [
            xml.replace("failurePolicy=\"ignore\"", "failurePolicy=\"continue\""),
            xml.replace("hideWindow=\"true\"", "hideWindow=\"yes\""),
            xml.replace("path=\"ActionProbe.exe\"", "path=\"../outside.exe\""),
            xml.replace("path=\"{InstallDir}\\Missing.exe\"", "path=\"Missing.exe\""),
            xml.replace("id=\"first\"", "id=\"first\" timeoutSeconds=\"0\""),
            xml.replace("id=\"first\"", "id=\"first\" runOnUpgrade=\"true\""),
            xml.replace("{ProgramData}", "{Unknown}"),
            xml.replace("id=\"last\"", "id=\"first\""),
        ] {
            assert!(Project::parse(&bad).is_err(), "accepted invalid action XML");
        }
        let s = crate::action_protocol::SEP.to_string();
        let data = [
            "EMB1",
            "Install/a",
            "notify",
            "true",
            "5",
            "3",
            "C:\\app.exe",
            "C:\\",
            "3",
            "",
            "a b",
            "a\"b\\",
        ]
        .join(&s);
        let r = crate::action_protocol::Request::parse(&data).unwrap();
        assert_eq!(r.args, vec!["", "a b", "a\"b\\"]);
        assert!(r.hide);
        assert_eq!(r.policy, "notify");
        assert_eq!(r.id, "Install/a");
        assert_eq!(r.ui, 5);
        assert_eq!(r.timeout, 3);
        assert_eq!(r.executable, "C:\\app.exe");
        assert_eq!(r.directory, "C:\\");
        assert!(crate::action_protocol::Request::parse("bad").is_err());
        assert_eq!(crate::action_protocol::quote("a\"b\\"), "\"a\\\"b\\\\\"");
        Ok(())
    }
    #[test]
    fn upgrade_tables_block_conflicts_and_preserve_shared_component_identity() -> Result<()> {
        let t = temp()?;
        fs::create_dir(t.0.join("payload"))?;
        fs::write(t.0.join("payload/shared.txt"), b"v1")?;
        let mut p = Project::new()?;
        p.source = "payload".into();
        p.product.name = "Upgrade Test".into();
        let upgrade = p.product.upgrade_code.clone();
        let mut ids = Vec::new();
        for version in ["1.0.0", "2.0.0"] {
            p.product.version = version.into();
            p.product.product_code = msi::guid()?;
            p.output_directory = format!("out-{version}");
            let path = t.0.join(format!("{version}.embproj"));
            p.save_new(&path)?;
            let output = p.build(&path)?;
            msi::verify_app_search(&output)?;
            let db = msi::Database::read(&output)?;
            assert_eq!(db.query("SELECT `UpgradeCode`,`VersionMax`,`Attributes` FROM `Upgrade` WHERE `ActionProperty` = 'EMB_OLD_PRODUCTS'",3)?[0],vec![upgrade.clone(),version.into(),"0".into()]);
            assert_eq!(db.query("SELECT `Attributes` FROM `Upgrade` WHERE `ActionProperty` = 'EMB_SAME_PRODUCTS'",1)?[0][0],"770");
            assert_eq!(db.query("SELECT `Attributes` FROM `Upgrade` WHERE `ActionProperty` = 'EMB_NEWER_PRODUCTS'",1)?[0][0],"2");
            assert_eq!(db.query("SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action` = 'InstallExecute'",1)?[0][0],"6500");
            assert_eq!(db.query("SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action` = 'RemoveExistingProducts'",1)?[0][0],"6550");
            assert_eq!(
                db.query(
                    "SELECT `Type` FROM `RegLocator` WHERE `Signature_` = 'EmbPreviousDir'",
                    1
                )?[0][0],
                "18"
            );
            assert_eq!(db.query("SELECT `Sequence` FROM `InstallUISequence` WHERE `Action` = 'EmbBlockLayoutChange'",1)?[0][0],"80");
            ids.push(db.query("SELECT `ComponentId` FROM `Component`", 1)?);
        }
        assert_eq!(ids[0], ids[1]);
        Ok(())
    }
}
#[derive(Default)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<Node>,
    text: String,
    line: u64,
}
impl Node {
    fn error(&self, message: &str) -> Box<dyn std::error::Error> {
        format!("XML line {} <{}>: {message}", self.line, self.name).into()
    }
    fn check(&self, attrs: &[&str], children: &[&str], text: bool) -> Result<()> {
        for name in self.attrs.keys() {
            if !attrs.contains(&name.as_str()) {
                return Err(self.error(&format!("unsupported attribute {name}")));
            }
        }
        for child in &self.children {
            if !children.contains(&child.name.as_str()) {
                return Err(child.error("unsupported element; feature may not be implemented yet"));
            }
        }
        if !text && !self.text.trim().is_empty() {
            return Err(self.error("unexpected text"));
        }
        Ok(())
    }
    fn attr(&self, name: &str) -> Result<&str> {
        self.attrs
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| self.error(&format!("missing attribute {name}")))
    }
    fn optional(&self, name: &str) -> Result<Option<&Node>> {
        let mut matches = self.children.iter().filter(|n| n.name == name);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(self.error(&format!("duplicate element {name}")));
        }
        Ok(first)
    }
    fn required(&self, name: &str) -> Result<&Node> {
        self.optional(name)?
            .ok_or_else(|| self.error(&format!("missing element {name}")))
    }
}
fn boolean(value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("boolean must be true or false".into()),
    }
}
fn tree(text: &str) -> Result<Node> {
    use xml::common::Position;
    if text.len() > 1024 * 1024 {
        return Err("project exceeds 1 MiB".into());
    }
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("DTD and entity declarations are prohibited".into());
    }
    let mut reader = EventReader::from_str(text);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut count = 0;
    loop {
        match reader.next()? {
            XmlEvent::StartElement {
                name,
                attributes,
                namespace,
                ..
            } => {
                count += 1;
                if count > 10000 || stack.len() >= 64 {
                    return Err("XML structure limit exceeded".into());
                }
                if name.namespace.is_some()
                    || name.prefix.is_some()
                    || namespace
                        .iter()
                        .any(|(p, _)| !matches!(p, "xml" | "xmlns" | ""))
                {
                    return Err("XML namespaces are not supported".into());
                }
                let mut node = Node {
                    name: name.local_name,
                    line: reader.position().row + 1,
                    ..Node::default()
                };
                for a in attributes {
                    if a.name.namespace.is_some() || a.name.prefix.is_some() {
                        return Err("namespaced attributes are unsupported".into());
                    }
                    node.attrs.insert(a.name.local_name, a.value);
                }
                stack.push(node);
            }
            XmlEvent::EndElement { .. } => {
                let node = stack.pop().ok_or("unexpected closing tag")?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            XmlEvent::Characters(s) | XmlEvent::CData(s) | XmlEvent::Whitespace(s) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&s);
                }
            }
            XmlEvent::EndDocument => break,
            XmlEvent::ProcessingInstruction { .. } => {
                return Err("processing instructions are unsupported".into());
            }
            _ => {}
        }
    }
    root.ok_or_else(|| "missing root element".into())
}
impl Project {
    pub fn new() -> Result<Self> {
        Ok(Self {
            product: Product {
                name: "MyApp".into(),
                manufacturer: "MyCompany".into(),
                version: "1.0.0".into(),
                product_code: msi::guid()?,
                upgrade_code: msi::guid()?,
                architecture: "x64".into(),
                installation: "{ProgramFiles}\\{Manufacturer}\\{ProductName}".into(),
                allow_directory_change: true,
                existing_files: crate::config::ExistingFiles::Default,
                registry: Vec::new(),
                shortcuts: Vec::new(),
                actions: crate::actions::Actions::default(),
                upgrade: true,
            },
            source: "./payload".into(),
            output_directory: "./dist".into(),
            output_file: "Setup.msi".into(),
            license: None,
            branding: "Express MSI Builder".into(),
            storage: CabStorage::Embedded,
            cab_limit: None,
            launch: None,
        })
    }
    pub fn load(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        Self::parse(std::str::from_utf8(&bytes)?.trim_start_matches('\u{feff}'))
    }
    pub fn parse(text: &str) -> Result<Self> {
        let root = tree(text)?;
        if root.name != "ExpressMsiBuilderProject" {
            return Err(root.error("expected ExpressMsiBuilderProject root"));
        }
        root.check(
            &["schemaVersion"],
            &[
                "Product",
                "Source",
                "Installation",
                "UI",
                "Build",
                "Registry",
                "Shortcuts",
                "Actions",
                "Upgrade",
            ],
            false,
        )?;
        if root.attr("schemaVersion")? != "1" {
            return Err(root.error("unsupported schemaVersion"));
        }
        let p = root.required("Product")?;
        p.check(
            &[
                "name",
                "manufacturer",
                "version",
                "productCode",
                "upgradeCode",
                "architecture",
                "scope",
            ],
            &[],
            false,
        )?;
        if p.attr("scope")? != "perMachine" {
            return Err(p.error("only perMachine scope is implemented"));
        }
        let installation = root.required("Installation")?;
        installation.check(
            &["directory", "allowDirectoryChange", "existingFiles"],
            &[],
            false,
        )?;
        let product = Product {
            name: p.attr("name")?.into(),
            manufacturer: p.attr("manufacturer")?.into(),
            version: p.attr("version")?.into(),
            product_code: crate::config::guid(p.attr("productCode")?)?,
            upgrade_code: crate::config::guid(p.attr("upgradeCode")?)?,
            architecture: p.attr("architecture")?.into(),
            installation: installation.attr("directory")?.into(),
            allow_directory_change: boolean(installation.attr("allowDirectoryChange")?)?,
            existing_files: crate::config::ExistingFiles::parse(
                installation
                    .attrs
                    .get("existingFiles")
                    .map(String::as_str)
                    .unwrap_or("default"),
            )?,
            registry: Vec::new(),
            shortcuts: Vec::new(),
            actions: crate::actions::Actions::default(),
            upgrade: false,
        };
        product.validate().map_err(|e| p.error(&e.to_string()))?;
        let source = root.required("Source")?;
        source.check(&["directory"], &[], false)?;
        let build = root.required("Build")?;
        build.check(&["outputDirectory", "outputFile"], &["Cabinet"], false)?;
        let mut result = Self {
            product,
            source: source.attr("directory")?.into(),
            output_directory: build.attr("outputDirectory")?.into(),
            output_file: build.attr("outputFile")?.into(),
            license: None,
            branding: "Express MSI Builder".into(),
            storage: CabStorage::Embedded,
            cab_limit: None,
            launch: None,
        };
        if let Some(cab) = build.optional("Cabinet")? {
            cab.check(&["storage", "split", "targetSizeMiB"], &[], false)?;
            result.storage = match cab.attr("storage")? {
                "embedded" => CabStorage::Embedded,
                "external" => CabStorage::External,
                _ => return Err(cab.error("storage must be embedded or external")),
            };
            match cab.attr("split")? {
                "none" => {
                    if cab.attrs.contains_key("targetSizeMiB") {
                        return Err(cab.error("targetSizeMiB requires split=size"));
                    }
                }
                "size" => {
                    let mib = cab.attr("targetSizeMiB")?.parse::<u64>()?;
                    if !(1..=2047).contains(&mib) {
                        return Err(cab.error("targetSizeMiB must be 1..2047"));
                    }
                    result.cab_limit = Some(mib * 1024 * 1024);
                }
                _ => return Err(cab.error("split must be none or size")),
            }
        }
        if let Some(ui) = root.optional("UI")? {
            ui.check(&[], &["License", "BrandingText", "FinishLaunch"], false)?;
            if let Some(n) = ui.optional("License")? {
                n.check(&["file"], &[], false)?;
                result.license = Some(n.attr("file")?.into());
            }
            if let Some(n) = ui.optional("BrandingText")? {
                n.check(&[], &[], true)?;
                result.branding = n.text.clone();
            }
            if let Some(n) = ui.optional("FinishLaunch")? {
                n.check(&["file"], &["Arguments"], false)?;
                let mut args = Vec::new();
                if let Some(a) = n.optional("Arguments")? {
                    a.check(&[], &["Argument"], false)?;
                    for v in &a.children {
                        v.check(&[], &[], true)?;
                        args.push(v.text.clone());
                    }
                }
                result.launch = Some(Launch {
                    file: PathBuf::from(n.attr("file")?),
                    arguments: args,
                });
            }
        }
        if let Some(registry) = root.optional("Registry")? {
            registry.check(&[], &["Entry"], false)?;
            for entry in &registry.children {
                entry.check(
                    &["id", "root", "key", "name", "type", "view", "uninstall"],
                    &["Data"],
                    false,
                )?;
                if entry.attrs.get("uninstall").is_some_and(|v| v != "remove") {
                    return Err(entry.error("only uninstall=remove is implemented"));
                }
                let kind = entry.attr("type")?.to_owned();
                let data = entry.required("Data")?;
                data.check(
                    &[],
                    if kind == "REG_MULTI_SZ" {
                        &["String"]
                    } else {
                        &[]
                    },
                    kind != "REG_MULTI_SZ",
                )?;
                let mut strings = Vec::new();
                for s in &data.children {
                    s.check(&[], &[], true)?;
                    strings.push(s.text.clone());
                }
                result.product.registry.push(crate::registry::Entry {
                    id: entry.attr("id")?.into(),
                    root: entry.attr("root")?.into(),
                    key: entry.attr("key")?.into(),
                    name: entry.attr("name")?.into(),
                    kind,
                    view: entry.attrs.get("view").cloned().unwrap_or("auto".into()),
                    text: if data.children.is_empty() {
                        data.text.clone()
                    } else {
                        String::new()
                    },
                    strings,
                });
            }
        }
        if let Some(shortcuts) = root.optional("Shortcuts")? {
            shortcuts.check(&[], &["Shortcut"], false)?;
            for e in &shortcuts.children {
                e.check(
                    &[
                        "id",
                        "location",
                        "name",
                        "target",
                        "workingDirectory",
                        "description",
                    ],
                    &["Arguments"],
                    false,
                )?;
                let mut arguments = Vec::new();
                if let Some(args) = e.optional("Arguments")? {
                    args.check(&[], &["Argument"], false)?;
                    for arg in &args.children {
                        arg.check(&[], &[], true)?;
                        arguments.push(arg.text.clone());
                    }
                }
                result.product.shortcuts.push(crate::shortcut::Entry {
                    id: e.attr("id")?.into(),
                    location: e.attr("location")?.into(),
                    name: e.attr("name")?.into(),
                    target: e.attr("target")?.into(),
                    working_directory: e
                        .attrs
                        .get("workingDirectory")
                        .map(PathBuf::from)
                        .unwrap_or(PathBuf::from(".")),
                    description: e.attrs.get("description").cloned().unwrap_or_default(),
                    arguments,
                });
            }
        }
        if let Some(actions) = root.optional("Actions")? {
            actions.check(&[], &["Install", "Uninstall"], false)?;
            for (phase, entries) in [
                ("Install", &mut result.product.actions.install),
                ("Uninstall", &mut result.product.actions.uninstall),
            ] {
                if let Some(list) = actions.optional(phase)? {
                    list.check(&[], &["Action"], false)?;
                    for a in &list.children {
                        a.check(
                            &[
                                "id",
                                "name",
                                "enabled",
                                "hideWindow",
                                "failurePolicy",
                                "timeoutSeconds",
                                "runOnUpgrade",
                            ],
                            &["Executable", "Arguments", "WorkingDirectory"],
                            false,
                        )?;
                        if a.attrs.get("runOnUpgrade").is_some_and(|v| v != "false") {
                            return Err(a.error("runOnUpgrade=true is not implemented yet"));
                        }
                        let exe = a.required("Executable")?;
                        exe.check(&["kind", "path"], &[], false)?;
                        let mut arguments = Vec::new();
                        if let Some(args) = a.optional("Arguments")? {
                            args.check(&[], &["Argument"], false)?;
                            for arg in &args.children {
                                arg.check(&[], &[], true)?;
                                arguments.push(arg.text.clone());
                            }
                        }
                        let directory = if let Some(d) = a.optional("WorkingDirectory")? {
                            d.check(&[], &[], true)?;
                            d.text.clone()
                        } else {
                            String::new()
                        };
                        let timeout = if let Some(value) = a.attrs.get("timeoutSeconds") {
                            let n = value.parse::<u64>()?;
                            if n == 0 {
                                return Err(
                                    a.error("timeoutSeconds must be 1..86400 when specified")
                                );
                            }
                            n
                        } else {
                            0
                        };
                        entries.push(crate::actions::Entry {
                            id: a.attr("id")?.into(),
                            name: a.attrs.get("name").cloned().unwrap_or(a.attr("id")?.into()),
                            enabled: boolean(
                                a.attrs.get("enabled").map(String::as_str).unwrap_or("true"),
                            )?,
                            hide: boolean(
                                a.attrs
                                    .get("hideWindow")
                                    .map(String::as_str)
                                    .unwrap_or("false"),
                            )?,
                            policy: a
                                .attrs
                                .get("failurePolicy")
                                .cloned()
                                .unwrap_or("stop".into()),
                            timeout,
                            kind: exe.attr("kind")?.into(),
                            path: exe.attr("path")?.into(),
                            arguments,
                            directory,
                        });
                    }
                }
            }
        }
        if let Some(upgrade) = root.optional("Upgrade")? {
            upgrade.check(&["enabled"], &[], false)?;
            result.product.upgrade = boolean(upgrade.attr("enabled")?)?;
        }
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        self.product.validate()?;
        if self
            .cab_limit
            .is_some_and(|n| n % (1024 * 1024) != 0 || !(1..=2047).contains(&(n / (1024 * 1024))))
        {
            return Err("CAB size must be an integer 1..2047 MiB".into());
        }
        for value in [&self.source, &self.output_directory] {
            if value.trim().is_empty() || value.contains('\0') {
                return Err("source/output path must not be empty".into());
            }
        }
        if self.license.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err("license path is empty".into());
        }
        let p = Path::new(&self.output_file);
        crate::payload::msi_name(p)?;
        if p.components().count() != 1
            || !matches!(p.components().next(), Some(std::path::Component::Normal(_)))
            || !p.extension().is_some_and(|s| s.eq_ignore_ascii_case("msi"))
            || self.output_file.contains(['\\', '/', ':'])
        {
            return Err("outputFile must be a plain .msi filename".into());
        }
        if self.branding.contains(['\0', '\n', '\r']) {
            return Err("BrandingText must be a single line".into());
        }
        if let Some(l) = &self.launch {
            l.target()?;
        }
        Ok(())
    }
    pub fn build(&self, path: &Path) -> Result<PathBuf> {
        self.validate()?;
        let absolute = fs::canonicalize(path)?;
        let base = absolute.parent().ok_or("project has no parent")?;
        let source = base.join(&self.source);
        if !source.is_dir() {
            return Err(format!("Source.directory does not exist: {}", source.display()).into());
        }
        let output = base.join(&self.output_directory).join(&self.output_file);
        let license = self.license.as_ref().map(|p| base.join(p));
        package::build_configured(&package::BuildSpec {
            source: &source,
            output: &output,
            license: license.as_deref(),
            branding: &self.branding,
            storage: self.storage,
            cab_limit: self.cab_limit,
            launch: self.launch.as_ref(),
            product: Some(&self.product),
        })?;
        Ok(output)
    }
    pub fn xml(&self) -> Result<Vec<u8>> {
        use xml::writer::{EmitterConfig, XmlEvent as E};
        self.validate()?;
        let mut bytes = Vec::new();
        {
            let mut w = EmitterConfig::new()
                .perform_indent(true)
                .create_writer(&mut bytes);
            w.write(E::start_element("ExpressMsiBuilderProject").attr("schemaVersion", "1"))?;
            let p = &self.product;
            w.write(
                E::start_element("Product")
                    .attr("name", &p.name)
                    .attr("manufacturer", &p.manufacturer)
                    .attr("version", &p.version)
                    .attr("productCode", &p.product_code)
                    .attr("upgradeCode", &p.upgrade_code)
                    .attr("architecture", &p.architecture)
                    .attr("scope", "perMachine"),
            )?;
            w.write(E::end_element())?;
            w.write(E::start_element("Source").attr("directory", &self.source))?;
            w.write(E::end_element())?;
            w.write(
                E::start_element("Installation")
                    .attr("directory", &p.installation)
                    .attr("existingFiles", p.existing_files.name())
                    .attr(
                        "allowDirectoryChange",
                        if p.allow_directory_change {
                            "true"
                        } else {
                            "false"
                        },
                    ),
            )?;
            w.write(E::end_element())?;
            if !p.registry.is_empty() {
                w.write(E::start_element("Registry"))?;
                for e in &p.registry {
                    w.write(
                        E::start_element("Entry")
                            .attr("id", &e.id)
                            .attr("root", &e.root)
                            .attr("key", &e.key)
                            .attr("name", &e.name)
                            .attr("type", &e.kind)
                            .attr("view", &e.view)
                            .attr("uninstall", "remove"),
                    )?;
                    w.write(E::start_element("Data"))?;
                    if e.kind == "REG_MULTI_SZ" {
                        for s in &e.strings {
                            w.write(E::start_element("String"))?;
                            w.write(E::characters(s))?;
                            w.write(E::end_element())?;
                        }
                    } else {
                        w.write(E::characters(&e.text))?;
                    }
                    w.write(E::end_element())?;
                    w.write(E::end_element())?;
                }
                w.write(E::end_element())?;
            }
            if !p.shortcuts.is_empty() {
                w.write(E::start_element("Shortcuts"))?;
                for e in &p.shortcuts {
                    w.write(
                        E::start_element("Shortcut")
                            .attr("id", &e.id)
                            .attr("location", &e.location)
                            .attr("name", &e.name)
                            .attr(
                                "target",
                                e.target.to_str().ok_or("invalid shortcut target")?,
                            )
                            .attr(
                                "workingDirectory",
                                e.working_directory
                                    .to_str()
                                    .ok_or("invalid shortcut working directory")?,
                            )
                            .attr("description", &e.description),
                    )?;
                    w.write(E::start_element("Arguments"))?;
                    for arg in &e.arguments {
                        w.write(E::start_element("Argument"))?;
                        w.write(E::characters(arg))?;
                        w.write(E::end_element())?;
                    }
                    w.write(E::end_element())?;
                    w.write(E::end_element())?;
                }
                w.write(E::end_element())?;
            }
            if !p.actions.install.is_empty() || !p.actions.uninstall.is_empty() {
                w.write(E::start_element("Actions"))?;
                for (phase, entries) in [
                    ("Install", &p.actions.install),
                    ("Uninstall", &p.actions.uninstall),
                ] {
                    w.write(E::start_element(phase))?;
                    for a in entries {
                        let timeout = a.timeout.to_string();
                        let mut start = E::start_element("Action")
                            .attr("id", &a.id)
                            .attr("name", &a.name)
                            .attr("enabled", if a.enabled { "true" } else { "false" })
                            .attr("hideWindow", if a.hide { "true" } else { "false" })
                            .attr("failurePolicy", &a.policy)
                            .attr("runOnUpgrade", "false");
                        if a.timeout > 0 {
                            start = start.attr("timeoutSeconds", &timeout);
                        }
                        w.write(start)?;
                        w.write(
                            E::start_element("Executable")
                                .attr("kind", &a.kind)
                                .attr("path", &a.path),
                        )?;
                        w.write(E::end_element())?;
                        w.write(E::start_element("Arguments"))?;
                        for arg in &a.arguments {
                            w.write(E::start_element("Argument"))?;
                            w.write(E::characters(arg))?;
                            w.write(E::end_element())?;
                        }
                        w.write(E::end_element())?;
                        if !a.directory.is_empty() {
                            w.write(E::start_element("WorkingDirectory"))?;
                            w.write(E::characters(&a.directory))?;
                            w.write(E::end_element())?;
                        }
                        w.write(E::end_element())?;
                    }
                    w.write(E::end_element())?;
                }
                w.write(E::end_element())?;
            }
            w.write(
                E::start_element("Upgrade")
                    .attr("enabled", if p.upgrade { "true" } else { "false" }),
            )?;
            w.write(E::end_element())?;
            w.write(E::start_element("UI"))?;
            if let Some(file) = &self.license {
                w.write(E::start_element("License").attr("file", file))?;
                w.write(E::end_element())?;
            }
            w.write(E::start_element("BrandingText"))?;
            w.write(E::characters(&self.branding))?;
            w.write(E::end_element())?;
            if let Some(l) = &self.launch {
                w.write(
                    E::start_element("FinishLaunch")
                        .attr("file", l.file.to_str().ok_or("invalid launch path")?),
                )?;
                w.write(E::start_element("Arguments"))?;
                for arg in &l.arguments {
                    w.write(E::start_element("Argument"))?;
                    w.write(E::characters(arg))?;
                    w.write(E::end_element())?;
                }
                w.write(E::end_element())?;
                w.write(E::end_element())?;
            }
            w.write(E::end_element())?;
            w.write(
                E::start_element("Build")
                    .attr("outputDirectory", &self.output_directory)
                    .attr("outputFile", &self.output_file),
            )?;
            let mut c = E::start_element("Cabinet")
                .attr(
                    "storage",
                    if self.storage == CabStorage::Embedded {
                        "embedded"
                    } else {
                        "external"
                    },
                )
                .attr(
                    "split",
                    if self.cab_limit.is_some() {
                        "size"
                    } else {
                        "none"
                    },
                );
            let size = self.cab_limit.map(|n| (n / (1024 * 1024)).to_string());
            if let Some(n) = &size {
                c = c.attr("targetSizeMiB", n);
            }
            w.write(c)?;
            w.write(E::end_element())?;
            w.write(E::end_element())?;
            w.write(E::end_element())?;
        }
        Ok(bytes)
    }
    pub fn save_new(&self, path: &Path) -> Result<()> {
        let bytes = self.xml()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".emb-save-{}.tmp", msi::guid()?));
        let operation = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::hard_link(&temp, path)?;
            Ok(())
        })();
        let _ = fs::remove_file(&temp);
        operation
    }
}
