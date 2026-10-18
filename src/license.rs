use crate::msi::Result;
use std::{fs, path::Path};

/// ScrollableText expects RTF. TXT is converted with explicit UTF-16 escapes.
pub fn load(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    if bytes.len() > 1024 * 1024 {
        return Err("license file must be at most 1 MiB".into());
    }
    let text = std::str::from_utf8(&bytes)?.trim_start_matches('\u{feff}');
    if text.trim().is_empty() || text.contains('\0') {
        return Err("license is empty or contains NUL".into());
    }
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("txt") => Ok(txt_to_rtf(text)),
        Some("rtf") => {
            if !text.starts_with("{\\rtf1") || !text.trim_end().ends_with('}') || !text.is_ascii() {
                return Err("RTF must be ASCII with a {\\rtf1 header; encode non-ASCII text with RTF Unicode/hex escapes".into());
            }
            Ok(text.to_owned())
        }
        _ => Err("license must be a UTF-8 .txt or ASCII .rtf file".into()),
    }
}
fn txt_to_rtf(text: &str) -> String {
    let mut rtf = String::from(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0 Malgun Gothic;}}\\f0\\fs18\\uc1 ",
    );
    for unit in text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .encode_utf16()
    {
        match unit {
            92 | 123 | 125 => {
                rtf.push('\\');
                rtf.push(char::from_u32(unit.into()).unwrap());
            }
            10 => rtf.push_str("\\par\n"),
            9 => rtf.push_str("\\tab "),
            32..=126 => rtf.push(char::from_u32(unit.into()).unwrap()),
            _ => rtf.push_str(&format!("\\u{}?", unit as i16)),
        }
    }
    rtf.push('}');
    rtf
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn txt_escapes_rtf_and_unicode_without_losing_lines() {
        let rtf = txt_to_rtf("가{a}\\\r\n\t😀");
        assert!(rtf.contains("\\u-21504?\\{a\\}\\\\\\par\n\\tab "));
        assert!(rtf.contains("\\u-10179?\\u-8704?"));
        assert!(rtf.is_ascii());
    }
}
