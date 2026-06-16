/// Returns true if `text` matches a glob `pattern` containing `*` wildcards.
///
/// Each `*` matches zero or more characters. All other characters are compared
/// literally (case-sensitive). Examples: `"org.junit:*"` matches any artifact
/// starting with `"org.junit:"`, `"*:core"` matches any artifact ending with
/// `":core"`, `"*"` matches everything.
pub(super) fn glob_matches(pattern: &str, text: &str) -> bool {
    let segments: Vec<&str> = pattern.split('*').collect();
    if segments.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }
        if i == 0 {
            if !text.starts_with(segment) {
                return false;
            }
            pos = segment.len();
        } else if i == segments.len() - 1 {
            if !text[pos..].ends_with(segment) {
                return false;
            }
        } else {
            match text[pos..].find(segment) {
                Some(offset) => pos += offset + segment.len(),
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_exact_match() {
        assert!(glob_matches("org.junit:junit", "org.junit:junit"));
        assert!(!glob_matches("org.junit:junit", "org.junit:junit-bom"));
    }

    #[test]
    fn glob_trailing_wildcard() {
        assert!(glob_matches("org.junit:*", "org.junit:junit"));
        assert!(glob_matches("org.junit:*", "org.junit:junit-bom"));
        assert!(!glob_matches("org.junit:*", "org.mockito:mockito-core"));
    }

    #[test]
    fn glob_leading_wildcard() {
        assert!(glob_matches("*:core", "org.example:core"));
        assert!(!glob_matches("*:core", "org.example:core-api"));
    }

    #[test]
    fn glob_both_wildcards() {
        assert!(glob_matches("*:junit*", "org.junit:junit"));
        assert!(glob_matches("*:junit*", "org.junit:junit-bom"));
        assert!(!glob_matches("*:junit*", "org.junit:mockito"));
    }

    #[test]
    fn glob_match_all() {
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("*:*", "org.junit:junit"));
    }

    #[test]
    fn glob_middle_wildcard() {
        assert!(glob_matches("org.*:core", "org.example:core"));
        assert!(glob_matches("org.*:core", "org.wildfly:core"));
        assert!(!glob_matches("org.*:core", "com.example:core"));
    }

    #[test]
    fn glob_npm_packages() {
        assert!(glob_matches("react*", "react"));
        assert!(glob_matches("react*", "react-dom"));
        assert!(!glob_matches("react*", "preact"));
        assert!(glob_matches("@scope/*", "@scope/utils"));
    }
}
