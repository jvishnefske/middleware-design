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
//! Exit codes: 0 success, 1 traceability problems (check mode), 2 usage or
//! I/O error.

mod cli;
mod trace;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
