use crate::{msi::Result, payload};
use std::path::{Component, PathBuf};

pub struct Launch {
    pub file: PathBuf,
    pub arguments: Vec<String>,
}
impl Launch {
    pub fn resolve<'a>(&self, files: &'a [payload::File]) -> Result<&'a payload::File> {
        if self.file.as_os_str().is_empty()
            || self
                .file
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(
                "launch file must be a relative installed .exe path without . or ..".into(),
            );
        }
        if !self
            .file
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
        {
            return Err("launch file must be an installed .exe".into());
        }
        let key = payload::key(&payload::target_path(
            self.file.to_str().ok_or("invalid launch target")?,
        ))?;
        files
            .iter()
            .find(|f| payload::key(&f.relative).is_ok_and(|k| k == key))
            .ok_or_else(|| "launch file is not included in the payload".into())
    }
    pub fn target(&self) -> Result<String> {
        let mut args = Vec::new();
        for value in &self.arguments {
            if value.contains('\0') {
                return Err("launch arguments must not contain NUL".into());
            }
            args.push(quote(value));
        }
        let literal = args.join(" ");
        let mut formatted = String::new();
        for c in literal.chars() {
            match c {
                '[' => formatted.push_str("[\\[]"),
                ']' => formatted.push_str("[\\]]"),
                _ => formatted.push(c),
            }
        }
        if formatted.encode_utf16().count() > 255 {
            return Err(
                "encoded launch arguments exceed the prototype's 255-character limit".into(),
            );
        }
        Ok(formatted)
    }
}
// Windows CRT argument quoting: preserve empty values, quotes and trailing slashes.
fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        if c == '"' {
            out.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            out.push_str(&"\\".repeat(slashes));
        }
        out.push(c);
        slashes = 0;
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argument_quotes_and_msi_literals_are_preserved() -> Result<()> {
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("C:\\data\\"), "\"C:\\data\\\\\"");
        let launch = Launch {
            file: "app.exe".into(),
            arguments: vec!["[ProductName]".into(), "한글".into()],
        };
        assert_eq!(launch.target()?, "\"[\\[]ProductName[\\]]\" \"한글\"");
        Ok(())
    }
}
