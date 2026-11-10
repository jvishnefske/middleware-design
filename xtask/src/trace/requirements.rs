//! Line-oriented parser for requirement lines in `design.md`.
//!
//! A requirement is a Markdown task-list line whose text begins with a bold
//! requirement ID:
//!
//! - `- [x] **REQ-TYP-001**: text` (ticked, no ASIL tag)
//! - `- [ ] **REQ-TYP-001** [ASIL B]: text` (open, tagged)
//!
//! The checkbox may be `[x]`, `[X]` or `[ ]`, the bullet `-` or `*`, and the
//! line may be indented. The optional tag sits between the closing `**` and
//! the colon; the valid tags are `[ASIL A]` to `[ASIL D]` and `[QM]`. Any
//! other bracketed text is recorded as [`AsilTag::Invalid`].
//!
//! Every other line is ignored, including tables such as the hand-written
//! traceability matrix that mention IDs in cells, and task-list lines that do
//! not start with `**REQ-`. A task-list line that does start with `**REQ-`
//! but is otherwise malformed (bad ID, unclosed bold, no colon) yields a
//! [`Problem::MalformedRequirement`] rather than being silently skipped.

use super::{Asil, AsilTag, Problem, ReqId, Requirement, Status};

/// Parses all requirement lines in `design`, returning the requirements in
/// file order and any malformed-line problems.
pub fn parse(design: &str) -> (Vec<Requirement>, Vec<Problem>) {
    design
        .lines()
        .enumerate()
        .filter_map(|(idx, text)| parse_line(idx.saturating_add(1), text))
        .fold(
            (Vec::new(), Vec::new()),
            |(mut reqs, mut problems), parsed| {
                match parsed {
                    Ok(req) => reqs.push(req),
                    Err(problem) => problems.push(problem),
                }
                (reqs, problems)
            },
        )
}

/// Parses one line. `None` means "not a requirement line"; `Some(Err)` means
/// it claims to be one but is malformed.
fn parse_line(line: usize, text: &str) -> Option<Result<Requirement, Problem>> {
    let (status, rest) = checkbox(text.trim_start())?;
    let bold = rest.strip_prefix("**REQ-")?;
    Some(parse_body(line, status, bold))
}

/// Recognises the bullet and checkbox prefix.
fn checkbox(text: &str) -> Option<(Status, &str)> {
    let rest = text
        .strip_prefix("- ")
        .or_else(|| text.strip_prefix("* "))?;
    let (status, rest) = if let Some(r) = rest.strip_prefix("[x]") {
        (Status::Ticked, r)
    } else if let Some(r) = rest.strip_prefix("[X]") {
        (Status::Ticked, r)
    } else {
        (Status::Open, rest.strip_prefix("[ ]")?)
    };
    Some((status, rest.trim_start()))
}

/// Parses everything after `**REQ-`.
fn parse_body(line: usize, status: Status, after_prefix: &str) -> Result<Requirement, Problem> {
    let malformed = |reason: &str| Problem::MalformedRequirement {
        line,
        reason: reason.to_owned(),
    };
    let (id_tail, after_bold) = after_prefix
        .split_once("**")
        .ok_or_else(|| malformed("unclosed bold around requirement ID"))?;
    let id_text = format!("REQ-{id_tail}");
    let id = ReqId::parse(&id_text)
        .ok_or_else(|| malformed(&format!("invalid requirement ID {id_text:?}")))?;

    let after_bold = after_bold.trim_start();
    let (asil, after_tag) = match after_bold.strip_prefix('[') {
        Some(tagged) => {
            let (inner, tail) = tagged
                .split_once(']')
                .ok_or_else(|| malformed("unclosed '[' in ASIL tag"))?;
            let tag = Asil::parse_tag(inner)
                .map_or_else(|| AsilTag::Invalid(inner.to_owned()), AsilTag::Valid);
            (tag, tail.trim_start())
        }
        None => (AsilTag::Missing, after_bold),
    };
    let text = after_tag
        .strip_prefix(':')
        .ok_or_else(|| malformed("expected ':' after requirement ID or ASIL tag"))?;

    Ok(Requirement {
        id,
        asil,
        status,
        line,
        text: text.trim().to_owned(),
    })
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

    const DESIGN: &str = "\
# Design

Requirements are tracked with REQ-xxx identifiers.

## 1 Types
- [x] **REQ-TYP-001**: Untagged ticked requirement
- [ ] **REQ-TYP-002** [ASIL B]: Tagged open requirement
- [X] **REQ-TYP-003** [QM]: Upper-case tick, QM
  - [x] **REQ-TYP-004** [ASIL D]:   Indented, padded text
* [ ] **REQ-TYP-005** [ASIL A]: Star bullet
- [x] A checklist item without an ID
- plain bullet mentioning **REQ-TYP-099**: not a task

## Traceability Matrix

| Artifact | Requirements |
|----------|--------------|
| src/types/unit.rs | REQ-TYP-001, REQ-TYP-002 |
| - [x] **REQ-TYP-098**: inside a table cell | x |
";

    #[test]
    fn parses_tagged_untagged_ticked_and_open() {
        let (reqs, problems) = parse(DESIGN);
        assert!(problems.is_empty(), "{problems:?}");
        let summary: Vec<(&str, AsilTag, Status, usize)> = reqs
            .iter()
            .map(|r| (r.id.as_str(), r.asil.clone(), r.status, r.line))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("REQ-TYP-001", AsilTag::Missing, Status::Ticked, 6),
                ("REQ-TYP-002", AsilTag::Valid(Asil::B), Status::Open, 7),
                ("REQ-TYP-003", AsilTag::Valid(Asil::Qm), Status::Ticked, 8),
                ("REQ-TYP-004", AsilTag::Valid(Asil::D), Status::Ticked, 9),
                ("REQ-TYP-005", AsilTag::Valid(Asil::A), Status::Open, 10),
            ]
        );
        assert_eq!(reqs[0].text, "Untagged ticked requirement");
        assert_eq!(reqs[3].text, "Indented, padded text");
    }

    #[test]
    fn traceability_matrix_table_is_ignored() {
        let (reqs, _) = parse(DESIGN);
        assert!(reqs.iter().all(|r| r.id.as_str() != "REQ-TYP-098"));
        assert!(reqs.iter().all(|r| r.id.as_str() != "REQ-TYP-099"));
    }

    #[test]
    fn invalid_tag_is_recorded() {
        let (reqs, problems) = parse("- [x] **REQ-A-1** [ASIL E]: x\n");
        assert!(problems.is_empty());
        assert_eq!(reqs[0].asil, AsilTag::Invalid("ASIL E".to_owned()));
    }

    #[test]
    fn malformed_lines_are_problems() {
        let design = "\
- [x] **REQ-bad-1**: lower-case area
- [x] **REQ-A-1 unclosed bold
- [x] **REQ-A-2** no colon
- [x] **REQ-A-3** [ASIL B no close
";
        let (reqs, problems) = parse(design);
        assert!(reqs.is_empty());
        let lines: Vec<usize> = problems
            .iter()
            .map(|p| match p {
                Problem::MalformedRequirement { line, .. } => *line,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(lines, vec![1, 2, 3, 4]);
    }

    #[test]
    fn duplicates_are_both_returned() {
        let (reqs, _) = parse("- [x] **REQ-A-1** [QM]: a\n- [ ] **REQ-A-1** [QM]: b\n");
        assert_eq!(reqs.len(), 2);
    }
}
