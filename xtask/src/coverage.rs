//! Structural-coverage gate (ISO 26262-6 Table 12), as a pure core.
//!
//! The shell in `main.rs` runs `cargo llvm-cov --branch` on the pinned
//! [`TOOLCHAIN`] for the [`PACKAGES`] and hands the JSON summary export to
//! [`parse_summary`]. [`evaluate`] then compares the workspace totals with
//! [`THRESHOLDS`] and lists every file that still has uncovered branches.
//!
//! Percentages are kept as integer basis points (1/100 of a percent), rounded
//! down, so the gate never compares floating-point values. A category with no
//! instrumented items counts as fully covered.
//!
//! Branch coverage needs a nightly compiler (`-Z coverage-options=branch`),
//! so the toolchain is pinned here; the pin is a tool-confidence item listed
//! in `docs/tool-qualification.md`. Justified gaps below 100% are recorded in
//! `docs/coverage-exclusions.md`.

use std::fmt;

use serde_json::Value;

/// Toolchain that provides branch coverage instrumentation.
pub const TOOLCHAIN: &str = "nightly-2025-09-08";

/// Packages whose non-test code is measured.
pub const PACKAGES: [&str; 2] = ["middleware", "middleware-codegen"];

/// Workspace-relative directory that receives the reports.
pub const OUTPUT_DIR: &str = "target/coverage";

/// Minimum coverage, in basis points, below which the gate fails.
///
/// Set just under the level measured when the gate was introduced (96.78%
/// branches, 98.94% lines after `cargo fmt`; baseline before requirements-based testing was
/// 71.81% branches). Every branch still uncovered at that point is test
/// code or a tool artefact, as listed in `docs/coverage-exclusions.md`; the
/// small margin absorbs randomised property-test paths in test code. Raise,
/// never lower, these values without updating that document.
pub const THRESHOLDS: Thresholds = Thresholds {
    branch_bp: 9650,
    line_bp: 9890,
};

/// Gate thresholds in basis points (10000 = 100%).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// Minimum branch coverage.
    pub branch_bp: u64,
    /// Minimum line coverage.
    pub line_bp: u64,
}

/// Instrumented and covered item counts for one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// Number of instrumented items.
    pub count: u64,
    /// Number of those executed.
    pub covered: u64,
}

impl Counts {
    /// Coverage in basis points, rounded down; 10000 when `count` is 0.
    pub fn basis_points(self) -> u64 {
        match self.covered.saturating_mul(10_000).checked_div(self.count) {
            Some(bp) => bp.min(10_000),
            None => 10_000,
        }
    }

    /// Items not executed.
    pub fn missed(self) -> u64 {
        self.count.saturating_sub(self.covered)
    }
}

/// Coverage of one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCoverage {
    /// Path relative to the workspace root, `/`-separated.
    pub path: String,
    /// Branch counts.
    pub branches: Counts,
    /// Line counts.
    pub lines: Counts,
}

/// Parsed coverage summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Workspace branch totals.
    pub branches: Counts,
    /// Workspace line totals.
    pub lines: Counts,
    /// Per-file coverage, sorted by path.
    pub files: Vec<FileCoverage>,
}

/// Why a summary could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageError {
    /// The input is not JSON.
    Json(String),
    /// A required field is missing or has the wrong type.
    Field(String),
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "coverage summary is not valid JSON: {e}"),
            Self::Field(p) => write!(f, "coverage summary lacks numeric field `{p}`"),
        }
    }
}

/// Outcome of comparing a summary with thresholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// One message per threshold missed; empty means the gate passed.
    pub failures: Vec<String>,
    /// Files with at least one uncovered branch, as `(path, missed, count)`.
    pub branch_gaps: Vec<(String, u64, u64)>,
}

impl Verdict {
    /// Whether every threshold was met.
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Formats basis points as a percentage with two decimals.
pub fn percent(bp: u64) -> String {
    format!("{}.{:02}%", bp / 100, bp % 100)
}

/// Parses an `llvm.coverage.json.export` summary.
///
/// `root` is the absolute workspace root, stripped from file names so paths
/// in the result are workspace-relative.
///
/// # Errors
/// [`CoverageError::Json`] for malformed input; [`CoverageError::Field`] if
/// the export lacks `data[0].totals` or a file entry lacks its summary.
pub fn parse_summary(json: &str, root: &str) -> Result<Summary, CoverageError> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| CoverageError::Json(e.to_string()))?;
    let data = value
        .get("data")
        .and_then(|d| d.get(0))
        .ok_or_else(|| CoverageError::Field("data[0]".to_owned()))?;
    let totals = data
        .get("totals")
        .ok_or_else(|| CoverageError::Field("data[0].totals".to_owned()))?;
    let mut files = data
        .get("files")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|f| file_coverage(f, root))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Summary {
        branches: counts(totals, "branches", "totals")?,
        lines: counts(totals, "lines", "totals")?,
        files,
    })
}

/// Parses one entry of `data[0].files`.
fn file_coverage(file: &Value, root: &str) -> Result<FileCoverage, CoverageError> {
    let name = file
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| CoverageError::Field("files[].filename".to_owned()))?;
    let summary = file
        .get("summary")
        .ok_or_else(|| CoverageError::Field(format!("{name}.summary")))?;
    let path = name
        .strip_prefix(root)
        .map(|p| p.trim_start_matches('/'))
        .unwrap_or(name)
        .replace('\\', "/");
    Ok(FileCoverage {
        branches: counts(summary, "branches", name)?,
        lines: counts(summary, "lines", name)?,
        path,
    })
}

/// Reads `{count, covered}` from `parent[key]`.
fn counts(parent: &Value, key: &str, context: &str) -> Result<Counts, CoverageError> {
    let field = |name: &str| {
        parent
            .get(key)
            .and_then(|c| c.get(name))
            .and_then(Value::as_u64)
            .ok_or_else(|| CoverageError::Field(format!("{context}.{key}.{name}")))
    };
    Ok(Counts {
        count: field("count")?,
        covered: field("covered")?,
    })
}

/// Compares `summary` against `thresholds`.
pub fn evaluate(summary: &Summary, thresholds: Thresholds) -> Verdict {
    let checks = [
        ("branch", summary.branches, thresholds.branch_bp),
        ("line", summary.lines, thresholds.line_bp),
    ];
    let failures = checks
        .iter()
        .filter(|(_, c, min)| c.basis_points() < *min)
        .map(|(kind, c, min)| {
            format!(
                "{kind} coverage {} is below the threshold {}",
                percent(c.basis_points()),
                percent(*min)
            )
        })
        .collect();
    let branch_gaps = summary
        .files
        .iter()
        .filter(|f| f.branches.missed() > 0)
        .map(|f| (f.path.clone(), f.branches.missed(), f.branches.count))
        .collect();
    Verdict {
        failures,
        branch_gaps,
    }
}

#[cfg(test)]
// Test code: panicking on a failed expectation is the point of a test.
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing))]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"type":"llvm.coverage.json.export","version":"3.0.1",
      "data":[{"totals":{"branches":{"count":220,"covered":158},
                         "lines":{"count":3813,"covered":3547}},
               "files":[
                 {"filename":"/ws/b.rs","summary":{"branches":{"count":0,"covered":0},
                                                  "lines":{"count":10,"covered":10}}},
                 {"filename":"/ws/a.rs","summary":{"branches":{"count":8,"covered":7},
                                                  "lines":{"count":10,"covered":9}}}]}]}"#;

    fn sample() -> Summary {
        parse_summary(SAMPLE, "/ws").unwrap()
    }

    #[test]
    fn basis_points_round_down_and_handle_empty() {
        assert_eq!(
            Counts {
                count: 220,
                covered: 158
            }
            .basis_points(),
            7181
        );
        assert_eq!(
            Counts {
                count: 3,
                covered: 2
            }
            .basis_points(),
            6666
        );
        assert_eq!(
            Counts {
                count: 0,
                covered: 0
            }
            .basis_points(),
            10_000
        );
        assert_eq!(
            Counts {
                count: 4,
                covered: 4
            }
            .basis_points(),
            10_000
        );
        assert_eq!(
            Counts {
                count: 4,
                covered: 1
            }
            .missed(),
            3
        );
    }

    #[test]
    fn percent_formats_two_decimals() {
        assert_eq!(percent(7181), "71.81%");
        assert_eq!(percent(10_000), "100.00%");
        assert_eq!(percent(5), "0.05%");
    }

    #[test]
    fn parses_totals_and_sorts_relative_files() {
        let s = sample();
        assert_eq!(
            s.branches,
            Counts {
                count: 220,
                covered: 158
            }
        );
        assert_eq!(
            s.lines,
            Counts {
                count: 3813,
                covered: 3547
            }
        );
        let paths: Vec<_> = s.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.rs", "b.rs"]);
        assert_eq!(
            s.files[0].branches,
            Counts {
                count: 8,
                covered: 7
            }
        );
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(matches!(
            parse_summary("{", "/"),
            Err(CoverageError::Json(_))
        ));
        assert!(matches!(
            parse_summary("{}", "/"),
            Err(CoverageError::Field(_))
        ));
        let no_lines = r#"{"data":[{"totals":{"branches":{"count":1,"covered":1}}}]}"#;
        assert_eq!(
            parse_summary(no_lines, "/"),
            Err(CoverageError::Field("totals.lines.count".to_owned()))
        );
        let bad_file = r#"{"data":[{"totals":{"branches":{"count":1,"covered":1},
            "lines":{"count":1,"covered":1}},"files":[{"filename":"x.rs"}]}]}"#;
        assert_eq!(
            parse_summary(bad_file, "/"),
            Err(CoverageError::Field("x.rs.summary".to_owned()))
        );
    }

    #[test]
    fn missing_files_array_is_empty() {
        let json = r#"{"data":[{"totals":{"branches":{"count":0,"covered":0},
            "lines":{"count":0,"covered":0}}}]}"#;
        assert!(parse_summary(json, "/").unwrap().files.is_empty());
    }

    #[test]
    fn evaluate_passes_at_exact_threshold() {
        let v = evaluate(
            &sample(),
            Thresholds {
                branch_bp: 7181,
                line_bp: 9302,
            },
        );
        assert!(v.passed(), "{:?}", v.failures);
        assert_eq!(v.branch_gaps, vec![("a.rs".to_owned(), 1, 8)]);
    }

    #[test]
    fn evaluate_fails_one_below_threshold() {
        let v = evaluate(
            &sample(),
            Thresholds {
                branch_bp: 7182,
                line_bp: 9303,
            },
        );
        assert_eq!(v.failures.len(), 2);
        assert!(v.failures[0].contains("branch coverage 71.81%"));
        assert!(v.failures[1].contains("line coverage 93.02%"));
    }
}
