//! Requirement-to-test traceability: the pure core of `cargo xtask trace`.
//!
//! Nothing in this module or its submodules performs I/O. The shell in
//! `main.rs` reads files and hands their contents in as `&str`; this module
//! returns plain data:
//!
//! - [`requirements`] parses `design.md` into [`Requirement`]s.
//! - [`tests_scan`] scans one Rust source file for `#[test]` functions that
//!   carry `/// Verifies: REQ-…` doc lines, producing [`VerifyingTest`]s.
//! - [`report`] renders the generated `docs/traceability.md` table.
//! - [`analyze`] combines the parsed data and returns every [`Problem`]
//!   found, sorted, so output is deterministic.
//!
//! The checks performed by [`analyze`] are:
//!
//! 1. every ticked requirement has at least one verifying test;
//! 2. every requirement ID cited by a test exists in `design.md`;
//! 3. every requirement, ticked or not, carries a valid ASIL tag;
//! 4. no requirement ID is defined twice;
//! 5. no `Verifies:` line is dangling (unattached to a `#[test]` fn) or
//!    malformed.
//!
//! The sixth check, that the committed report matches what would be
//! generated, is [`stale_report_problem`], kept separate because only
//! `--check` mode compares against the file on disk.

pub mod report;
pub mod requirements;
pub mod tests_scan;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A validated requirement identifier of the form `REQ-<LETTERS>-<DIGITS>`,
/// for example `REQ-TYP-001`.
///
/// The only way to build one is [`ReqId::parse`], so every value held is
/// well formed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReqId(String);

impl ReqId {
    /// Parses `text` as a requirement ID, returning `None` unless it is
    /// exactly `REQ-`, one or more ASCII uppercase letters, `-`, and one or
    /// more ASCII digits.
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix("REQ-")?;
        let (area, number) = rest.split_once('-')?;
        let area_ok = !area.is_empty() && area.chars().all(|c| c.is_ascii_uppercase());
        let number_ok = !number.is_empty() && number.chars().all(|c| c.is_ascii_digit());
        if area_ok && number_ok {
            Some(Self(text.to_owned()))
        } else {
            None
        }
    }

    /// The identifier text, e.g. `REQ-TYP-001`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReqId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// ISO 26262 integrity classification attached to a requirement.
///
/// Written in `design.md` as `[ASIL A]` … `[ASIL D]` or `[QM]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Asil {
    /// Quality managed: no ASIL assigned.
    Qm,
    /// ASIL A.
    A,
    /// ASIL B.
    B,
    /// ASIL C.
    C,
    /// ASIL D.
    D,
}

impl Asil {
    /// Parses the text inside the brackets of a tag (`"ASIL B"`, `"QM"`).
    pub fn parse_tag(inner: &str) -> Option<Self> {
        match inner {
            "QM" => Some(Self::Qm),
            "ASIL A" => Some(Self::A),
            "ASIL B" => Some(Self::B),
            "ASIL C" => Some(Self::C),
            "ASIL D" => Some(Self::D),
            _ => None,
        }
    }

    /// The tag text without brackets, as written in `design.md`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Qm => "QM",
            Self::A => "ASIL A",
            Self::B => "ASIL B",
            Self::C => "ASIL C",
            Self::D => "ASIL D",
        }
    }
}

/// The classification state of a requirement line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AsilTag {
    /// No tag present between the ID and the colon.
    Missing,
    /// A bracketed tag was present but is not one of the five valid tags.
    Invalid(String),
    /// A valid tag.
    Valid(Asil),
}

/// Whether a requirement's checkbox is ticked (`- [x]`) or not (`- [ ]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// `- [ ]`: not yet validated.
    Open,
    /// `- [x]`: claimed validated, so it must be backed by a test.
    Ticked,
}

/// One requirement line from `design.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// The requirement identifier.
    pub id: ReqId,
    /// The ASIL classification as written.
    pub asil: AsilTag,
    /// Checkbox state.
    pub status: Status,
    /// 1-based line number in `design.md`.
    pub line: usize,
    /// Requirement text after the colon, trimmed.
    pub text: String,
}

/// A `#[test]` function that cites one or more requirements.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerifyingTest {
    /// Path relative to the workspace root, `/`-separated.
    pub path: String,
    /// Function name.
    pub name: String,
    /// 1-based line number of the `fn` line.
    pub line: usize,
    /// Requirement IDs cited, in the order written.
    pub ids: Vec<ReqId>,
}

/// A location in a scanned source file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceLoc {
    /// Path relative to the workspace root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path, self.line)
    }
}

/// Everything wrong with the traceability evidence. Variant order is the
/// order problems are listed in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Problem {
    /// A requirement ID appears on more than one requirement line.
    DuplicateRequirement {
        /// The repeated ID.
        id: ReqId,
        /// Every `design.md` line defining it, ascending.
        lines: Vec<usize>,
    },
    /// A requirement has no ASIL tag.
    MissingAsil {
        /// The requirement.
        id: ReqId,
        /// Its `design.md` line.
        line: usize,
    },
    /// A requirement has a bracketed tag that is not a valid ASIL tag.
    InvalidAsil {
        /// The requirement.
        id: ReqId,
        /// Its `design.md` line.
        line: usize,
        /// The tag text found inside the brackets.
        tag: String,
    },
    /// A ticked requirement has no verifying test.
    Untested {
        /// The requirement.
        id: ReqId,
        /// Its `design.md` line.
        line: usize,
    },
    /// A test cites an ID that is not defined in `design.md`.
    UnknownId {
        /// The cited ID.
        id: ReqId,
        /// The citing test's location.
        at: SourceLoc,
        /// The citing test's name.
        test: String,
    },
    /// A `Verifies:` line not attached to a `#[test]` function.
    DanglingVerifies {
        /// Location of the `Verifies:` line.
        at: SourceLoc,
    },
    /// A `Verifies:` line whose ID list cannot be parsed.
    MalformedVerifies {
        /// Location of the `Verifies:` line.
        at: SourceLoc,
        /// The offending text after `Verifies:`.
        text: String,
    },
    /// `design.md` contains a line that looks like a requirement but is
    /// not well formed (for example a malformed ID or missing colon).
    MalformedRequirement {
        /// Its `design.md` line.
        line: usize,
        /// Why it was rejected.
        reason: String,
    },
    /// The committed report differs from what would be generated.
    StaleReport {
        /// The report path relative to the workspace root.
        path: String,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRequirement { id, lines } => {
                let joined: Vec<String> = lines.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "duplicate requirement {id} defined at design.md lines {}",
                    joined.join(", ")
                )
            }
            Self::MissingAsil { id, line } => {
                write!(f, "design.md:{line}: {id} has no ASIL tag")
            }
            Self::InvalidAsil { id, line, tag } => write!(
                f,
                "design.md:{line}: {id} has invalid ASIL tag [{tag}] \
                 (expected [ASIL A], [ASIL B], [ASIL C], [ASIL D] or [QM])"
            ),
            Self::Untested { id, line } => {
                write!(
                    f,
                    "design.md:{line}: {id} is ticked but has no verifying test"
                )
            }
            Self::UnknownId { id, at, test } => {
                write!(f, "{at}: test {test} cites {id}, which is not in design.md")
            }
            Self::DanglingVerifies { at } => {
                write!(f, "{at}: `Verifies:` line is not attached to a #[test] fn")
            }
            Self::MalformedVerifies { at, text } => write!(
                f,
                "{at}: malformed `Verifies:` list {text:?} \
                 (expected comma-separated REQ-AAA-NNN IDs)"
            ),
            Self::MalformedRequirement { line, reason } => {
                write!(f, "design.md:{line}: malformed requirement line: {reason}")
            }
            Self::StaleReport { path } => write!(
                f,
                "{path} is out of date; run `cargo xtask trace` to regenerate it"
            ),
        }
    }
}

/// Result of scanning one source file: the verifying tests found and any
/// problems with `Verifies:` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    /// Tests carrying `Verifies:` lines.
    pub tests: Vec<VerifyingTest>,
    /// Dangling or malformed `Verifies:` lines.
    pub problems: Vec<Problem>,
}

impl ScanResult {
    /// Combines two scan results, returning a new one holding both.
    pub fn merge(self, other: Self) -> Self {
        let Self {
            mut tests,
            mut problems,
        } = self;
        tests.extend(other.tests);
        problems.extend(other.problems);
        Self { tests, problems }
    }
}

/// Map from requirement ID to the tests that cite it, each list sorted.
pub type Coverage<'a> = BTreeMap<&'a ReqId, Vec<&'a VerifyingTest>>;

/// Builds the ID → citing tests map. Each list is sorted by (path, line,
/// name) and holds a test once even if it cites the ID repeatedly.
pub fn coverage(tests: &[VerifyingTest]) -> Coverage<'_> {
    let mut map: Coverage<'_> = BTreeMap::new();
    for test in tests {
        let unique: BTreeSet<&ReqId> = test.ids.iter().collect();
        for id in unique {
            map.entry(id).or_default().push(test);
        }
    }
    for list in map.values_mut() {
        list.sort_by(|a, b| (&a.path, a.line, &a.name).cmp(&(&b.path, b.line, &b.name)));
    }
    map
}

/// Runs checks 1 to 5 over parsed requirements and scanned tests, returning
/// every problem found (including `parse_problems` from parsing) sorted and
/// de-duplicated.
pub fn analyze(
    requirements: &[Requirement],
    tests: &[VerifyingTest],
    parse_problems: Vec<Problem>,
) -> Vec<Problem> {
    let mut problems: BTreeSet<Problem> = parse_problems.into_iter().collect();

    let mut lines_by_id: BTreeMap<&ReqId, Vec<usize>> = BTreeMap::new();
    for req in requirements {
        lines_by_id.entry(&req.id).or_default().push(req.line);
    }
    for (id, lines) in &lines_by_id {
        if lines.len() > 1 {
            let mut sorted = lines.clone();
            sorted.sort_unstable();
            problems.insert(Problem::DuplicateRequirement {
                id: (*id).clone(),
                lines: sorted,
            });
        }
    }

    let covered = coverage(tests);
    for req in requirements {
        match &req.asil {
            AsilTag::Missing => {
                problems.insert(Problem::MissingAsil {
                    id: req.id.clone(),
                    line: req.line,
                });
            }
            AsilTag::Invalid(tag) => {
                problems.insert(Problem::InvalidAsil {
                    id: req.id.clone(),
                    line: req.line,
                    tag: tag.clone(),
                });
            }
            AsilTag::Valid(_) => {}
        }
        if req.status == Status::Ticked && !covered.contains_key(&req.id) {
            problems.insert(Problem::Untested {
                id: req.id.clone(),
                line: req.line,
            });
        }
    }

    for test in tests {
        for id in &test.ids {
            if !lines_by_id.contains_key(id) {
                problems.insert(Problem::UnknownId {
                    id: id.clone(),
                    at: SourceLoc {
                        path: test.path.clone(),
                        line: test.line,
                    },
                    test: test.name.clone(),
                });
            }
        }
    }

    problems.into_iter().collect()
}

/// Check 6: returns a [`Problem::StaleReport`] when `existing` (the file on
/// disk, `None` if absent) differs from `generated`.
pub fn stale_report_problem(
    path: &str,
    existing: Option<&str>,
    generated: &str,
) -> Option<Problem> {
    if existing == Some(generated) {
        None
    } else {
        Some(Problem::StaleReport {
            path: path.to_owned(),
        })
    }
}

/// Counts of each problem category, for the summary line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    /// Requirements parsed.
    pub requirements: usize,
    /// Verifying tests found.
    pub tests: usize,
    /// [`Problem::MissingAsil`] and [`Problem::InvalidAsil`] count.
    pub untagged: usize,
    /// [`Problem::Untested`] count.
    pub untested: usize,
    /// All other problems.
    pub other: usize,
}

/// Tallies `problems` into a [`Summary`].
pub fn summarize(requirements: usize, tests: usize, problems: &[Problem]) -> Summary {
    problems.iter().fold(
        Summary {
            requirements,
            tests,
            ..Summary::default()
        },
        |acc, p| match p {
            Problem::MissingAsil { .. } | Problem::InvalidAsil { .. } => Summary {
                untagged: acc.untagged.saturating_add(1),
                ..acc
            },
            Problem::Untested { .. } => Summary {
                untested: acc.untested.saturating_add(1),
                ..acc
            },
            _ => Summary {
                other: acc.other.saturating_add(1),
                ..acc
            },
        },
    )
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} requirements, {} verifying tests; {} without valid ASIL tag, \
             {} ticked but untested, {} other problems",
            self.requirements, self.tests, self.untagged, self.untested, self.other
        )
    }
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

    fn id(s: &str) -> ReqId {
        ReqId::parse(s).unwrap()
    }

    fn req(s: &str, asil: AsilTag, status: Status, line: usize) -> Requirement {
        Requirement {
            id: id(s),
            asil,
            status,
            line,
            text: "text".to_owned(),
        }
    }

    fn test_fn(name: &str, ids: &[&str]) -> VerifyingTest {
        VerifyingTest {
            path: "middleware/src/lib.rs".to_owned(),
            name: name.to_owned(),
            line: 10,
            ids: ids.iter().map(|s| id(s)).collect(),
        }
    }

    #[test]
    fn req_id_validation() {
        assert!(ReqId::parse("REQ-TYP-001").is_some());
        assert!(ReqId::parse("REQ-A-1").is_some());
        assert!(ReqId::parse("REQ-typ-001").is_none());
        assert!(ReqId::parse("REQ-TYP-").is_none());
        assert!(ReqId::parse("REQ--001").is_none());
        assert!(ReqId::parse("REQ-TYP-00a").is_none());
        assert!(ReqId::parse("XREQ-TYP-001").is_none());
        assert!(ReqId::parse("REQ-TYP-001-2").is_none());
    }

    #[test]
    fn asil_tags_round_trip() {
        for asil in [Asil::Qm, Asil::A, Asil::B, Asil::C, Asil::D] {
            assert_eq!(Asil::parse_tag(asil.label()), Some(asil));
        }
        assert_eq!(Asil::parse_tag("ASIL E"), None);
        assert_eq!(Asil::parse_tag("asil b"), None);
    }

    #[test]
    fn ticked_without_test_is_untested_but_open_is_not() {
        let reqs = [
            req("REQ-A-1", AsilTag::Valid(Asil::B), Status::Ticked, 1),
            req("REQ-A-2", AsilTag::Valid(Asil::B), Status::Open, 2),
        ];
        let problems = analyze(&reqs, &[], Vec::new());
        assert_eq!(
            problems,
            vec![Problem::Untested {
                id: id("REQ-A-1"),
                line: 1
            }]
        );
    }

    #[test]
    fn verified_tagged_requirement_has_no_problems() {
        let reqs = [req("REQ-A-1", AsilTag::Valid(Asil::B), Status::Ticked, 1)];
        let tests = [test_fn("t", &["REQ-A-1"])];
        assert!(analyze(&reqs, &tests, Vec::new()).is_empty());
    }

    #[test]
    fn missing_and_invalid_asil_reported_for_open_and_ticked() {
        let reqs = [
            req("REQ-A-1", AsilTag::Missing, Status::Open, 1),
            req(
                "REQ-A-2",
                AsilTag::Invalid("ASIL E".to_owned()),
                Status::Open,
                2,
            ),
        ];
        let problems = analyze(&reqs, &[], Vec::new());
        assert_eq!(
            problems,
            vec![
                Problem::MissingAsil {
                    id: id("REQ-A-1"),
                    line: 1
                },
                Problem::InvalidAsil {
                    id: id("REQ-A-2"),
                    line: 2,
                    tag: "ASIL E".to_owned()
                },
            ]
        );
    }

    #[test]
    fn unknown_cited_id_reported() {
        let reqs = [req("REQ-A-1", AsilTag::Valid(Asil::Qm), Status::Ticked, 1)];
        let tests = [test_fn("t", &["REQ-A-1", "REQ-ZZZ-999"])];
        let problems = analyze(&reqs, &tests, Vec::new());
        assert_eq!(
            problems,
            vec![Problem::UnknownId {
                id: id("REQ-ZZZ-999"),
                at: SourceLoc {
                    path: "middleware/src/lib.rs".to_owned(),
                    line: 10
                },
                test: "t".to_owned()
            }]
        );
    }

    #[test]
    fn duplicate_ids_reported_once_with_all_lines() {
        let reqs = [
            req("REQ-A-1", AsilTag::Valid(Asil::A), Status::Open, 7),
            req("REQ-A-1", AsilTag::Valid(Asil::A), Status::Open, 3),
        ];
        let problems = analyze(&reqs, &[], Vec::new());
        assert_eq!(
            problems,
            vec![Problem::DuplicateRequirement {
                id: id("REQ-A-1"),
                lines: vec![3, 7]
            }]
        );
    }

    #[test]
    fn parse_problems_are_kept_and_sorted() {
        let at = SourceLoc {
            path: "x.rs".to_owned(),
            line: 1,
        };
        let parse = vec![
            Problem::DanglingVerifies { at: at.clone() },
            Problem::MissingAsil {
                id: id("REQ-A-9"),
                line: 9,
            },
        ];
        let problems = analyze(&[], &[], parse);
        assert_eq!(
            problems,
            vec![
                Problem::MissingAsil {
                    id: id("REQ-A-9"),
                    line: 9
                },
                Problem::DanglingVerifies { at },
            ]
        );
    }

    #[test]
    fn coverage_counts_each_test_once_per_id() {
        let tests = [test_fn("t", &["REQ-A-1", "REQ-A-1"])];
        let cov = coverage(&tests);
        assert_eq!(cov.get(&id("REQ-A-1")).map(Vec::len), Some(1));
    }

    #[test]
    fn stale_report_detection() {
        assert_eq!(stale_report_problem("r.md", Some("a"), "a"), None);
        assert!(stale_report_problem("r.md", Some("a"), "b").is_some());
        assert!(stale_report_problem("r.md", None, "b").is_some());
    }

    #[test]
    fn summary_counts_categories() {
        let problems = [
            Problem::MissingAsil {
                id: id("REQ-A-1"),
                line: 1,
            },
            Problem::Untested {
                id: id("REQ-A-1"),
                line: 1,
            },
            Problem::StaleReport {
                path: "r.md".to_owned(),
            },
        ];
        let s = summarize(5, 2, &problems);
        assert_eq!(
            s,
            Summary {
                requirements: 5,
                tests: 2,
                untagged: 1,
                untested: 1,
                other: 1
            }
        );
    }
}
