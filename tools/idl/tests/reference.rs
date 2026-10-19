//! The message reference lists every message, type, field, variant and
//! value, with its version and doc comment.

use fluidic_ui_idl::{Source, Target, check, generate};
use std::path::PathBuf;

fn reference(text: &str) -> String {
    let schema = check(&[Source::new("s.idl", text)]).expect("valid schema");
    let target = Target {
        schema: PathBuf::from("s"),
        rust: None,
        go: None,
        reference: Some(PathBuf::from("reference.md")),
    };
    let mut files = generate(&schema, &target).expect("generated");
    assert_eq!(files.len(), 1);
    files.remove(0).1
}

const SCHEMA: &str = "\
/// A small contract.
contract 0.2

/// The app's first message.
message hello from app since 0.1 {
    /// The versions the app speaks.
    versions: list<version> since 0.1
    optional title: string since 0.2
}

message bye from either since 0.1 {
    reason: reason since 0.1
}

/// Why it ended.
union reason since 0.1 {
    /// Finished.
    done since 0.1
    /// No version in common.
    no_common_version {
        host_versions: list<version> since 0.1
    } since 0.1
    crashed(info) since 0.2
}

struct info since 0.1 {
    /// Two lines
    /// of doc, with a | in them.
    by_name: map<string, info> since 0.1
}

enum side since 0.1 {
    start since 0.1
    /// The end.
    end since 0.2
}
";

#[test]
fn lists_messages_with_their_sender_fields_and_versions() {
    let text = reference(SCHEMA);
    for line in [
        "Contract 0.2. A small contract.",
        "### `hello`",
        "From the app, since 0.1. The app's first message.",
        "| `versions` | `list<version>` | 0.1 | The versions the app speaks. |",
        "| `title` | optional `string` | 0.2 | |",
        "### `bye`",
        "From either side, since 0.1.",
        "| `reason` | `reason` | 0.1 | |",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing line {line:?} in:\n{text}"
        );
    }
}

#[test]
fn lists_types_with_their_variants_values_and_docs() {
    let text = reference(SCHEMA);
    for line in [
        "### `reason` (union)",
        "Since 0.1. Why it ended.",
        "| `done` | nothing | 0.1 | Finished. |",
        "| `no_common_version` | the fields below | 0.1 | No version in common. |",
        "| `crashed` | `info` | 0.2 | |",
        "#### `reason.no_common_version`",
        "| `host_versions` | `list<version>` | 0.1 | |",
        "### `info` (struct)",
        "| `by_name` | `map<string, info>` | 0.1 | Two lines of doc, with a \\| in them. |",
        "### `side` (enum)",
        "| `end` | 0.2 | The end. |",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing line {line:?} in:\n{text}"
        );
    }
}

#[test]
fn messages_come_before_other_types() {
    let text = reference(SCHEMA);
    let at = |s: &str| text.find(s).unwrap_or_else(|| panic!("{s} missing"));
    assert!(at("### `bye`") < at("### `reason` (union)"));
}

#[test]
fn a_doc_with_backslashes_and_pipes_stays_in_its_cell() {
    let text =
        reference("contract 0.1\nstruct s since 0.1 {\n    /// a \\| b\n    x: u32 since 0.1\n}\n");
    let line = r"| `x` | `u32` | 0.1 | a \\\| b |";
    assert!(
        text.lines().any(|l| l == line),
        "missing line {line:?} in:\n{text}"
    );
}
