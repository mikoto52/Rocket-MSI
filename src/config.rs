use crate::msi::Result;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExistingFiles {
    #[default]
    Default,
    Skip,
    Overwrite,
}
impl ExistingFiles {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "default" => Ok(Self::Default),
            "skip" => Ok(Self::Skip),
            "overwrite" => Ok(Self::Overwrite),
            _ => Err("Installation.existingFiles must be default, skip, or overwrite".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Skip => "skip",
            Self::Overwrite => "overwrite",
        }
    }
    pub fn mode(self) -> &'static str {
        match self {
            Self::Default => "omus",
            Self::Skip => "pmus",
            Self::Overwrite => "amus",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Product {
    pub name: String,
    pub manufacturer: String,
    pub version: String,
    pub product_code: String,
    pub upgrade_code: String,
    pub architecture: String,
    pub installation: String,
    pub allow_directory_change: bool,
    pub existing_files: ExistingFiles,
    pub registry: Vec<crate::registry::Entry>,
    pub shortcuts: Vec<crate::shortcut::Entry>,
    pub actions: crate::actions::Actions,
    pub upgrade: bool,
}
impl Default for Product {
    fn default() -> Self {
        Self {
            name: "Express MSI Builder 설치 테스트".into(),
            manufacturer: "Express MSI Builder".into(),
            version: "0.1.0".into(),
            product_code: "{9DB58DC8-21CE-45E7-9311-8932DC040001}".into(),
            upgrade_code: "{9DB58DC8-21CE-45E7-9311-8932DC040002}".into(),
            architecture: "x64".into(),
            installation: "{ProgramFiles}\\Express MSI Builder Demo".into(),
            allow_directory_change: true,
            existing_files: ExistingFiles::Default,
            registry: Vec::new(),
            shortcuts: Vec::new(),
            actions: crate::actions::Actions::default(),
            upgrade: false,
        }
    }
}
pub fn guid(value: &str) -> Result<String> {
    let s = value
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(value);
    if s.len() != 36
        || !s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
        || s.chars().all(|c| c == '0' || c == '-')
    {
        return Err("invalid or zero GUID".into());
    }
    Ok(format!("{{{}}}", s.to_ascii_uppercase()))
}
impl Product {
    pub fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("Product.name", &self.name),
            ("Product.manufacturer", &self.manufacturer),
        ] {
            if value.trim().is_empty()
                || value.chars().any(char::is_control)
                || value.encode_utf16().count() > 128
            {
                return Err(format!("{field}: empty, control characters, or too long").into());
            }
        }
        let parts = self
            .version
            .split('.')
            .map(str::parse::<u32>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if parts.len() != 3 || parts[0] > 255 || parts[1] > 255 || parts[2] > 65535 {
            return Err("Product.version must be major.minor.build (255.255.65535 maximum)".into());
        }
        let p = guid(&self.product_code)?;
        let u = guid(&self.upgrade_code)?;
        if p == u {
            return Err("ProductCode and UpgradeCode must differ".into());
        }
        if !matches!(self.architecture.as_str(), "x86" | "x64") {
            return Err("architecture must be x86 or x64".into());
        }
        self.install_parts()?;
        crate::registry::validate(&self.registry, self)?;
        crate::shortcut::validate(&self.shortcuts, self)?;
        self.actions.validate(self)?;
        Ok(())
    }
    pub fn install_parts(&self) -> Result<Vec<String>> {
        let rest = self
            .installation
            .strip_prefix("{ProgramFiles}\\")
            .ok_or("Installation.directory must start with {ProgramFiles}\\")?;
        let mut out = Vec::new();
        for part in rest.split('\\') {
            let value = part
                .replace("{Manufacturer}", &self.manufacturer)
                .replace("{ProductName}", &self.name)
                .replace("{ProductVersion}", &self.version);
            if value.is_empty()
                || matches!(value.as_str(), "." | "..")
                || value.contains(['{', '}', '\\', '/'])
            {
                return Err("unsupported installation variable or directory segment".into());
            }
            crate::payload::msi_name(std::path::Path::new(&value))?;
            out.push(value);
        }
        Ok(out)
    }
}
