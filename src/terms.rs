//! URL policy shared by the project loader and the standalone embedded UI.
pub fn valid_url(value: &str) -> bool {
    if value.len() > 2048
        || value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\\' | '"' | '<' | '>'))
    {
        return false;
    }
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty() && !authority.contains('@') && !authority.starts_with(':')
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terms_links_allow_only_explicit_web_urls() {
        for url in [
            "https://example.com/terms?app=demo&lang=ko",
            "http://localhost:8080/license",
            "HTTPS://예시.한국/약관",
            "https://[::1]/terms",
        ] {
            assert!(valid_url(url), "{url}");
        }
        for url in [
            "",
            "javascript:alert(1)",
            "file:///C:/terms.txt",
            "https://",
            "https:///terms",
            "https://user:pass@example.com",
            "https://example.com/\n",
            "https://example.com/ a",
            "https://example.com\\x",
            "ms-settings:privacy",
        ] {
            assert!(!valid_url(url), "{url}");
        }
    }
}
