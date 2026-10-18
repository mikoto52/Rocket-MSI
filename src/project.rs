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
            &["Product", "Source", "Installation", "UI", "Build"],
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
        installation.check(&["directory", "allowDirectoryChange"], &[], false)?;
        let product = Product {
            name: p.attr("name")?.into(),
            manufacturer: p.attr("manufacturer")?.into(),
            version: p.attr("version")?.into(),
            product_code: crate::config::guid(p.attr("productCode")?)?,
            upgrade_code: crate::config::guid(p.attr("upgradeCode")?)?,
            architecture: p.attr("architecture")?.into(),
            installation: installation.attr("directory")?.into(),
            allow_directory_change: boolean(installation.attr("allowDirectoryChange")?)?,
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
