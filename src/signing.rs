//! Build-time Authenticode signing. Secrets are never part of the project model.
use crate::msi::Result;
#[cfg(test)]
use std::fs;
use std::path::Path;
#[path = "signing_native.rs"]
mod native;

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
pub fn sign(file: &Path, config: &Signing, base: &Path, password: Option<&str>) -> Result<()> {
    config.validate()?;
    native::sign(file, config, base, password)?;
    native::verify(file)
}
pub fn verify(file: &Path) -> Result<()> {
    native::verify(file)
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
