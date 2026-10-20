//! Build-time Authenticode signing. Secrets are never part of the project model.
use crate::msi::Result;
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Signing {
    pub source: String,
    pub pfx_file: String,
    pub thumbprint: String,
    pub store_location: String,
    pub timestamp_url: String,
}
impl Default for Signing {
    fn default() -> Self {
        Self {
            source: "pfx".into(),
            pfx_file: String::new(),
            thumbprint: String::new(),
            store_location: "currentUser".into(),
            timestamp_url: String::new(),
        }
    }
}
impl Signing {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.source.as_str(), "pfx" | "store") {
            return Err("Signing.source must be pfx or store".into());
        }
        if !matches!(self.store_location.as_str(), "currentUser" | "localMachine") {
            return Err("Signing.storeLocation must be currentUser or localMachine".into());
        }
        if self.source == "pfx" && (self.pfx_file.trim().is_empty() || self.pfx_file.contains('\0'))
        {
            return Err("Signing.pfxFile is required".into());
        }
        if self.source == "store"
            && (self.thumbprint.len() != 40
                || !self.thumbprint.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(
                "Signing.thumbprint must be a 40-digit SHA-1 certificate identifier".into(),
            );
        }
        if !self.timestamp_url.is_empty()
            && (!(self.timestamp_url.starts_with("https://")
                || self.timestamp_url.starts_with("http://"))
                || self
                    .timestamp_url
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control()))
        {
            return Err("Signing.timestampUrl must be an HTTP(S) URL without whitespace".into());
        }
        Ok(())
    }
}
pub fn tool() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ROCKET_MSI_SIGNTOOL") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err("ROCKET_MSI_SIGNTOOL does not identify an existing signtool.exe".into());
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let path = dir.join("signtool.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    if let Some(root) = std::env::var_os("ProgramFiles(x86)") {
        let bin = PathBuf::from(root).join("Windows Kits/10/bin");
        let mut versions: Vec<_> = fs::read_dir(bin)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        versions.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
        for version in versions {
            for arch in ["x64", "x86"] {
                let path = version.path().join(arch).join("signtool.exe");
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }
    Err("MSI signing requires Windows SDK SignTool (or ROCKET_MSI_SIGNTOOL)".into())
}
fn run(command: &mut Command, operation: &str) -> Result<()> {
    // No arguments, password, or signer output are included in diagnostics.
    let mut child = command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| format!("{operation}: could not start SignTool"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("{operation} failed (SignTool exit {}). Check the certificate, private key, password, timestamp server, and Windows trust chain.", status.code().unwrap_or(-1)).into())
            };
        }
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{operation} timed out after 120 seconds").into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}
pub fn sign(file: &Path, config: &Signing, base: &Path, password: Option<&str>) -> Result<()> {
    config.validate()?;
    let tool = tool()?;
    let mut command = Command::new(&tool);
    command.args(["sign", "/fd", "SHA256"]);
    if config.source == "pfx" {
        let path = base.join(&config.pfx_file);
        if !path.is_file() {
            return Err("Signing PFX file does not exist".into());
        }
        command.arg("/f").arg(path);
        if let Some(password) = password.filter(|p| !p.is_empty()) {
            command.arg("/p").arg(password);
        }
    } else {
        command.args(["/s", "My", "/sha1", &config.thumbprint]);
        if config.store_location == "localMachine" {
            command.arg("/sm");
        }
    }
    if !config.timestamp_url.is_empty() {
        command.args(["/tr", &config.timestamp_url, "/td", "SHA256"]);
    }
    command.arg(file);
    run(&mut command, "MSI signing")?;
    verify_with_tool(&tool, file, !config.timestamp_url.is_empty())
}
fn verify_with_tool(tool: &Path, file: &Path, timestamp: bool) -> Result<()> {
    let mut command = Command::new(tool);
    command.args(["verify", "/pa", "/all"]);
    if timestamp {
        command.arg("/tw");
    }
    command.arg(file);
    run(&mut command, "MSI signature verification")
}
pub fn verify(file: &Path) -> Result<()> {
    verify_with_tool(&tool()?, file, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signing_rejects_invalid_sources_certificates_and_urls() {
        let mut c = Signing::default();
        assert!(c.validate().is_err());
        c.pfx_file = "certs/code signing.pfx".into();
        assert!(c.validate().is_ok());
        c.timestamp_url = "file:///tmp/timestamp".into();
        assert!(c.validate().is_err());
        c.timestamp_url = "https://timestamp.example.test".into();
        assert!(c.validate().is_ok());
        c.source = "store".into();
        assert!(c.validate().is_err());
        c.thumbprint = "A1".repeat(20);
        assert!(c.validate().is_ok());
        c.store_location = "other".into();
        assert!(c.validate().is_err());
    }
    #[test]
    fn verification_rejects_unsigned_msi() -> Result<()> {
        let file = std::env::current_dir()?
            .join("target")
            .join(format!("unsigned-{}.msi", crate::msi::guid()?));
        fs::write(&file, b"not a signed MSI")?;
        let result = verify(&file);
        fs::remove_file(file)?;
        assert!(result.is_err());
        Ok(())
    }
}
