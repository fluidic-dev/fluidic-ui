//! How schema names become Rust and Go names.

use crate::model::Direction;

/// Written in capitals in Go names, as Go's own style does.
const INITIALISMS: &[&str] = &[
    "api", "cpu", "gpu", "html", "http", "id", "ip", "json", "os", "sdk", "ui", "uri", "url",
    "utf8", "uuid",
];

fn capitalised(part: &str) -> String {
    let mut chars = part.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// `sdk_info` becomes `SdkInfo`.
pub(crate) fn rust_type(name: &str) -> String {
    name.split('_').map(capitalised).collect()
}

/// `sdk_info` becomes `SDKInfo`, and `app_id` becomes `AppID`.
pub(crate) fn go_name(name: &str) -> String {
    name.split('_')
        .map(|p| {
            if INITIALISMS.contains(&p) {
                p.to_ascii_uppercase()
            } else {
                capitalised(p)
            }
        })
        .collect()
}

/// A union variant's or enum value's Go name, from its parent's Go name:
/// `ByeReasonDone`.
pub(crate) fn go_case(parent: &str, case: &str) -> String {
    format!("{parent}{}", go_name(case))
}

/// The Go name of a union's unknown case, from the union's Go name:
/// `ByeReasonUnknown`.
pub(crate) fn go_unknown(union: &str) -> String {
    format!("{union}Unknown")
}

/// The unexported Go functions that read and write a type: `readSDKInfo`
/// and `writeSDKInfo`.
pub(crate) fn go_codec(go_type: &str) -> [String; 2] {
    [format!("read{go_type}"), format!("write{go_type}")]
}

/// Every top-level name the Go runtime declares (`readU32`, `writeFloat`,
/// `jsonReader`…), read from its source so that none is missed.
pub(crate) fn go_runtime_names() -> Vec<String> {
    let runtime = include_str!("../runtime/json.go");
    runtime
        .lines()
        .filter_map(|line| {
            let rest = ["func ", "type ", "const ", "var "]
                .iter()
                .find_map(|k| line.strip_prefix(k))?;
            // A method's name is not a package name.
            if line.starts_with("func (") {
                return None;
            }
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The Go functions every type gets: `EncodeSDKInfo` and `DecodeSDKInfo`.
pub(crate) fn go_functions(go_type: &str) -> [String; 2] {
    [format!("Encode{go_type}"), format!("Decode{go_type}")]
}

/// The union of the messages each side sends: the sender, the union's name,
/// and its doc.
pub(crate) const MESSAGE_UNIONS: [(Direction, &str, &str); 2] = [
    (Direction::App, "AppMessage", "the messages an app sends"),
    (Direction::Host, "HostMessage", "the messages a host sends"),
];

/// Names the generated code declares for itself, besides the message unions,
/// and what they are for.
pub(crate) const GENERATED: &[(&str, &str)] = &[
    ("CONTRACT", "the contract version"),
    ("Contract", "the contract version"),
    ("Canonical", "its codec"),
    ("DecodeError", "its codec"),
    ("EncodeError", "its codec"),
    ("Option", "Rust's own `Option`"),
    ("Some", "Rust's own `Some`"),
    ("None", "Rust's own `None`"),
    ("Result", "Rust's own `Result`"),
    ("Ok", "Rust's own `Ok`"),
    ("Err", "Rust's own `Err`"),
    ("Vec", "Rust's own `Vec`"),
    ("Box", "Rust's own `Box`"),
    ("BTreeMap", "Rust's own `BTreeMap`"),
];

/// Rust's keywords, which a field name must be escaped from.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
    "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Keywords that cannot be raw identifiers either.
const RUST_NOT_RAW: &[&str] = &["self", "crate", "super"];

/// A field's name in Rust. A keyword becomes a raw identifier (`r#type`),
/// or takes a trailing underscore (`self_`) where Rust allows no raw one; no
/// schema name ends in one.
pub(crate) fn rust_field(name: &str) -> String {
    if RUST_NOT_RAW.contains(&name) {
        format!("{name}_")
    } else if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Go's keywords, which no identifier can be.
const GO_KEYWORDS: &[&str] = &[
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
];

/// Whether `name` can be a Go package's name.
pub(crate) fn is_go_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name != "_"
        && !GO_KEYWORDS.contains(&name)
}
