//! # Platform requirement checks (REQ-PLT-*)
//!
//! Host-side tests that give evidence for the platform requirements in
//! section 8 of `design.md`. They fall into four groups:
//!
//! 1. **Reference-target build.** [`builds_for_thumbv6m_without_std_or_alloc`]
//!    runs `cargo build -p middleware -p nostd-gen --target
//!    thumbv6m-none-eabi` as a subprocess, in its own target directory under
//!    the workspace `target/nostd-check` so it never waits on the build lock
//!    held by the outer `cargo test`. [`target_dependency_graph_has_no_std_or_alloc_feature`]
//!    asks `cargo tree` which features the target build enables.
//! 2. **Source scans.** Simple line-based scans of `middleware/src` (no heap
//!    or `std` paths in non-test code) and of every member's `src/` (no
//!    non-test `allow` of a denied lint). Their limits are listed on each
//!    test.
//! 3. **Lint policy.** A small line-based reader of the workspace and member
//!    `Cargo.toml` files checks the `[workspace.lints]` set and every member's
//!    opt-in.
//! 4. **API shape.** Exhaustive cross-crate `match`es over the error enums,
//!    function-pointer coercions that pin fallible signatures to `Result`
//!    and `update`/`process` to take `self` by value, and `size_of`
//!    relations that show storage is inline.
//!
//! Everything here uses `std` only; no extra dev-dependency is needed.

// Test code: a panic is a test failure, which is the intended reporting
// mechanism, and the fixed tables below are indexed by literals.
#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)
)]

use core::mem::size_of;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use middleware::component::active::Schedule;
use middleware::component::LifecycleState;
use middleware::control::{
    ControlBlock, ControlError as BlockError, HighPassFilter, LowPassFilter, NotchFilter,
    PidController, PidControllerBuilder, RateLimiter, Saturator,
};
use middleware::state_machine::{
    FsmError, State, StateMachine, TimedGuard, Transition, TransitionResult,
};
use middleware::telemetry::buffer::BufferError;
use middleware::{
    BoundedString, BoundedVec, ControlError, CycleError, DiscreteStateSpace, Event, FrameworkError,
    InitError, InvokeError, LifecycleError, PidGains, PidLimits, PortError, RecvError, SendError,
    SerializationError, SerializeError, StateMachineError, TelemetryBuffer,
};

/// Reference embedded target (RP2040, Cortex-M0+).
const TARGET: &str = "thumbv6m-none-eabi";

/// Maximum directory depth walked below a scan root; bounds every walk.
const MAX_DEPTH: usize = 8;

/// Workspace root: `checks/nostd-gen/../..`.
fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("checks/nostd-gen sits two levels below the workspace root")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// A `cargo` command for the workspace with the outer build's
/// instrumentation and wrapper settings removed, so a run under
/// `cargo llvm-cov` or `cargo clippy` does not leak `-C instrument-coverage`
/// or a clippy driver into the cross build.
fn clean_cargo(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(root);
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy().into_owned();
        let leaks = key.ends_with("RUSTFLAGS")
            || key == "RUSTDOCFLAGS"
            || key == "RUSTC_WRAPPER"
            || key == "RUSTC_WORKSPACE_WRAPPER"
            || key == "LLVM_PROFILE_FILE"
            || key == "CARGO_TARGET_DIR"
            || key == "CARGO_BUILD_TARGET"
            || key == "CARGO_MAKEFLAGS"
            || key.starts_with("CARGO_LLVM_COV")
            || key.starts_with("__CARGO");
        if leaks {
            cmd.env_remove(&key);
        }
    }
    cmd
}

/// Fails loudly (rather than skipping) when the reference target's standard
/// library is not installed.
fn require_target_installed() {
    let out = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .expect("running `rustup target list --installed`; rustup is required for this check");
    let installed = String::from_utf8_lossy(&out.stdout);
    assert!(
        installed.lines().any(|l| l.trim() == TARGET),
        "Rust target {TARGET} is not installed; run `rustup target add {TARGET}`. \
         This check verifies REQ-PLT-001/002/023 and must not be skipped."
    );
}

/// Builds the library and the generated example model for the reference
/// `no_std` target in a subprocess. `nostd-gen` is `#![no_std]` and expands
/// `examples/attitude_controller.yaml` with `middleware_model!`, so success
/// shows the generated code and the library need no `std`.
///
/// The build produces rlibs, not a linked image, so on its own it does not
/// prove the absence of `alloc` (`alloc` exists for this target). That part
/// of the evidence is the `cargo tree` feature check and the source scan
/// below.
/// Verifies: REQ-PLT-001, REQ-PLT-023
#[test]
fn builds_for_thumbv6m_without_std_or_alloc() {
    require_target_installed();
    let root = workspace_root();
    let target_dir = root.join("target").join("nostd-check");
    let out = clean_cargo(&root)
        .args([
            "build",
            "--locked",
            "-p",
            "middleware",
            "-p",
            "nostd-gen",
            "--target",
            TARGET,
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .expect("spawning cargo");
    assert!(
        out.status.success(),
        "cargo build --target {TARGET} failed ({}):\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let rlib = target_dir
        .join(TARGET)
        .join("debug")
        .join("libnostd_gen.rlib");
    assert!(rlib.is_file(), "expected build output {}", rlib.display());
}

/// Lists every package and enabled feature in the target-side dependency
/// graph of `middleware` and `nostd-gen` (normal edges, proc-macros
/// excluded) and asserts no package enables a `std` or `alloc` feature.
/// This is the literal "no alloc feature required" clause; it relies on
/// dependencies gating their `alloc` use behind such a feature, which is the
/// Cargo convention for `nalgebra`, `minicbor` and `num-*`.
/// Verifies: REQ-PLT-002, REQ-PLT-023
#[test]
fn target_dependency_graph_has_no_std_or_alloc_feature() {
    let root = workspace_root();
    let out = clean_cargo(&root)
        .args([
            "tree",
            "--locked",
            "-p",
            "middleware",
            "-p",
            "nostd-gen",
            "--target",
            TARGET,
            "-e",
            "normal,no-proc-macro",
            "--prefix",
            "none",
            "-f",
            "{p}|{f}",
        ])
        .output()
        .expect("spawning cargo tree");
    assert!(
        out.status.success(),
        "cargo tree failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut packages = BTreeSet::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let (pkg, features) = line.split_once('|').expect("format {p}|{f}");
        let pkg_name = pkg.split_whitespace().next().unwrap_or_default().to_owned();
        let features = features.trim_end_matches(" (*)");
        for f in features.split(',').map(str::trim) {
            assert!(
                f != "std" && f != "alloc",
                "{pkg_name} enables feature `{f}` on {TARGET}"
            );
        }
        packages.insert(pkg_name);
    }
    // Guard against an empty or misparsed listing passing vacuously.
    for expected in ["middleware", "nostd-gen", "nalgebra", "minicbor"] {
        assert!(
            packages.contains(expected),
            "cargo tree output lacks {expected}: {text}"
        );
    }
}

/// Returns every `.rs` file below `dir`, depth-bounded by [`MAX_DEPTH`].
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = pending.pop() {
        let entries = fs::read_dir(&d).unwrap_or_else(|e| panic!("listing {}: {e}", d.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() && depth < MAX_DEPTH {
                pending.push((path, depth + 1));
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Removes `//` comments (including doc comments) and the contents of
/// ordinary one-line string literals from a line. Escaped quotes, raw
/// strings and multi-line literals are not handled.
fn strip_line(line: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut prev = '\0';
    for c in line.chars() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
                out.push('"');
            }
        } else if c == '"' {
            in_str = true;
            out.push('"');
        } else if c == '/' && prev == '/' {
            out.pop();
            break;
        } else {
            out.push(c);
        }
        prev = c;
    }
    out
}

fn brace_delta(line: &str) -> i64 {
    line.chars()
        .map(|c| match c {
            '{' => 1,
            '}' => -1,
            _ => 0,
        })
        .sum()
}

/// Returns the non-test code of a source file, comments and string contents
/// stripped, one entry per line. A `#[cfg(test)]` attribute removes itself,
/// any attributes after it, and the next item: up to its `;` if it has no
/// body, otherwise to the brace that closes its body.
fn non_test_lines(src: &str) -> Vec<String> {
    let mut kept = Vec::new();
    let mut lines = src.lines().map(strip_line);
    while let Some(line) = lines.next() {
        if line.trim() != "#[cfg(test)]" {
            kept.push(line);
            continue;
        }
        let mut depth = 0i64;
        let mut opened = false;
        for item in lines.by_ref() {
            let t = item.trim();
            if !opened && (t.is_empty() || t.starts_with("#[")) {
                continue;
            }
            depth += brace_delta(&item);
            opened |= item.contains('{');
            if (opened && depth <= 0) || (!opened && t.ends_with(';')) {
                break;
            }
        }
    }
    kept
}

/// Splits code into identifier-like tokens, keeping `::` and `!` attached
/// as separate one-character markers so paths and macros can be matched.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in line.chars() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if c == ':' || c == '!' {
                out.push(c.to_string());
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Heap-owning `std`/`alloc` types and macros that must not appear in
/// library code.
const HEAP_IDENTS: [&str; 7] = ["Vec", "Box", "String", "Rc", "Arc", "ToString", "ToOwned"];

/// Scans non-test code under `middleware/src` for `extern crate alloc` or
/// `std`, `alloc::`/`std::` paths, the heap types in [`HEAP_IDENTS`] and the
/// `vec!`/`format!` macros. Matching is by whole identifier, so
/// `BoundedVec` and `BoundedString` are not hits.
///
/// Limits: the scan is textual. It does not expand macros, so a heap type
/// reached through a macro from another crate is not seen; it does not
/// resolve `use` renames; and its `cfg(test)` stripping only understands an
/// item directly after a line that is exactly `#[cfg(test)]`. It is a guard
/// against accidental use, backed by the target build and feature check.
/// Verifies: REQ-PLT-002, REQ-PLT-001
#[test]
fn library_source_uses_no_heap_or_std() {
    let src_dir = workspace_root().join("middleware").join("src");
    let files = rust_files(&src_dir);
    assert!(
        files.len() > 10,
        "expected the middleware sources, found {files:?}"
    );
    let mut hits = Vec::new();
    for file in files.iter().filter(|f| !f.ends_with("test_support.rs")) {
        for (n, line) in non_test_lines(&read(file)).iter().enumerate() {
            let toks = tokens(line);
            let next = |i: usize| toks.get(i + 1).map(String::as_str);
            for (i, t) in toks.iter().enumerate() {
                let path_root = (t == "alloc" || t == "std") && next(i) == Some(":");
                let extern_crate = t == "crate"
                    && i > 0
                    && toks[i - 1] == "extern"
                    && matches!(next(i), Some("alloc" | "std"));
                let macro_call = (t == "vec" || t == "format") && next(i) == Some("!");
                if path_root || extern_crate || macro_call || HEAP_IDENTS.contains(&t.as_str()) {
                    hits.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
                    break;
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "heap or std use in library code:\n{}",
        hits.join("\n")
    );
}

/// The scanner itself: test items are stripped, heap identifiers are found,
/// look-alike names are not.
#[test]
fn source_scanner_strips_tests_and_matches_whole_identifiers() {
    let src = "use a::BoundedVec;\n#[cfg(test)]\nmod tests;\n#[cfg(test)]\n#[allow(x)]\nmod t {\n    fn f() { let v: Vec<u8> = vec![]; }\n}\nfn g() -> String { todo() } // Box\n";
    let kept = non_test_lines(src);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(tokens(&kept[0]).contains(&"BoundedVec".to_owned()));
    assert!(!tokens(&kept[0]).contains(&"Vec".to_owned()));
    let g = tokens(&kept[1]);
    assert!(g.contains(&"String".to_owned()));
    assert!(
        !g.contains(&"Box".to_owned()),
        "comment must be stripped: {g:?}"
    );
    assert_eq!(
        strip_line(r#"let s = "Vec // no"; // c"#),
        r#"let s = ""; "#
    );
}

/// A minimal line-based reader for the flat TOML used by this workspace's
/// manifests. Returns `(section, key, value)` triples with comments removed;
/// array values spanning several lines are joined. Inline tables are kept as
/// raw text.
fn toml_entries(text: &str) -> Vec<(String, String, String)> {
    let mut entries = Vec::new();
    let mut section = String::new();
    let mut lines = text.lines();
    while let Some(raw) = lines.next() {
        let line = raw.split(" #").next().unwrap_or_default().trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && !line.contains('=') {
            section = line
                .trim_matches(|c| c == '[' || c == ']')
                .trim()
                .to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let mut value = value.trim().to_owned();
        if value.starts_with('[') {
            while !value.contains(']') {
                let Some(more) = lines.next() else { break };
                value.push_str(more.split(" #").next().unwrap_or_default().trim());
            }
        }
        entries.push((section.clone(), key.trim().to_owned(), value));
    }
    entries
}

fn lookup<'a>(
    entries: &'a [(String, String, String)],
    section: &str,
    key: &str,
) -> Option<&'a str> {
    entries
        .iter()
        .find(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v.as_str())
}

/// Lint level of an entry written either as `"deny"` or as an inline table
/// `{ level = "deny", ... }`.
fn lint_level(value: &str) -> &str {
    let v = value.trim();
    let v = match v.find("level") {
        Some(i) if v.starts_with('{') => v[i..].split_once('=').map_or(v, |(_, r)| r),
        _ => v,
    };
    v.trim()
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or_default()
}

/// Workspace members from the root manifest's `[workspace] members`.
fn workspace_members(entries: &[(String, String, String)]) -> Vec<String> {
    let list = lookup(entries, "workspace", "members").expect("[workspace] members");
    list.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|m| m.trim().trim_matches('"').to_owned())
        .filter(|m| !m.is_empty())
        .collect()
}

/// The coding-guideline clippy lints that every crate must deny.
const DENIED_CLIPPY: [&str; 7] = [
    "unwrap_used",
    "expect_used",
    "panic",
    "indexing_slicing",
    "float_cmp",
    "as_conversions",
    "cast_possible_truncation",
];

/// Reads the root manifest and asserts `[workspace.lints.rust]` forbids
/// `unsafe_code` and `[workspace.lints.clippy]` denies (or forbids) every lint
/// in [`DENIED_CLIPPY`]; then asserts every workspace member opts in with
/// `[lints] workspace = true` and adds no local lint tables that could relax
/// them. `forbid` cannot be overridden by an inner `allow`, so this makes
/// `unsafe` a compile error in every crate.
/// Verifies: REQ-PLT-020, REQ-PLT-022
#[test]
fn workspace_lint_policy_is_enforced_by_every_member() {
    let root = workspace_root();
    let entries = toml_entries(&read(&root.join("Cargo.toml")));
    assert_eq!(
        lookup(&entries, "workspace.lints.rust", "unsafe_code").map(lint_level),
        Some("forbid"),
        "unsafe_code must be forbidden workspace-wide"
    );
    for lint in DENIED_CLIPPY {
        let level = lookup(&entries, "workspace.lints.clippy", lint).map(lint_level);
        assert!(
            matches!(level, Some("deny" | "forbid")),
            "clippy::{lint} must be denied in [workspace.lints.clippy], found {level:?}"
        );
    }
    let members = workspace_members(&entries);
    assert!(members.len() >= 4, "unexpected member list {members:?}");
    for member in &members {
        let manifest = read(&root.join(member).join("Cargo.toml"));
        let m = toml_entries(&manifest);
        let opted_in = lookup(&m, "lints", "workspace") == Some("true")
            || lookup(&m, "", "lints.workspace") == Some("true");
        assert!(
            opted_in,
            "{member}/Cargo.toml must set [lints] workspace = true"
        );
        let local = m.iter().find(|(s, _, _)| s.starts_with("lints."));
        assert!(
            local.is_none(),
            "{member}/Cargo.toml overrides lints locally: {local:?}"
        );
    }
}

/// The TOML reader and lint-level parser handle the forms the policy test
/// accepts, so a reformatted manifest is not misread as compliant.
#[test]
fn toml_reader_handles_tables_arrays_and_inline_levels() {
    let text = "[workspace]\nmembers = [\n  \"a\",\n  \"b\", # c\n]\n[workspace.lints.clippy]\npanic = { level = \"deny\", priority = 1 }\nfloat_cmp = \"warn\" # no\n";
    let e = toml_entries(text);
    assert_eq!(workspace_members(&e), ["a", "b"]);
    assert_eq!(
        lookup(&e, "workspace.lints.clippy", "panic").map(lint_level),
        Some("deny")
    );
    assert_eq!(
        lookup(&e, "workspace.lints.clippy", "float_cmp").map(lint_level),
        Some("warn")
    );
    assert_eq!(lookup(&e, "workspace.lints.clippy", "missing"), None);
}

/// Scans each member's `src/` for an `allow`/`expect`/`warn` attribute that
/// names `unsafe_code` or a lint in [`DENIED_CLIPPY`] and is not wrapped in
/// `cfg_attr(test, ...)`. Such an attribute would switch the guideline off
/// in non-test code. Attributes are matched from `#[`/`#![` to the bracket
/// that closes them, so multi-line `cfg_attr`s are handled; comments are
/// stripped first, string contents are not inspected.
/// Verifies: REQ-PLT-022
#[test]
fn no_member_relaxes_a_denied_lint_outside_tests() {
    let root = workspace_root();
    let members = workspace_members(&toml_entries(&read(&root.join("Cargo.toml"))));
    let mut hits = Vec::new();
    for member in &members {
        for file in rust_files(&root.join(member).join("src")) {
            let code: String = read(&file)
                .lines()
                .map(|l| strip_line(l) + "\n")
                .collect::<String>()
                .split_whitespace()
                .collect();
            for attr in attributes(&code) {
                let relaxes = ["allow(", "expect(", "warn("]
                    .iter()
                    .any(|a| attr.contains(a));
                let names_denied = attr.contains("unsafe_code")
                    || DENIED_CLIPPY
                        .iter()
                        .any(|l| attr.contains(&format!("clippy::{l}")));
                if relaxes && names_denied && !attr.starts_with("cfg_attr(test,") {
                    hits.push(format!("{}: #[{attr}]", file.display()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "denied lint relaxed in non-test code:\n{}",
        hits.join("\n")
    );
}

/// Bodies of all `#[...]` / `#![...]` attributes in whitespace-free code.
fn attributes(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(i) = rest.find('#') {
        rest = &rest[i + 1..];
        let body = rest.strip_prefix('!').unwrap_or(rest);
        let Some(body) = body.strip_prefix('[') else {
            continue;
        };
        let mut depth = 1usize;
        let end = body.char_indices().find_map(|(j, c)| {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(j)
        });
        if let Some(end) = end {
            out.push(body[..end].to_owned());
        }
    }
    out
}

#[test]
fn attribute_extractor_finds_nested_and_inner_attributes() {
    let attrs = attributes(
        "#![forbid(unsafe_code)]#[cfg_attr(test,allow(clippy::panic))]fn f(){let a=x[0];}",
    );
    assert_eq!(
        attrs,
        ["forbid(unsafe_code)", "cfg_attr(test,allow(clippy::panic))"]
    );
}

// Exhaustive matches over every public error enum. They live in a different
// crate from the enums, so if any enum were `#[non_exhaustive]` these
// wildcard-free matches would fail to compile; a new variant (catch-all or
// otherwise) also fails to compile until it is listed here and reviewed.
// Unit patterns (`E::V` with no fields) additionally prove no variant carries
// a payload such as an error code or message, the usual shape of a catch-all.

fn init_name(e: InitError) -> &'static str {
    match e {
        InitError::InvalidConfig => "InvalidConfig",
        InitError::ResourceUnavailable => "ResourceUnavailable",
        InitError::DependencyFailed => "DependencyFailed",
        InitError::HardwareFailed => "HardwareFailed",
    }
}
fn lifecycle_name(e: LifecycleError) -> &'static str {
    match e {
        LifecycleError::InvalidTransition => "InvalidTransition",
        LifecycleError::EntryActionFailed => "EntryActionFailed",
        LifecycleError::ExitActionFailed => "ExitActionFailed",
        LifecycleError::HookFailed => "HookFailed",
    }
}
fn port_name(e: PortError) -> &'static str {
    match e {
        PortError::SendBufferFull => "SendBufferFull",
        PortError::ReceiveBufferEmpty => "ReceiveBufferEmpty",
        PortError::Timeout => "Timeout",
        PortError::InvalidMessage => "InvalidMessage",
        PortError::NotConnected => "NotConnected",
    }
}
fn invoke_name(e: InvokeError) -> &'static str {
    match e {
        InvokeError::InvalidParameters => "InvalidParameters",
        InvokeError::InvalidState => "InvalidState",
        InvokeError::NotSupported => "NotSupported",
        InvokeError::ProcessingFailed => "ProcessingFailed",
    }
}
fn cycle_name(e: CycleError) -> &'static str {
    match e {
        CycleError::NotReady => "NotReady",
        CycleError::InputUnavailable => "InputUnavailable",
        CycleError::Timeout => "Timeout",
        CycleError::ComputationError => "ComputationError",
    }
}
fn recv_name(e: RecvError) -> &'static str {
    match e {
        RecvError::NoMessage => "NoMessage",
        RecvError::Disconnected => "Disconnected",
        RecvError::DeserializationFailed => "DeserializationFailed",
    }
}
fn send_name(e: SendError) -> &'static str {
    match e {
        SendError::Full => "Full",
        SendError::Disconnected => "Disconnected",
        SendError::SerializationFailed => "SerializationFailed",
    }
}
fn serialization_name(e: SerializationError) -> &'static str {
    match e {
        SerializationError::BufferTooSmall => "BufferTooSmall",
        SerializationError::InvalidFormat => "InvalidFormat",
        SerializationError::EncodingError => "EncodingError",
        SerializationError::DecodingError => "DecodingError",
    }
}
fn state_machine_name(e: StateMachineError) -> &'static str {
    match e {
        StateMachineError::NoValidTransition => "NoValidTransition",
        StateMachineError::GuardFailed => "GuardFailed",
        StateMachineError::ActionFailed => "ActionFailed",
        StateMachineError::InvalidState => "InvalidState",
        StateMachineError::InvalidEvent => "InvalidEvent",
        StateMachineError::InvalidTimeStep => "InvalidTimeStep",
    }
}
fn serialize_name(e: SerializeError) -> &'static str {
    match e {
        SerializeError::BufferTooSmall => "BufferTooSmall",
        SerializeError::InvalidFormat => "InvalidFormat",
        SerializeError::UnsupportedType => "UnsupportedType",
        SerializeError::EncodeFailed => "EncodeFailed",
        SerializeError::DecodeFailed => "DecodeFailed",
    }
}
fn control_name(e: ControlError) -> &'static str {
    match e {
        ControlError::InvalidGain => "InvalidGain",
        ControlError::InvalidTimeStep => "InvalidTimeStep",
        ControlError::Overflow => "Overflow",
        ControlError::DivisionByZero => "DivisionByZero",
        ControlError::DimensionMismatch => "DimensionMismatch",
    }
}
fn block_name(e: BlockError) -> &'static str {
    match e {
        BlockError::InvalidInput => "InvalidInput",
        BlockError::InvalidDelta => "InvalidDelta",
        BlockError::NumericalError => "NumericalError",
        BlockError::InvalidConfig => "InvalidConfig",
    }
}
fn fsm_name(e: FsmError) -> &'static str {
    match e {
        FsmError::EntryFailed => "EntryFailed",
        FsmError::ExitFailed => "ExitFailed",
        FsmError::ActionFailed => "ActionFailed",
    }
}
fn buffer_name(e: BufferError) -> &'static str {
    match e {
        BufferError::BufferOverflow => "BufferOverflow",
        BufferError::EncodeError => "EncodeError",
    }
}
/// `FrameworkError` wraps the leaf enums; each arm delegates to the leaf's
/// exhaustive match, so the whole tree is covered without a wildcard.
fn framework_name(e: FrameworkError) -> (&'static str, &'static str) {
    match e {
        FrameworkError::Init(x) => ("Init", init_name(x)),
        FrameworkError::Lifecycle(x) => ("Lifecycle", lifecycle_name(x)),
        FrameworkError::Port(x) => ("Port", port_name(x)),
        FrameworkError::StateMachine(x) => ("StateMachine", state_machine_name(x)),
        FrameworkError::Serialize(x) => ("Serialize", serialize_name(x)),
        FrameworkError::Control(x) => ("Control", control_name(x)),
        FrameworkError::Invoke(x) => ("Invoke", invoke_name(x)),
        FrameworkError::Cycle(x) => ("Cycle", cycle_name(x)),
    }
}

/// Variant names commonly used for catch-all errors.
const CATCH_ALL_NAMES: [&str; 7] = [
    "Other",
    "Unknown",
    "Unspecified",
    "Generic",
    "Custom",
    "Misc",
    "Internal",
];

/// Checks one enum's variant list: the `Debug` name of each value equals the
/// name its exhaustive match gives (so the list and match agree with the
/// real variants), names are distinct, the count is as expected, and none is
/// a catch-all name.
fn check_enum<E: Copy + core::fmt::Debug>(all: &[E], name: fn(E) -> &'static str, count: usize) {
    assert_eq!(all.len(), count);
    let mut seen = BTreeSet::new();
    for &e in all {
        let n = name(e);
        assert_eq!(format!("{e:?}"), n);
        assert!(seen.insert(n), "duplicate variant {n}");
        assert!(!CATCH_ALL_NAMES.contains(&n), "catch-all variant {n}");
    }
}

/// Every public error enum is matched exhaustively from outside the crate,
/// without a wildcard, and every variant is a named, payload-free case (the
/// wrapper `FrameworkError` holds only other exhaustive enums). Compile
/// failure of this file is the detection mechanism for `#[non_exhaustive]`
/// or a new variant; the runtime part checks the lists are complete and
/// contain no catch-all name.
/// Verifies: REQ-PLT-011
#[test]
fn error_enums_are_exhaustive_without_catch_all() {
    use InitError as I;
    check_enum(
        &[
            I::InvalidConfig,
            I::ResourceUnavailable,
            I::DependencyFailed,
            I::HardwareFailed,
        ],
        init_name,
        4,
    );
    use LifecycleError as L;
    check_enum(
        &[
            L::InvalidTransition,
            L::EntryActionFailed,
            L::ExitActionFailed,
            L::HookFailed,
        ],
        lifecycle_name,
        4,
    );
    use PortError as P;
    check_enum(
        &[
            P::SendBufferFull,
            P::ReceiveBufferEmpty,
            P::Timeout,
            P::InvalidMessage,
            P::NotConnected,
        ],
        port_name,
        5,
    );
    use InvokeError as V;
    check_enum(
        &[
            V::InvalidParameters,
            V::InvalidState,
            V::NotSupported,
            V::ProcessingFailed,
        ],
        invoke_name,
        4,
    );
    use CycleError as C;
    check_enum(
        &[
            C::NotReady,
            C::InputUnavailable,
            C::Timeout,
            C::ComputationError,
        ],
        cycle_name,
        4,
    );
    check_enum(
        &[
            RecvError::NoMessage,
            RecvError::Disconnected,
            RecvError::DeserializationFailed,
        ],
        recv_name,
        3,
    );
    check_enum(
        &[
            SendError::Full,
            SendError::Disconnected,
            SendError::SerializationFailed,
        ],
        send_name,
        3,
    );
    use SerializationError as Sn;
    check_enum(
        &[
            Sn::BufferTooSmall,
            Sn::InvalidFormat,
            Sn::EncodingError,
            Sn::DecodingError,
        ],
        serialization_name,
        4,
    );
    use StateMachineError as M;
    check_enum(
        &[
            M::NoValidTransition,
            M::GuardFailed,
            M::ActionFailed,
            M::InvalidState,
            M::InvalidEvent,
            M::InvalidTimeStep,
        ],
        state_machine_name,
        6,
    );
    use SerializeError as Sz;
    check_enum(
        &[
            Sz::BufferTooSmall,
            Sz::InvalidFormat,
            Sz::UnsupportedType,
            Sz::EncodeFailed,
            Sz::DecodeFailed,
        ],
        serialize_name,
        5,
    );
    use ControlError as K;
    check_enum(
        &[
            K::InvalidGain,
            K::InvalidTimeStep,
            K::Overflow,
            K::DivisionByZero,
            K::DimensionMismatch,
        ],
        control_name,
        5,
    );
    use BlockError as B;
    check_enum(
        &[
            B::InvalidInput,
            B::InvalidDelta,
            B::NumericalError,
            B::InvalidConfig,
        ],
        block_name,
        4,
    );
    check_enum(
        &[
            FsmError::EntryFailed,
            FsmError::ExitFailed,
            FsmError::ActionFailed,
        ],
        fsm_name,
        3,
    );
    check_enum(
        &[BufferError::BufferOverflow, BufferError::EncodeError],
        buffer_name,
        2,
    );

    let wrapped = [
        FrameworkError::from(I::HardwareFailed),
        L::HookFailed.into(),
        P::NotConnected.into(),
        M::InvalidEvent.into(),
        Sz::DecodeFailed.into(),
        K::DimensionMismatch.into(),
        V::ProcessingFailed.into(),
        C::ComputationError.into(),
    ];
    let outer: BTreeSet<_> = wrapped.iter().map(|&e| framework_name(e).0).collect();
    assert_eq!(outer.len(), 8, "each wrapper variant reached once");
    for &e in &wrapped {
        let (o, inner) = framework_name(e);
        assert!(!CATCH_ALL_NAMES.contains(&o));
        assert_eq!(format!("{e:?}"), format!("{o}({inner})"));
    }
}

/// Pins the signatures of the library's fallible operations to `Result` by
/// coercing them to explicit function-pointer types: if any returned
/// `Option`, `bool` or a bare value, or panicked instead of reporting, this
/// would not compile (the panic part is covered by the denied `unwrap`,
/// `expect` and `panic` lints checked above). The runtime part exercises the
/// error side of each so the `Err` variants are real outcomes.
///
/// Strength: this covers the named operations, not "all" by construction.
/// Verifies: REQ-PLT-010
#[test]
fn fallible_operations_return_result() {
    type Upd<B> = fn(B, f32, f32) -> Result<(B, f32), BlockError>;
    let _: Upd<LowPassFilter> = <LowPassFilter as ControlBlock>::update;
    let _: Upd<HighPassFilter> = <HighPassFilter as ControlBlock>::update;
    let _: Upd<NotchFilter> = <NotchFilter as ControlBlock>::update;
    let _: Upd<RateLimiter> = <RateLimiter as ControlBlock>::update;
    let _: Upd<Saturator> = <Saturator as ControlBlock>::update;
    type PidUpd = fn(PidController, (f32, f32), f32) -> Result<(PidController, f32), BlockError>;
    let _: PidUpd = <PidController as ControlBlock>::update;
    let _: fn(PidControllerBuilder) -> Result<PidController, BlockError> =
        PidControllerBuilder::build;
    let _: fn(f32, u8) -> Result<Schedule, InitError> = Schedule::from_hz;
    let _: fn(TimedGuard, f32) -> Result<(TimedGuard, bool), StateMachineError> =
        TimedGuard::update;
    type Process = fn(Machine, Ev) -> Result<(Machine, TransitionResult<St>), FsmError>;
    let _: Process = Machine::process;
    let _: fn(LifecycleState, LifecycleState) -> Result<LifecycleState, LifecycleError> =
        LifecycleState::transition_to;
    let _: fn(&mut BoundedVec<u8, 2>, u8) -> Result<(), SerializeError> = BoundedVec::push;
    let _: fn(&mut BoundedString<2>, &str) -> Result<(), SerializeError> = BoundedString::push_str;

    assert_eq!(
        LowPassFilter::new(0.5).update(f32::NAN, 0.01).err(),
        Some(BlockError::InvalidInput)
    );
    assert_eq!(
        Saturator::new(-1.0, 1.0).update(0.0, -1.0).err(),
        Some(BlockError::InvalidDelta)
    );
    assert_eq!(
        PidControllerBuilder::new().build().err(),
        Some(BlockError::InvalidConfig)
    );
    assert_eq!(
        Schedule::from_hz(2000.0, 1).err(),
        Some(InitError::InvalidConfig)
    );
    assert_eq!(
        TimedGuard::new(1.0).update(-1.0).err(),
        Some(StateMachineError::InvalidTimeStep)
    );
    let mut v: BoundedVec<u8, 1> = BoundedVec::new();
    assert_eq!(v.push(1), Ok(()));
    assert_eq!(v.push(2), Err(SerializeError::BufferTooSmall));
    let mut s: BoundedString<2> = BoundedString::new();
    assert_eq!(s.push_str("abc"), Err(SerializeError::BufferTooSmall));
    let mut buf: TelemetryBuffer<1> = TelemetryBuffer::new();
    let msg = middleware::ControlTelemetry {
        timestamp_ms: 1,
        setpoint: 1.0,
        measurement: 0.5,
        output: 0.25,
        error: 0.5,
    };
    assert_eq!(buf.encode(&msg), Err(BufferError::BufferOverflow));
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum St {
    A,
    B,
}
impl State for St {
    type Context = u32;
    fn on_enter(&self, ctx: &mut u32) -> Result<(), LifecycleError> {
        *ctx = ctx.saturating_add(1);
        Ok(())
    }
    fn on_exit(&self, _: &mut u32) -> Result<(), LifecycleError> {
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum Ev {
    Go,
}
impl Event for Ev {
    const NAME: &'static str = "Ev";
}
type Machine = StateMachine<St, Ev, u32, 1>;
static TABLE: [Transition<St, Ev>; 1] = [Transition {
    source: St::A,
    event: Ev::Go,
    target: St::B,
    guard: None,
    action: None,
}];

fn assert_copy<T: Copy>() {}

/// Storage is fixed and inline. Every control block and value type is
/// `Copy`, and a `Copy` type cannot own a heap allocation (it cannot have a
/// `Drop` that frees one). For the capacity-generic containers, `size_of`
/// grows by exactly capacity times element size, which is only true if the
/// elements are stored in the value itself rather than behind a pointer.
/// `DiscreteStateSpace<2, 1, 1>` is exactly its 11 matrix entries of `f32`.
/// `StateMachine`'s size does not depend on the table length `N`: the table
/// is a `&'static` array fixed at compile time, not a growable buffer.
///
/// Strength: covers the listed public types; a new type is not checked
/// until added here (the heap source scan is the general backstop).
/// Verifies: REQ-PLT-003
#[test]
fn data_structures_have_fixed_inline_size() {
    assert_copy::<LowPassFilter>();
    assert_copy::<HighPassFilter>();
    assert_copy::<NotchFilter>();
    assert_copy::<RateLimiter>();
    assert_copy::<Saturator>();
    assert_copy::<PidController>();
    assert_copy::<PidGains>();
    assert_copy::<PidLimits>();
    assert_copy::<DiscreteStateSpace<2, 1, 1>>();
    assert_copy::<FrameworkError>();

    const F: usize = size_of::<f32>();
    assert_eq!(
        size_of::<DiscreteStateSpace<2, 1, 1>>(),
        (4 + 2 + 2 + 1 + 2) * F
    );

    assert_eq!(
        size_of::<TelemetryBuffer<264>>() - size_of::<TelemetryBuffer<8>>(),
        256
    );
    let opt = size_of::<Option<u32>>();
    assert_eq!(
        size_of::<BoundedVec<u32, 64>>() - size_of::<BoundedVec<u32, 8>>(),
        56 * opt
    );
    assert_eq!(
        size_of::<BoundedString<128>>() - size_of::<BoundedString<64>>(),
        64
    );
    assert_eq!(
        size_of::<StateMachine<St, Ev, u32, 1>>(),
        size_of::<StateMachine<St, Ev, u32, 64>>()
    );
}

/// State transitions are by-value transformations. The signature coercions
/// show `ControlBlock::update`, `reset` and `StateMachine::process` consume
/// `self` and return the next instance; the runtime part shows the value
/// kept from before the call is unchanged while the returned one moved on.
///
/// Strength: covers the control-block and state-machine transitions. Some
/// APIs are deliberately `&mut self` (container `push`, `TelemetryBuffer::
/// encode`, `ActiveComponent::run_cycle`, `State` hooks on the context), so
/// the requirement holds for state transitions, not for every mutation.
/// Verifies: REQ-PLT-021
#[test]
fn state_transitions_return_new_instances() {
    let _: fn(LowPassFilter) -> LowPassFilter = <LowPassFilter as ControlBlock>::reset;
    let _: fn(PidController) -> PidController = <PidController as ControlBlock>::reset;

    let before = LowPassFilter::new(0.5);
    let (after, _) = before.update(1.0, 0.01).expect("valid input");
    assert!(before.state().abs() < 1e-6, "original untouched");
    assert!(
        after.state() > 0.25,
        "new instance advanced: {}",
        after.state()
    );

    let pid = PidController::new(
        PidGains::new(0.0, 1.0, 0.0),
        PidLimits::new(-10.0, 10.0, 10.0),
    );
    let (pid2, _) = pid.update((1.0, 0.0), 0.1).expect("valid input");
    assert!(pid.state().integral.abs() < 1e-6);
    assert!(pid2.state().integral > 0.05);
    let reset = pid2.reset();
    assert!(reset.state().integral.abs() < 1e-6);

    let fsm: Machine = StateMachine::new(St::A, 0, &TABLE);
    let kept = fsm.clone();
    let (next, result) = fsm.process(Ev::Go).expect("hooks succeed");
    assert_eq!(
        result,
        TransitionResult::Transitioned {
            from: St::A,
            to: St::B
        }
    );
    assert_eq!(next.current_state(), St::B);
    assert_eq!(*next.context(), 1);
    assert_eq!(kept.current_state(), St::A);
    assert_eq!(*kept.context(), 0);
}
