//! Validate a bounded Windows ICO file before adding it to an MSI.
use std::{ffi::c_void, fs, os::windows::ffi::OsStrExt, path::Path};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[link(name = "user32")]
unsafe extern "system" {
    fn LoadImageW(
        instance: *mut c_void,
        name: *const u16,
        kind: u32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> *mut c_void;
    fn DestroyIcon(icon: *mut c_void) -> i32;
}
pub fn validate(path: &Path) -> Result<()> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ico"))
    {
        return Err("Quick MSI icon must be an .ico file".into());
    }
    let size = fs::metadata(path)?.len();
    if !(22..=10 * 1024 * 1024).contains(&size) {
        return Err("Quick MSI icon must be a valid ICO file no larger than 10 MiB".into());
    }
    let bytes = fs::read(path)?;
    validate_directory(&bytes)?;
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if name[..name.len() - 1].contains(&0) {
        return Err("icon path contains NUL".into());
    }
    let icon = unsafe { LoadImageW(std::ptr::null_mut(), name.as_ptr(), 1, 96, 96, 0x10) };
    if icon.is_null() {
        return Err(format!("Windows cannot decode Quick MSI icon: {}", path.display()).into());
    }
    unsafe {
        DestroyIcon(icon);
    }
    Ok(())
}
fn validate_directory(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 6 || bytes[..4] != [0, 0, 1, 0] {
        return Err("invalid ICO header".into());
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let end = 6 + count * 16;
    if count == 0 || end > bytes.len() {
        return Err("invalid ICO directory".into());
    }
    for entry in bytes[6..end].as_chunks::<16>().0 {
        let size = u32::from_le_bytes(entry[8..12].try_into()?) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into()?) as usize;
        if size == 0 || offset < end || offset.checked_add(size).is_none_or(|end| end > bytes.len())
        {
            return Err("ICO image data is out of bounds".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_headers_truncated_directories_and_out_of_bounds_images() {
        let valid = include_bytes!("../assets/installer/app-icon.ico");
        assert!(validate_directory(valid).is_ok());
        for len in [0, 5, 6, 21] {
            assert!(validate_directory(&valid[..len]).is_err());
        }
        let mut invalid = valid.to_vec();
        invalid[2] = 2;
        assert!(validate_directory(&invalid).is_err());
        invalid = valid.to_vec();
        invalid[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_directory(&invalid).is_err());
        assert!(validate(Path::new("assets/installer/app-icon.ico")).is_ok());
    }
}
