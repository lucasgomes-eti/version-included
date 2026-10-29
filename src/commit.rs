/// A commit as read from `git log`: its hash and full raw message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub message: String,
}

impl Commit {
    pub fn new(hash: impl Into<String>, message: impl Into<String>) -> Self {
        Commit {
            hash: hash.into(),
            message: message.into(),
        }
    }

    /// The first line of the message.
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("").trim_end()
    }
}

/// A parsed `<type>(<scope>)!: <subject>` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header<'a> {
    pub kind: &'a str,
    pub scope: Option<&'a str>,
    pub breaking: bool,
    pub subject: &'a str,
}

/// Parses the first line of a commit message as a Karma-style header.
///
/// Returns `None` when the line doesn't follow the format: the type must be
/// non-empty and made of ASCII letters, digits, `-` or `_`; the optional
/// scope sits in parentheses; an optional `!` follows; then `: ` and a
/// non-empty subject.
pub fn parse_header(line: &str) -> Option<Header<'_>> {
    let line = line.trim_end();
    let kind_end = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(line.len());
    let (kind, mut rest) = line.split_at(kind_end);
    if kind.is_empty() {
        return None;
    }

    let mut scope = None;
    if let Some(after_paren) = rest.strip_prefix('(') {
        let close = after_paren.find(')')?;
        let inner = &after_paren[..close];
        if inner.contains('(') {
            return None;
        }
        scope = Some(inner);
        rest = &after_paren[close + 1..];
    }

    let breaking = match rest.strip_prefix('!') {
        Some(after_bang) => {
            rest = after_bang;
            true
        }
        None => false,
    };

    let subject = rest.strip_prefix(": ")?.trim();
    if subject.is_empty() {
        return None;
    }

    Some(Header {
        kind,
        scope,
        breaking,
        subject,
    })
}

/// The footer token that marks a breaking change.
pub const BREAKING_FOOTER: &str = "BREAKING CHANGE:";

/// Whether any line after the header starts with `BREAKING CHANGE:`.
///
/// This is checked independently of the header, so it also applies to
/// commits whose header doesn't follow the format.
pub fn has_breaking_footer(message: &str) -> bool {
    message
        .lines()
        .skip(1)
        .any(|line| line.trim_start().starts_with(BREAKING_FOOTER))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(
        kind: &'static str,
        scope: Option<&'static str>,
        breaking: bool,
        subject: &'static str,
    ) -> Header<'static> {
        Header {
            kind,
            scope,
            breaking,
            subject,
        }
    }

    #[test]
    fn parses_type_and_subject() {
        assert_eq!(
            parse_header("feat: add login"),
            Some(header("feat", None, false, "add login"))
        );
    }

    #[test]
    fn parses_scope() {
        assert_eq!(
            parse_header("fix(middleware): handle ranges"),
            Some(header("fix", Some("middleware"), false, "handle ranges"))
        );
    }

    #[test]
    fn parses_empty_scope() {
        assert_eq!(
            parse_header("fix(): x"),
            Some(header("fix", Some(""), false, "x"))
        );
    }

    #[test]
    fn parses_bang_without_scope() {
        assert_eq!(
            parse_header("feat!: drop v1 api"),
            Some(header("feat", None, true, "drop v1 api"))
        );
    }

    #[test]
    fn parses_bang_after_scope() {
        assert_eq!(
            parse_header("feat(api)!: drop v1"),
            Some(header("feat", Some("api"), true, "drop v1"))
        );
    }

    #[test]
    fn allows_dash_underscore_and_digits_in_type() {
        assert_eq!(parse_header("dep_s-2: x").unwrap().kind, "dep_s-2");
    }

    #[test]
    fn trims_trailing_whitespace() {
        assert_eq!(parse_header("fix: x  \r").unwrap().subject, "x");
    }

    #[test]
    fn keeps_type_case() {
        assert_eq!(parse_header("Feat: x").unwrap().kind, "Feat");
    }

    #[test]
    fn rejects_non_matching_headers() {
        for line in [
            "",
            "update readme",
            ": no type",
            "feat:no space",
            "feat:",
            "feat: ",
            "feat :x",
            "feat(scope: x",
            "feat(a(b)): x",
            "feat(scope) : x",
            "feat!!: x",
            "fe at: x",
            "Merge branch 'main'",
        ] {
            assert_eq!(parse_header(line), None, "{line:?}");
        }
    }

    #[test]
    fn breaking_footer_in_body() {
        assert!(has_breaking_footer(
            "feat: x\n\nbody\n\nBREAKING CHANGE: removed y"
        ));
        assert!(has_breaking_footer(
            "feat: x\n\nBREAKING CHANGE:\n\nexplain"
        ));
        assert!(has_breaking_footer(
            "chore: x\n\n  BREAKING CHANGE: indented"
        ));
    }

    #[test]
    fn breaking_footer_ignores_header_and_lookalikes() {
        assert!(!has_breaking_footer("BREAKING CHANGE: in header only"));
        assert!(!has_breaking_footer("feat: x\n\nbreaking change: lower"));
        assert!(!has_breaking_footer("feat: x\n\nBREAKING CHANGES: plural"));
        assert!(!has_breaking_footer(
            "feat: x\n\nnot a BREAKING CHANGE: mid-line"
        ));
        assert!(!has_breaking_footer("feat: x"));
    }

    #[test]
    fn subject_is_first_line() {
        assert_eq!(Commit::new("a", "feat: x \nbody").subject(), "feat: x");
        assert_eq!(Commit::new("a", "").subject(), "");
    }
}
