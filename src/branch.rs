/// Matches `text` against a glob `pattern` where `*` matches any sequence of
/// characters (including `/` and the empty sequence) and `?` matches exactly
/// one character. All other characters match themselves.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Position of the last `*` seen and the text position it was tried at,
    // so a failed match can backtrack by letting that `*` absorb one more char.
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        match pattern.get(p) {
            Some(&'*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// Decides the current branch from `git rev-parse --abbrev-ref HEAD` output
/// and the `GITHUB_HEAD_REF` environment variable.
///
/// A detached HEAD (reported as `HEAD`) falls back to `GITHUB_HEAD_REF`;
/// empty values count as unset. Returns `None` when neither names a branch.
pub fn resolve(abbrev_ref: Option<&str>, github_head_ref: Option<&str>) -> Option<String> {
    let non_empty = |s: &&str| !s.trim().is_empty();
    abbrev_ref
        .map(str::trim)
        .filter(non_empty)
        .filter(|name| *name != "HEAD")
        .or_else(|| github_head_ref.map(str::trim).filter(non_empty))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_patterns() {
        assert!(glob_match("main", "main"));
        assert!(!glob_match("main", "mainline"));
        assert!(!glob_match("main", "mai"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn star_matches_any_sequence_including_slashes() {
        assert!(glob_match("feature/*", "feature/login"));
        assert!(glob_match("feature/*", "feature/a/b"));
        assert!(glob_match("feature/*", "feature/"));
        assert!(!glob_match("feature/*", "feature"));
        assert!(!glob_match("feature/*", "bugfix/login"));
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything/at/all"));
        assert!(glob_match("**", "x"));
    }

    #[test]
    fn star_in_the_middle_backtracks() {
        assert!(glob_match("release-*-rc", "release-1.2-rc"));
        assert!(glob_match("a*b*c", "aXbYbZc"));
        assert!(glob_match("*-fix", "a-fix-fix"));
        assert!(!glob_match("a*b*c", "aXbYbZ"));
        assert!(!glob_match("*x", "abc"));
    }

    #[test]
    fn question_mark_matches_one_char() {
        assert!(glob_match("v?", "v1"));
        assert!(!glob_match("v?", "v"));
        assert!(!glob_match("v?", "v12"));
        assert!(glob_match("hotfix/?.?", "hotfix/1.2"));
        assert!(glob_match("ü?", "üñ"));
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert!(!glob_match("feature/*", "Feature/x"));
    }

    #[test]
    fn resolve_uses_attached_branch() {
        assert_eq!(
            resolve(Some("main"), Some("feature/x")),
            Some("main".into())
        );
        assert_eq!(resolve(Some("feature/x\n"), None), Some("feature/x".into()));
    }

    #[test]
    fn resolve_detached_head_falls_back_to_github_head_ref() {
        assert_eq!(
            resolve(Some("HEAD"), Some("feature/x")),
            Some("feature/x".into())
        );
        assert_eq!(resolve(None, Some("feature/x")), Some("feature/x".into()));
        assert_eq!(
            resolve(Some(""), Some("feature/x")),
            Some("feature/x".into())
        );
    }

    #[test]
    fn resolve_returns_none_without_a_branch() {
        assert_eq!(resolve(Some("HEAD"), None), None);
        assert_eq!(resolve(Some("HEAD"), Some("")), None);
        assert_eq!(resolve(Some("HEAD"), Some("  ")), None);
        assert_eq!(resolve(None, None), None);
    }
}
