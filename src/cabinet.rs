use crate::{msi::Result, payload::File};
use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
    process::Command,
};

pub struct Cabinet {
    pub name: String,
    pub files: Range<usize>,
}
pub fn create(
    work: &Path,
    files: &[File],
    sizes: &[i32],
    limit: Option<u64>,
    embedded: bool,
) -> Result<Vec<Cabinet>> {
    let mut groups = Vec::new();
    let mut start = 0;
    let mut bytes = 0u64;
    for (index, &size) in sizes.iter().enumerate() {
        let size = size as u64;
        if limit.is_some_and(|n| size > n) {
            return Err(format!(
                "file exceeds CAB size limit; spanning is not supported: {}",
                files[index].relative.display()
            )
            .into());
        }
        if index > start && limit.is_some_and(|n| bytes + size > n) {
            groups.push(start..index);
            start = index;
            bytes = 0;
        }
        bytes += size;
    }
    groups.push(start..files.len());
    let system = std::env::var_os("SystemRoot").ok_or("SystemRoot missing")?;
    let makecab = PathBuf::from(system).join("System32/makecab.exe");
    let mut result = Vec::new();
    let mut index = 0;
    while index < groups.len() {
        // Data9999 is the last name that fits MSI's external 8.3 CAB syntax.
        if groups.len() > 9999 {
            return Err("too many cabinets".into());
        }
        let range = groups[index].clone();
        let name = if embedded && limit.is_none() {
            "payload.cab".to_owned()
        } else {
            format!("Data{}.cab", index + 1)
        };
        let mut ddf = format!(
            ".OPTION EXPLICIT\r\n.Set Cabinet=ON\r\n.Set Compress=ON\r\n.Set CompressionType=MSZIP\r\n.Set CabinetNameTemplate={name}\r\n.Set DiskDirectoryTemplate=.\r\n.Set MaxDiskSize=0\r\n"
        );
        for file in &files[range.clone()] {
            ddf.push_str(&format!("\"{}\" \"{}\"\r\n", file.id, file.id));
        }
        fs::write(work.join("payload.ddf"), ddf)?;
        let output = Command::new(&makecab)
            .current_dir(work)
            .args(["/f", "payload.ddf"])
            .output()?;
        let path = work.join(&name);
        if !output.status.success() || !path.is_file() {
            return Err(format!(
                "makecab failed for {name}: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        if limit.is_some_and(|n| path.metadata().is_ok_and(|m| m.len() > n)) {
            fs::remove_file(&path)?;
            if range.len() == 1 {
                return Err(format!(
                    "CAB overhead/compression exceeds size limit for {}; increase the limit",
                    files[range.start].relative.display()
                )
                .into());
            }
            let middle = range.start + range.len() / 2;
            groups[index] = range.start..middle;
            groups.insert(index + 1, middle..range.end);
            continue;
        }
        result.push(Cabinet { name, files: range });
        index += 1;
    }
    Ok(result)
}
