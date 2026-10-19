//! Every name the checker accepts generates Rust and Go that build cleanly.
//!
//! The schema is assembled from awkward names: Rust and Go keywords and
//! predeclared names, the generated code's own identifiers and locals, and
//! every function, type and constant the Go runtime declares. Each name the
//! checker accepts in a position is used there; then the generated Rust must
//! pass clippy with warnings denied, and the generated Go `go vet` and gofmt.
//!
//! It needs cargo and Go, so it is ignored by default; CI runs it with
//! `cargo test -p fluidic-ui-idl --test compile -- --ignored`.

use fluidic_ui_idl::{Source, check};
use std::path::Path;
use std::process::Command;

const RUST_WORDS: &[&str] = &[
    // Keywords, strict and reserved.
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "union",
    // The prelude, derives and common traits.
    "option",
    "some",
    "none",
    "result",
    "ok",
    "err",
    "vec",
    "string",
    "to_string",
    "clone",
    "copy",
    "debug",
    "default",
    "drop",
    "eq",
    "partial_eq",
    "hash",
    "into",
    "from",
    "iter",
    "send",
    "sync",
    "sized",
    "display",
    "formatter",
    "write",
    "read",
    "b_tree_map",
    "std",
    "core",
    // What the generated Rust declares and names.
    "json",
    "writer",
    "reader",
    "tag",
    "object",
    "bare",
    "data",
    "canonical",
    "decode_error",
    "encode_error",
    "version",
    "contract",
    "app_message",
    "host_message",
    "unknown",
    "key",
    "value",
    "name",
    "r",
    "w",
    "encode",
    "decode",
];

const GO_WORDS: &[&str] = &[
    // Keywords.
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    // Predeclared names.
    "any",
    "append",
    "bool",
    "byte",
    "cap",
    "clear",
    "close",
    "comparable",
    "complex",
    "complex64",
    "complex128",
    "copy",
    "delete",
    "error",
    "false",
    "float32",
    "float64",
    "imag",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "iota",
    "len",
    "make",
    "max",
    "min",
    "new",
    "nil",
    "panic",
    "print",
    "println",
    "real",
    "recover",
    "rune",
    "string",
    "true",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    // What the generated Go names.
    "known",
    "seen",
    "ok",
    "err",
    "v",
    "s",
    "items",
    "item",
    "m",
    "next",
    "marker",
];

/// Turns `writeFloat` into `float`, and `jsonReader` into `json_reader`.
fn snake(go: &str) -> String {
    let mut out = String::new();
    for (i, c) in go.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Every top-level name the Go runtime declares, as a schema name, and the
/// type each of its `readX` and `writeX` functions is for.
fn runtime_words() -> Vec<String> {
    let runtime = include_str!("../runtime/json.go");
    let mut words = Vec::new();
    for line in runtime.lines() {
        let Some(rest) = ["func ", "type ", "const ", "var "]
            .iter()
            .find_map(|k| line.strip_prefix(k))
        else {
            continue;
        };
        let rest = if line.starts_with("func (") {
            // A method: the name follows the receiver.
            rest.split_once(") ").map_or("", |(_, name)| name)
        } else {
            rest
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if name.is_empty() {
            continue;
        }
        for prefix in ["read", "write", "encode", "decode"] {
            if let Some(suffix) = name.strip_prefix(prefix).filter(|s| !s.is_empty()) {
                words.push(snake(suffix).trim_start_matches('_').to_string());
            }
        }
        words.push(snake(&name));
    }
    words
}

fn accepts(schema: &str) -> bool {
    check(&[Source::new("probe.idl", format!("contract 0.1\n{schema}"))]).is_ok()
}

/// The awkward schema: each name used wherever the checker accepts it.
fn schema() -> String {
    let mut words: Vec<String> = RUST_WORDS
        .iter()
        .chain(GO_WORDS)
        .map(|w| w.to_string())
        .chain(runtime_words())
        .collect();
    words.sort();
    words.dedup();

    let as_type: Vec<&String> = words
        .iter()
        .filter(|w| accepts(&format!("struct {w} since 0.1 {{}}")))
        .collect();
    let as_message: Vec<&String> = as_type
        .iter()
        .copied()
        .filter(|w| accepts(&format!("message {w} from either since 0.1 {{}}")))
        .collect();
    let as_field: Vec<&String> = words
        .iter()
        .filter(|w| accepts(&format!("struct p since 0.1 {{ {w}: u32 since 0.1 }}")))
        .collect();
    let as_case: Vec<&String> = words
        .iter()
        .filter(|w| accepts(&format!("union p since 0.1 {{ {w} since 0.1 }}")))
        .filter(|w| accepts(&format!("enum p since 0.1 {{ {w} since 0.1 }}")))
        .collect();

    // Half the type names are messages, so they are also message variants.
    // A name is kept only if it does not clash with one kept before it (as
    // `x` and `encode_x` do in Go).
    let mut types = String::new();
    let mut kept: Vec<&String> = Vec::new();
    // `encode` and `decode` are kept back for the unions below.
    let as_type: Vec<&String> = as_type
        .into_iter()
        .filter(|w| *w != "encode" && *w != "decode")
        .collect();
    for (i, w) in as_type.iter().enumerate() {
        let declaration = if i % 2 == 1 && as_message.contains(w) {
            format!("message {w} from either since 0.1 {{}}\n")
        } else {
            format!("struct {w} since 0.1 {{}}\n")
        };
        if accepts(&format!("{types}{declaration}")) {
            types += &declaration;
            kept.push(w);
        }
    }
    // Go writes a case of `encode` as `EncodeX`, which is also type `x`'s
    // encode function: each type name is tried as a case of both.
    for (kind, parent) in [("enum", "encode"), ("union", "decode")] {
        let mut cases = String::new();
        for w in &kept {
            let case = format!("    {w} since 0.1\n");
            if accepts(&format!(
                "{types}{kind} {parent} since 0.1 {{\n{cases}{case}}}\n"
            )) {
                cases += &case;
            }
        }
        if !cases.is_empty() {
            types += &format!("{kind} {parent} since 0.1 {{\n{cases}}}\n");
        }
    }
    let mut s = format!("contract 0.1\n{types}");
    // Each field type in turn: required and optional, scalars, containers,
    // a union and an enum. The union carries this struct, so it is held
    // here only through a list or map.
    let field_types = [
        "u32",
        "optional f64",
        "list<probe_union>",
        "optional map<string, probe_union>",
        "map<string, probe_enum>",
        "optional probe_enum",
        "optional list<string>",
        "string",
    ];
    s += "struct probe_fields since 0.1 {\n";
    for (i, w) in as_field.iter().enumerate() {
        s += &field(w, field_types[i % field_types.len()]);
    }
    s += "}\n";
    // A union's inline fields cannot hold the union itself, so these hold
    // scalars and the enum.
    let inline_types = ["u32", "optional u32", "probe_enum", "optional string"];
    s += "union probe_union since 0.1 {\n    probe_inline {\n";
    for (i, w) in as_field.iter().enumerate() {
        s += &field(w, inline_types[i % inline_types.len()]);
    }
    s += "    } since 0.1\n    probe_carried(probe_fields) since 0.1\n";
    for w in &as_case {
        s += &format!("    {w} since 0.1\n");
    }
    s += "}\nstruct probe_optional_union since 0.1 {\n    optional u: probe_union since 0.1\n}\n";
    s += "enum probe_enum since 0.1 {\n";
    for w in &as_case {
        s += &format!("    {w} since 0.1\n");
    }
    s += "}\n";
    with_docs(&s)
}

/// Docs to try on every declaration, field, variant and value: prose with
/// awkward characters, and every Markdown or Go structure that has broken
/// generated Rust or Go, some of it spread over lines. The checker should
/// refuse the structures; any doc it accepts goes into the schema, so one it
/// wrongly accepts fails the build.
const DOCS: &[&[&str]] = &[
    &["", "Prose with empty lines around it.", ""],
    &[
        "Awkward prose: `code`, *emphasis*, a | pipe, */ and /*, <b>html</b>, [a link](https://example.com), # mid-line.",
    ],
    &["Non-ASCII: naïve, 日本語, emoji 😀; trailing spaces follow.   "],
    &["a \\| b, a - b, x > y, 1.5, 2026 and #hashtag are prose."],
    &["*emphasis* first, then `code`; a | b on its own; x - y = z."],
    &["Intro.", "", "Overview", "", "More text."],
    &["a | b", "--|--"],
    &["a | b", ":-:|:-:"],
    &["Title", "=="],
    &["Title", "="],
    &["Title", "---"],
    &["_ _ _"],
    &["see[^1] now"],
    &["[^1]: a footnote"],
    &["  indented prose"],
    &["    four spaces"],
    &["\tA tab"],
    &["  * item"],
    &["- item"],
    &["* item"],
    &["+ item"],
    &["1. item"],
    &["> quote"],
    &["> ```"],
    &["- ```"],
    &[">     code"],
    &["```"],
    &["~~~"],
    &["# heading"],
    &["[x]: https://example.com"],
    &["- [x]: https://example.com"],
    &["> [y]: https://example.com"],
    &["| a | b |"],
    &["<!-- unclosed"],
    &["---"],
    &["==="],
];

/// The docs the checker accepts, as one doc: their lines in turn, with an
/// empty line before and after, which the checker drops.
fn accepted_doc_lines() -> String {
    let as_doc = |lines: &[&str]| {
        lines
            .iter()
            .map(|l| format!("/// {l}\n"))
            .collect::<String>()
    };
    let mut doc = as_doc(&[""]);
    for lines in DOCS {
        if accepts(&format!("{}struct p since 0.1 {{}}", as_doc(lines))) {
            let first = lines
                .iter()
                .position(|l| !l.is_empty())
                .unwrap_or(lines.len());
            let last = lines
                .iter()
                .rposition(|l| !l.is_empty())
                .map_or(first, |i| i + 1);
            doc += &as_doc(&lines[first..last]);
        }
    }
    doc += &as_doc(&[""]);
    assert!(
        accepts(&format!("{doc}struct p since 0.1 {{}}")),
        "the accepted docs are refused together:\n{doc}"
    );
    doc
}

/// Puts the accepted doc lines before every line that declares something.
fn with_docs(schema: &str) -> String {
    let doc = accepted_doc_lines();
    let mut out = String::new();
    for line in schema.lines() {
        if !line.trim_start().starts_with('}') {
            out += &doc;
        }
        out += line;
        out.push('\n');
    }
    out
}

fn field(name: &str, ty: &str) -> String {
    match ty.strip_prefix("optional ") {
        Some(ty) => format!("    optional {name}: {ty} since 0.1\n"),
        None => format!("    {name}: {ty} since 0.1\n"),
    }
}

fn run(dir: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", dir.join("../target"))
        .output()
        .unwrap_or_else(|e| panic!("cannot run {program}: {e}"));
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{program} {args:?} failed:\n{text}"
    );
    text
}

#[test]
#[ignore = "needs Go; CI runs it with --ignored"]
fn awkward_names_generate_code_that_builds() {
    let schema = schema();
    if let Err(diagnostics) = check(&[Source::new("probe.idl", schema.clone())]) {
        let lines: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
        panic!(
            "the assembled schema is refused:\n{}\n\n{schema}",
            lines.join("\n")
        );
    }

    let root = std::env::temp_dir().join(format!("fluidic-ui-idl-compile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["tools/idl", "schema", "rust/src", "go"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(root.join("schema/probe.idl"), &schema).unwrap();
    std::fs::write(
        root.join("tools/idl/targets"),
        "schema=schema rust=rust/src go=go go_package=probe\n",
    )
    .unwrap();
    std::fs::write(
        root.join("rust/Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    // The Go version the idl tool's own sample module requires.
    let sample = include_str!("../sample/go/go.mod");
    let go = sample
        .lines()
        .find(|l| l.starts_with("go "))
        .expect("a go line");
    std::fs::write(root.join("go/go.mod"), format!("module probe\n\n{go}\n")).unwrap();

    let generate = Command::new(env!("CARGO_BIN_EXE_fluidic-ui-idl"))
        .args(["generate", "--root"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        generate.status.success(),
        "{}\n\n{schema}",
        String::from_utf8_lossy(&generate.stderr)
    );

    run(
        &root.join("rust"),
        "cargo",
        &["clippy", "--quiet", "--all-targets", "--", "-D", "warnings"],
    );
    // A code block in a doc comment would run as a test.
    run(&root.join("rust"), "cargo", &["test", "--quiet", "--doc"]);
    run(&root.join("go"), "go", &["vet", "./..."]);
    let unformatted = run(&root.join("go"), "gofmt", &["-l", "."]);
    assert!(
        unformatted.trim().is_empty(),
        "gofmt would change: {unformatted}"
    );
}
