//! Line-oriented scanner that finds requirement-verifying tests in one Rust
//! source file.
//!
//! A verifying test is a `fn` preceded by a contiguous block of outer doc
//! comments (`///`) and outer attributes (`#[...]`, possibly spanning several
//! lines) where:
//!
//! - the block contains an attribute line that is exactly `#[test]`
//!   (optionally followed by a `//` comment), and
//! - at least one doc line in the block reads `/// Verifies: <IDS>`, where
//!   `<IDS>` is a comma-separated list of requirement IDs such as
//!   `REQ-FSM-010, REQ-FSM-011`.
//!
//! Other attributes (`#[should_panic]`, `#[cfg_attr(...)]`, ...) and other doc
//! lines may be interleaved in any order. The block is broken by anything
//! else: a blank line, an ordinary `//` comment, an inner attribute, or code.
//! The `fn` line may carry qualifiers (`pub`, `pub(crate)`, `async`, `const`,
//! `unsafe`, `extern "C"`).
//!
//! A `Verifies:` line whose block ends without reaching a `#[test]` fn is a
//! [`Problem::DanglingVerifies`]. A `Verifies:` line whose list does not parse
//! as IDs is a [`Problem::MalformedVerifies`]. Matching is purely textual: the
//! scanner does not understand string literals or block comments, which is
//! acceptable because the evidence convention is a whole doc line.

use super::{Problem, ReqId, ScanResult, SourceLoc, VerifyingTest};

/// A `Verifies:` line seen in the current block.
#[derive(Debug, Clone)]
struct Claim {
    line: usize,
    ids: Result<Vec<ReqId>, String>,
}

/// The doc/attribute block accumulated above a possible `fn`.
#[derive(Debug, Clone, Default)]
struct Block {
    claims: Vec<Claim>,
    has_test: bool,
    /// Unclosed `[` count of a multi-line attribute in progress.
    attr_depth: usize,
}

/// How a single line affects the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LineKind {
    /// `/// Verifies: ...`; carries the text after the colon.
    Verifies(String),
    /// Any other `///` line.
    Doc,
    /// A line starting `#[`; `is_test` when it is exactly `#[test]`, and
    /// `depth` is the unclosed bracket count at the end of the line.
    Attr { is_test: bool, depth: usize },
    /// A `fn` item line with its name.
    Fn(String),
    /// Anything else: ends the block.
    Other,
}

/// Scans `contents` (the text of the file at `path`, relative to the
/// workspace root) for verifying tests and `Verifies:` problems.
pub fn scan(path: &str, contents: &str) -> ScanResult {
    let (block, result) = contents.lines().enumerate().fold(
        (Block::default(), ScanResult::default()),
        |(block, result), (idx, text)| step(path, block, result, idx.saturating_add(1), text),
    );
    close_block(path, block, None, result)
}

/// Advances the scanner by one line, returning the new block and result.
fn step(
    path: &str,
    block: Block,
    result: ScanResult,
    line: usize,
    text: &str,
) -> (Block, ScanResult) {
    if block.attr_depth > 0 {
        let attr_depth = bracket_depth(block.attr_depth, text);
        return (
            Block {
                attr_depth,
                ..block
            },
            result,
        );
    }
    match classify(text) {
        LineKind::Verifies(list) => {
            let mut claims = block.claims;
            claims.push(Claim {
                line,
                ids: parse_ids(&list).ok_or(list),
            });
            (Block { claims, ..block }, result)
        }
        LineKind::Doc => (block, result),
        LineKind::Attr { is_test, depth } => (
            Block {
                has_test: block.has_test || is_test,
                attr_depth: depth,
                ..block
            },
            result,
        ),
        LineKind::Fn(name) => (
            Block::default(),
            close_block(path, block, Some((name, line)), result),
        ),
        LineKind::Other => (Block::default(), close_block(path, block, None, result)),
    }
}

/// Ends a block. If it reached a `fn` (`target`) and contains `#[test]`, its
/// claims become a [`VerifyingTest`]; otherwise every claim is dangling.
fn close_block(
    path: &str,
    block: Block,
    target: Option<(String, usize)>,
    result: ScanResult,
) -> ScanResult {
    if block.claims.is_empty() {
        return result;
    }
    let loc = |line: usize| SourceLoc {
        path: path.to_owned(),
        line,
    };
    match target {
        Some((name, fn_line)) if block.has_test => {
            let (ids, problems) = block.claims.into_iter().fold(
                (Vec::new(), Vec::new()),
                |(mut ids, mut problems), claim| {
                    match claim.ids {
                        Ok(found) => ids.extend(found),
                        Err(text) => problems.push(Problem::MalformedVerifies {
                            at: loc(claim.line),
                            text,
                        }),
                    }
                    (ids, problems)
                },
            );
            let tests = if ids.is_empty() {
                Vec::new()
            } else {
                vec![VerifyingTest {
                    path: path.to_owned(),
                    name,
                    line: fn_line,
                    ids,
                }]
            };
            result.merge(ScanResult { tests, problems })
        }
        _ => {
            let problems = block
                .claims
                .iter()
                .map(|c| Problem::DanglingVerifies { at: loc(c.line) })
                .collect();
            result.merge(ScanResult {
                tests: Vec::new(),
                problems,
            })
        }
    }
}

/// Classifies one source line.
fn classify(text: &str) -> LineKind {
    let trimmed = text.trim();
    if let Some(doc) = trimmed.strip_prefix("///") {
        // `////` is an ordinary comment in Rust, not a doc comment.
        if doc.starts_with('/') {
            return LineKind::Other;
        }
        return match doc.trim_start().strip_prefix("Verifies:") {
            Some(list) => LineKind::Verifies(list.trim().to_owned()),
            None => LineKind::Doc,
        };
    }
    if trimmed.starts_with("#[") {
        let code = trimmed
            .split_once("//")
            .map_or(trimmed, |(c, _)| c)
            .trim_end();
        return LineKind::Attr {
            is_test: code == "#[test]",
            depth: bracket_depth(0, trimmed),
        };
    }
    fn_name(trimmed).map_or(LineKind::Other, LineKind::Fn)
}

/// Returns the name if `trimmed` starts a `fn` item.
fn fn_name(trimmed: &str) -> Option<String> {
    let mut tokens = trimmed.split_whitespace();
    let fn_kw = tokens.by_ref().find(|t| !is_qualifier(t))?;
    if fn_kw != "fn" {
        return None;
    }
    let name: String = tokens
        .next()?
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Tokens that may precede `fn` in an item declaration.
fn is_qualifier(token: &str) -> bool {
    matches!(token, "pub" | "async" | "const" | "unsafe" | "extern")
        || token.starts_with("pub(")
        || token.starts_with('"')
}

/// Applies the `[`/`]` balance of `text` to `depth`.
fn bracket_depth(depth: usize, text: &str) -> usize {
    text.chars().fold(depth, |d, c| match c {
        '[' => d.saturating_add(1),
        ']' => d.saturating_sub(1),
        _ => d,
    })
}

/// Parses a comma-separated ID list; `None` if empty or any entry is invalid.
fn parse_ids(list: &str) -> Option<Vec<ReqId>> {
    let ids: Option<Vec<ReqId>> = list.split(',').map(|s| ReqId::parse(s.trim())).collect();
    ids.filter(|v| !v.is_empty())
}

#[cfg(test)]
// Test code: panicking on a failed expectation is the point of a test.
#[cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]
mod tests {
    use super::*;

    const PATH: &str = "middleware/src/fsm.rs";

    fn ids(test: &VerifyingTest) -> Vec<&str> {
        test.ids.iter().map(ReqId::as_str).collect()
    }

    fn dangling(line: usize) -> Problem {
        Problem::DanglingVerifies {
            at: SourceLoc {
                path: PATH.to_owned(),
                line,
            },
        }
    }

    #[test]
    fn single_id_test_is_found() {
        let src = "\
#[cfg(test)]
mod tests {
    /// Checks a thing.
    /// Verifies: REQ-TYP-001
    #[test]
    fn checks_thing() {}
}
";
        let r = scan(PATH, src);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(r.tests.len(), 1);
        assert_eq!(r.tests[0].name, "checks_thing");
        assert_eq!(r.tests[0].line, 6);
        assert_eq!(r.tests[0].path, PATH);
        assert_eq!(ids(&r.tests[0]), vec!["REQ-TYP-001"]);
    }

    #[test]
    fn multi_id_and_multiple_verifies_lines() {
        let src = "\
/// Verifies: REQ-FSM-010, REQ-FSM-011
///Verifies:REQ-FSM-012
#[test]
fn transitions() {}
";
        let r = scan(PATH, src);
        assert!(r.problems.is_empty());
        assert_eq!(
            ids(&r.tests[0]),
            vec!["REQ-FSM-010", "REQ-FSM-011", "REQ-FSM-012"]
        );
    }

    #[test]
    fn interleaved_attributes_and_qualifiers() {
        let src = "\
    #[should_panic(expected = \"boom\")]
    /// Verifies: REQ-A-1
    #[cfg_attr(miri, ignore)]
    #[test] // trailing comment
    /// More docs after the attribute.
    #[cfg_attr(
        feature = \"slow\",
        ignore
    )]
    pub(crate) async fn panics() {}
";
        let r = scan(PATH, src);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(r.tests[0].name, "panics");
        assert_eq!(r.tests[0].line, 10);
    }

    #[test]
    fn blank_line_breaks_block() {
        let src = "\
/// Verifies: REQ-A-1

#[test]
fn separated() {}
";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        assert_eq!(r.problems, vec![dangling(1)]);
    }

    #[test]
    fn plain_comment_breaks_block() {
        let src = "\
/// Verifies: REQ-A-1
// an ordinary comment
#[test]
fn separated() {}
";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        assert_eq!(r.problems, vec![dangling(1)]);
    }

    #[test]
    fn verifies_on_non_test_fn_is_dangling() {
        let src = "\
/// Verifies: REQ-A-1
pub fn helper() {}
";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        assert_eq!(r.problems, vec![dangling(1)]);
    }

    #[test]
    fn verifies_on_non_fn_item_is_dangling() {
        let src = "\
/// Verifies: REQ-A-1
#[test]
struct NotAFn;
";
        assert_eq!(scan(PATH, src).problems, vec![dangling(1)]);
    }

    #[test]
    fn verifies_at_end_of_file_is_dangling() {
        let src = "fn a() {}\n/// Verifies: REQ-A-1\n";
        assert_eq!(scan(PATH, src).problems, vec![dangling(2)]);
    }

    #[test]
    fn test_without_verifies_is_ignored() {
        let src = "#[test]\nfn untagged() {}\n";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        assert!(r.problems.is_empty());
    }

    #[test]
    fn malformed_list_is_reported() {
        let src = "\
/// Verifies: REQ-A-1, req-a-2
/// Verifies:
#[test]
fn bad() {}
";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        let lines: Vec<usize> = r
            .problems
            .iter()
            .map(|p| match p {
                Problem::MalformedVerifies { at, .. } => at.line,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(lines, vec![1, 2]);
    }

    #[test]
    fn quadruple_slash_is_not_a_doc_line() {
        let src = "//// Verifies: REQ-A-1\n#[test]\nfn t() {}\n";
        let r = scan(PATH, src);
        assert!(r.tests.is_empty());
        assert!(r.problems.is_empty());
    }

    #[test]
    fn fn_name_parsing() {
        assert_eq!(fn_name("fn a() {}"), Some("a".to_owned()));
        assert_eq!(
            fn_name("pub const unsafe fn b_2<T>()"),
            Some("b_2".to_owned())
        );
        assert_eq!(fn_name("extern \"C\" fn c()"), Some("c".to_owned()));
        assert_eq!(fn_name("let fn_x = 1;"), None);
        assert_eq!(fn_name("struct S;"), None);
    }
}
