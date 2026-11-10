//! Host-only build automation for the workspace, run as `cargo xtask <cmd>`.
//!
//! This file is the imperative shell: it parses arguments with [`cli`],
//! reads and writes files, prints, and chooses the exit code. All decisions
//! live in the pure [`trace`] core.
//!
//! `cargo xtask trace` reads `design.md`, scans every `*.rs` file under the
//! [`SCAN_ROOTS`] (skipping `target/` and hidden directories; `xtask/` is not
//! a root because its own fixtures contain example tags), writes
//! `docs/traceability.md`, prints any problems as warnings and exits 0.
//!
//! `cargo xtask trace --check` does the same analysis without writing, also
//! fails if `docs/traceability.md` differs from what would be generated, and
//! exits 1 when any problem is found.
//!
//! `cargo xtask coverage` runs `cargo llvm-cov --branch` on the pinned
//! nightly (via `rustup run`) for the measured packages, writes the JSON
//! summary and an HTML report under `target/coverage/`, prints the files that
//! still have uncovered branches, and exits 1 if the totals fall below the
//! thresholds in [`coverage`].
//!
//! Exit codes: 0 success, 1 traceability problems (check mode) or coverage
//! below threshold, 2 usage, I/O or tool error.

mod cli;
mod coverage;
mod trace;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

use cli::{Command, TraceMode, UsageError};
use trace::{report, requirements, tests_scan, Problem, ScanResult};

/// Workspace-relative directories scanned for verifying tests.
const SCAN_ROOTS: [&str; 3] = ["middleware", "middleware-codegen", "checks"];

/// Directory names never descended into.
const SKIP_DIRS: [&str; 1] = ["target"];

/// Maximum directory nesting below a scan root; bounds the walk.
const MAX_DEPTH: usize = 32;

/// Failures of the shell itself, as opposed to traceability problems.
#[derive(Debug)]
enum XtaskError {
    Usage(UsageError),
    Io { path: PathBuf, source: io::Error },
    NoWorkspaceRoot,
    TooDeep(PathBuf),
    Tool { what: String, detail: String },
    Coverage(coverage::CoverageError),
}

impl fmt::Display for XtaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(e) => write!(f, "{e}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoWorkspaceRoot => f.write_str("cannot locate workspace root"),
            Self::TooDeep(path) => write!(
                f,
                "{}: directory nesting exceeds {MAX_DEPTH} levels",
                path.display()
            ),
            Self::Tool { what, detail } => write!(f, "{what} failed: {detail}"),
            Self::Coverage(e) => write!(f, "{e}"),
        }
    }
}

/// Wraps an `io::Error` with the path it concerns.
fn io_err(path: &Path) -> impl FnOnce(io::Error) -> XtaskError + '_ {
    move |source| XtaskError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

/// Dispatches the parsed command.
fn run(args: &[String]) -> Result<ExitCode, XtaskError> {
    match cli::parse(args).map_err(XtaskError::Usage)? {
        Command::Help => {
            println!("{}", cli::USAGE);
            Ok(ExitCode::SUCCESS)
        }
        Command::Trace(mode) => run_trace(&workspace_root()?, mode),
        Command::Coverage => run_coverage(&workspace_root()?),
    }
}

/// The workspace root: the parent of this crate's manifest directory.
fn workspace_root() -> Result<PathBuf, XtaskError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or(XtaskError::NoWorkspaceRoot)
}

/// Runs `trace` in the given mode.
fn run_trace(root: &Path, mode: TraceMode) -> Result<ExitCode, XtaskError> {
    let design_path = root.join("design.md");
    let design = fs::read_to_string(&design_path).map_err(io_err(&design_path))?;
    let (reqs, req_problems) = requirements::parse(&design);

    let scan = scan_roots(root)?;
    let generated = report::render(&reqs, &scan.tests);
    let mut problems = trace::analyze(
        &reqs,
        &scan.tests,
        req_problems.into_iter().chain(scan.problems).collect(),
    );

    let report_path = root.join(report::REPORT_PATH);
    match mode {
        TraceMode::Write => {
            if let Some(dir) = report_path.parent() {
                fs::create_dir_all(dir).map_err(io_err(dir))?;
            }
            fs::write(&report_path, &generated).map_err(io_err(&report_path))?;
            println!("wrote {}", report::REPORT_PATH);
        }
        TraceMode::Check => {
            let existing = match fs::read_to_string(&report_path) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(io_err(&report_path)(e)),
            };
            problems.extend(trace::stale_report_problem(
                report::REPORT_PATH,
                existing.as_deref(),
                &generated,
            ));
        }
    }

    print_problems(&problems);
    let summary = trace::summarize(reqs.len(), scan.tests.len(), &problems);
    println!("trace: {summary}");
    match (mode, problems.is_empty()) {
        (TraceMode::Check, false) => {
            eprintln!("trace --check FAILED: {} problem(s)", problems.len());
            Ok(ExitCode::FAILURE)
        }
        (TraceMode::Check, true) => {
            println!("trace --check passed");
            Ok(ExitCode::SUCCESS)
        }
        (TraceMode::Write, _) => Ok(ExitCode::SUCCESS),
    }
}

/// Prints each problem on its own line.
fn print_problems(problems: &[Problem]) {
    if !problems.is_empty() {
        eprintln!("traceability problems:");
    }
    for p in problems {
        eprintln!("  - {p}");
    }
}

/// Scans every `*.rs` file under each existing scan root.
fn scan_roots(root: &Path) -> Result<ScanResult, XtaskError> {
    let mut files = Vec::new();
    for dir in SCAN_ROOTS {
        let path = root.join(dir);
        if path.is_dir() {
            collect_rs_files(&path, &mut files)?;
        }
    }
    files.sort();
    files.iter().try_fold(ScanResult::default(), |acc, file| {
        let contents = fs::read_to_string(file).map_err(io_err(file))?;
        Ok(acc.merge(tests_scan::scan(&relative(root, file), &contents)))
    })
}

/// Collects `*.rs` files under `start`, iteratively with a bounded depth.
fn collect_rs_files(start: &Path, out: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    let mut stack = vec![(start.to_path_buf(), 0_usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(XtaskError::TooDeep(dir));
        }
        for entry in fs::read_dir(&dir).map_err(io_err(&dir))? {
            let entry = entry.map_err(io_err(&dir))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(io_err(&path))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if file_type.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_ref()) {
                    stack.push((path, depth.saturating_add(1)));
                }
            } else if file_type.is_file() && path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// `file` relative to `root`, with `/` separators on every platform.
fn relative(root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Runs one `cargo llvm-cov` step on the pinned toolchain.
fn llvm_cov(root: &Path, args: &[&str]) -> Result<(), XtaskError> {
    let what = format!(
        "rustup run {} cargo llvm-cov {}",
        coverage::TOOLCHAIN,
        args.join(" ")
    );
    println!("running: {what}");
    let status = process::Command::new("rustup")
        .args(["run", coverage::TOOLCHAIN, "cargo", "llvm-cov"])
        .args(args)
        .current_dir(root)
        .env_remove("RUSTUP_TOOLCHAIN")
        .status()
        .map_err(|e| XtaskError::Tool {
            what: what.clone(),
            detail: e.to_string(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::Tool {
            what,
            detail: status.to_string(),
        })
    }
}

/// Measures coverage, writes the reports and applies the thresholds.
fn run_coverage(root: &Path) -> Result<ExitCode, XtaskError> {
    let packages: Vec<&str> = coverage::PACKAGES
        .iter()
        .flat_map(|p| ["--package", p])
        .collect();
    let out_dir = root.join(coverage::OUTPUT_DIR);
    fs::create_dir_all(&out_dir).map_err(io_err(&out_dir))?;
    let summary_path = out_dir.join("summary.json");
    let summary_arg = summary_path.to_string_lossy().into_owned();
    let out_arg = out_dir.to_string_lossy().into_owned();

    llvm_cov(root, &["clean", "--workspace"])?;
    llvm_cov(
        root,
        &[&["--branch", "--no-report"], packages.as_slice()].concat(),
    )?;
    llvm_cov(
        root,
        &[
            &[
                "report",
                "--branch",
                "--json",
                "--summary-only",
                "--output-path",
                &summary_arg,
            ],
            packages.as_slice(),
        ]
        .concat(),
    )?;
    llvm_cov(
        root,
        &[
            &["report", "--branch", "--html", "--output-dir", &out_arg],
            packages.as_slice(),
        ]
        .concat(),
    )?;

    let json = fs::read_to_string(&summary_path).map_err(io_err(&summary_path))?;
    let summary =
        coverage::parse_summary(&json, &root.to_string_lossy()).map_err(XtaskError::Coverage)?;
    let verdict = coverage::evaluate(&summary, coverage::THRESHOLDS);

    for (path, missed, count) in &verdict.branch_gaps {
        println!("  {path}: {missed} of {count} branches not covered");
    }
    println!(
        "coverage: branches {} ({} of {}), lines {} ({} of {})",
        coverage::percent(summary.branches.basis_points()),
        summary.branches.covered,
        summary.branches.count,
        coverage::percent(summary.lines.basis_points()),
        summary.lines.covered,
        summary.lines.count,
    );
    println!(
        "report: {}",
        out_dir.join("html").join("index.html").display()
    );
    println!("summary: {}", summary_path.display());
    if verdict.passed() {
        println!("coverage gate passed");
        Ok(ExitCode::SUCCESS)
    } else {
        for f in &verdict.failures {
            eprintln!("  - {f}");
        }
        eprintln!("coverage gate FAILED");
        Ok(ExitCode::FAILURE)
    }
}
