//! The rules that span declarations: what makes parsed files one schema.

use crate::Diagnostic;
use crate::model::{Field, Payload, Schema, Span, Type, TypeDecl, TypeKind, Version};
use crate::naming;
use std::collections::{HashMap, HashSet, VecDeque};

/// Names a type cannot take: they are read where a type is, and `self`
/// becomes Rust's `Self`.
const TYPE_RESERVED: &[&str] = &[
    "contract", "message", "struct", "union", "enum", "from", "since", "optional", "list", "map",
    "bool", "u32", "i32", "u64", "i64", "f32", "f64", "string", "version", "self",
];

/// Names a variant or enum value cannot take: the generated unknown case's,
/// and `self`, which becomes Rust's `Self`.
const CASE_RESERVED: &[&str] = &["unknown", "self"];

/// Names a field cannot take: the generated unknown case's. A field named
/// `self` is `self_` in Rust.
const FIELD_RESERVED: &[&str] = &["unknown"];

fn is_snake_case(name: &str) -> bool {
    let mut parts = name.split('_');
    let first = parts.next().unwrap_or_default();
    first.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && parts.all(|p| !p.is_empty())
}

/// Collects diagnostics, and checks names within one scope.
struct Names<'a> {
    seen: HashMap<&'a str, &'a Span>,
    reserved: &'static [&'static str],
}

impl<'a> Names<'a> {
    fn new(reserved: &'static [&'static str]) -> Self {
        Names {
            seen: HashMap::new(),
            reserved,
        }
    }

    fn declare(&mut self, name: &'a str, span: &'a Span, out: &mut Vec<Diagnostic>) {
        if !is_snake_case(name) {
            out.push(error(span, format!("`{name}` is not a snake_case name")));
        } else if self.reserved.contains(&name) {
            out.push(error(span, format!("`{name}` is reserved")));
        }
        if let Some(first) = self.seen.insert(name, span) {
            out.push(error(
                span,
                format!(
                    "`{name}` is declared again; it was declared at {}",
                    at(first)
                ),
            ));
            self.seen.insert(name, first);
        }
    }
}

fn named(ty: &Type) -> Option<&str> {
    match ty {
        Type::List(t) | Type::Map(t) => named(t),
        Type::Named(n) => Some(n),
        _ => None,
    }
}

fn error(span: &Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        span: span.clone(),
        message: message.into(),
    }
}

fn at(span: &Span) -> String {
    format!("{}:{}:{}", span.path, span.line, span.column)
}

pub(crate) fn schema(
    contracts: Vec<(Version, Vec<String>, Span)>,
    types: Vec<TypeDecl>,
    start: Span,
) -> Result<Schema, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut contracts = contracts.into_iter();
    let Some((contract, doc, first)) = contracts.next() else {
        return Err(vec![error(
            &start,
            "the schema has no `contract` declaration",
        )]);
    };
    for (_, _, span) in contracts {
        diagnostics.push(error(
            &span,
            format!(
                "`contract` is declared again; it was declared at {}",
                at(&first)
            ),
        ));
    }
    let mut type_names = Names::new(TYPE_RESERVED);
    for t in &types {
        type_names.declare(&t.name, &t.span, &mut diagnostics);
        let problem = match &t.kind {
            TypeKind::Message { .. } if t.name == "unknown" => Some(
                "a message cannot be named `unknown`: the message unions have an unknown case"
                    .to_string(),
            ),
            TypeKind::Union { variants } if variants.is_empty() => {
                Some(format!("union `{}` has no variants", t.name))
            }
            TypeKind::Enum { values } if values.is_empty() => {
                Some(format!("enum `{}` has no values", t.name))
            }
            _ => None,
        };
        if let Some(problem) = problem {
            diagnostics.push(error(&t.span, problem));
        }
    }
    let kinds: HashMap<&str, &TypeKind> =
        types.iter().map(|t| (t.name.as_str(), &t.kind)).collect();
    let since = |name: &str,
                 since: Version,
                 parent: (&str, Version),
                 span: &Span,
                 out: &mut Vec<Diagnostic>| {
        if since < parent.1 {
            out.push(error(
                span,
                format!(
                    "`{name}` is since {since}, earlier than `{}` (since {})",
                    parent.0, parent.1
                ),
            ));
        } else if since > contract {
            out.push(error(
                span,
                format!("`{name}` is since {since}, later than the contract ({contract})"),
            ));
        }
    };
    let fields = |parent: (&str, Version), fields: &[Field], out: &mut Vec<Diagnostic>| {
        let mut names = Names::new(FIELD_RESERVED);
        for f in fields {
            names.declare(&f.name, &f.span, out);
            since(&f.name, f.since, parent, &f.span, out);
            if f.since > parent.1 && !f.optional {
                out.push(error(
                    &f.span,
                    format!(
                        "field `{}` is added after `{}` (since {}), so it must be optional",
                        f.name, parent.0, parent.1
                    ),
                ));
            }
            if let Some(n) = named(&f.ty).filter(|n| !kinds.contains_key(n)) {
                out.push(error(
                    &f.span,
                    format!("field `{}` names `{n}`, which is not declared", f.name),
                ));
            }
        }
    };
    let root = ("contract", Version { major: 0, minor: 0 });
    for t in &types {
        since(&t.name, t.since, root, &t.span, &mut diagnostics);
        let parent = (t.name.as_str(), t.since);
        match &t.kind {
            TypeKind::Message { fields: f, .. } | TypeKind::Struct { fields: f } => {
                fields(parent, f, &mut diagnostics);
            }
            TypeKind::Union { variants } => {
                let mut names = Names::new(CASE_RESERVED);
                let mut carried_by: HashMap<&str, &crate::model::Variant> = HashMap::new();
                for v in variants {
                    if let Payload::Struct(carried) = &v.payload {
                        match carried_by.get(carried.as_str()) {
                            Some(first) => diagnostics.push(error(
                                &v.span,
                                format!(
                                    "variant `{}` carries `{carried}`, which `{}` at {} already carries",
                                    v.name,
                                    first.name,
                                    at(&first.span)
                                ),
                            )),
                            None => {
                                carried_by.insert(carried, v);
                            }
                        }
                    }
                    names.declare(&v.name, &v.span, &mut diagnostics);
                    since(&v.name, v.since, parent, &v.span, &mut diagnostics);
                    match &v.payload {
                        Payload::None => {}
                        Payload::Fields(f) => fields((&v.name, v.since), f, &mut diagnostics),
                        Payload::Struct(carried) => match kinds.get(carried.as_str()) {
                            Some(TypeKind::Struct { .. }) => {}
                            Some(kind) => diagnostics.push(error(
                                &v.span,
                                format!(
                                    "variant `{}` carries `{carried}`, which is {} {}, not a struct",
                                    v.name,
                                    if kind.keyword() == "enum" { "an" } else { "a" },
                                    kind.keyword()
                                ),
                            )),
                            None => diagnostics.push(error(
                                &v.span,
                                format!("variant `{}` carries `{carried}`, which is not declared", v.name),
                            )),
                        },
                    }
                }
            }
            TypeKind::Enum { values } => {
                let mut names = Names::new(CASE_RESERVED);
                for v in values {
                    names.declare(&v.name, &v.span, &mut diagnostics);
                    since(&v.name, v.since, parent, &v.span, &mut diagnostics);
                }
            }
        }
    }
    recursion(&types, &mut diagnostics);
    collisions(&types, &mut diagnostics);
    diagnostics.sort_by(|a, b| {
        (&a.span.path, a.span.line, a.span.column).cmp(&(&b.span.path, b.span.line, b.span.column))
    });
    if diagnostics.is_empty() {
        Ok(Schema {
            contract,
            doc,
            types,
        })
    } else {
        Err(diagnostics)
    }
}

/// The types a value of `t` holds directly, not through a list or map.
fn direct(t: &TypeDecl) -> Vec<&str> {
    let fields = t.kind.field_lists().into_iter().flatten();
    let named = fields.filter_map(|f| match &f.ty {
        Type::Named(n) => Some(n.as_str()),
        _ => None,
    });
    named.chain(t.kind.carried()).collect()
}

/// Plain Rust and Go types cannot hold themselves except through a list or map.
fn recursion(types: &[TypeDecl], out: &mut Vec<Diagnostic>) {
    let by_name: HashMap<&str, &TypeDecl> = types.iter().map(|t| (t.name.as_str(), t)).collect();
    let mut reported: HashSet<&str> = HashSet::new();
    for t in types {
        if reported.contains(t.name.as_str()) {
            continue;
        }
        // Shortest way back to `t`, breadth first.
        let mut previous: HashMap<&str, &str> = HashMap::new();
        let mut queue = VecDeque::from([t.name.as_str()]);
        let mut found = false;
        'search: while let Some(at) = queue.pop_front() {
            for next in by_name.get(at).map(|d| direct(d)).unwrap_or_default() {
                if next == t.name {
                    previous.insert(t.name.as_str(), at);
                    found = true;
                    break 'search;
                }
                if !previous.contains_key(next) && by_name.contains_key(next) {
                    previous.insert(next, at);
                    queue.push_back(next);
                }
            }
        }
        if !found {
            continue;
        }
        let mut between = Vec::new();
        let mut at = previous[t.name.as_str()];
        while at != t.name {
            between.push(at);
            at = previous[at];
        }
        between.reverse();
        reported.insert(&t.name);
        reported.extend(between.iter().copied());
        let through = if between.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = between.iter().map(|n| format!("`{n}`")).collect();
            format!(" through {}", names.join(", "))
        };
        out.push(error(
            &t.span,
            format!(
                "`{}` contains itself{through} without a list or map between",
                t.name
            ),
        ));
    }
}

/// What the generator declares for itself, by name, and what for.
fn generated_names() -> HashMap<String, String> {
    let mut names: HashMap<String, String> = naming::GENERATED
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_string()))
        .collect();
    for (_, union, doc) in naming::MESSAGE_UNIONS {
        let doc = doc.to_string();
        names.insert(union.to_string(), doc.clone());
        names.insert(naming::go_unknown(union), doc.clone());
        for function in naming::go_functions(union) {
            names.insert(function, doc.clone());
        }
    }
    for runtime in naming::go_runtime_names() {
        names.insert(runtime, "its codec".to_string());
    }
    names
}

/// A schema name, where it was written.
#[derive(Clone, Copy)]
struct Origin<'a> {
    name: &'a str,
    span: &'a Span,
    /// The type a variant, value or field belongs to; empty for a type.
    scope: &'a str,
}

/// A name the generated code declares, and the schema name it comes from.
struct Claim<'a> {
    generated: String,
    origin: Origin<'a>,
    /// Whether the name was refused as reserved where it was written, which
    /// is report enough.
    reserved: bool,
}

impl<'a> Claim<'a> {
    /// `reserved` lists the names refused where this one was written.
    fn new(
        generated: String,
        name: &'a str,
        span: &'a Span,
        scope: &'a str,
        reserved: &[&str],
    ) -> Self {
        let origin = Origin { name, span, scope };
        Claim {
            generated,
            origin,
            reserved: reserved.contains(&name),
        }
    }
}

/// One scope of generated names, and where each was first taken.
struct Namespace<'a> {
    lang: &'static str,
    taken: HashMap<String, Origin<'a>>,
}

impl<'a> Namespace<'a> {
    fn new(lang: &'static str) -> Self {
        Namespace {
            lang,
            taken: HashMap::new(),
        }
    }

    fn claim(&mut self, claim: &Claim<'a>, out: &mut Vec<Diagnostic>) {
        let Origin { name, span, scope } = claim.origin;
        let generated = &claim.generated;
        match self.taken.get(generated) {
            // The same name twice in one scope is reported as declared again.
            Some(first) if first.name == name && first.scope == scope => {}
            Some(first) => out.push(error(
                span,
                format!(
                    "`{name}` becomes the {} name `{generated}`, which `{}` at {} also becomes",
                    self.lang,
                    first.name,
                    at(first.span)
                ),
            )),
            None => {
                self.taken.insert(generated.clone(), claim.origin);
            }
        }
    }
}

/// Reports claims on a name the generated code declares for itself, once per
/// name written, though it may clash several ways.
fn generated_clashes<'a>(
    generated: &HashMap<String, String>,
    claims: impl Iterator<Item = &'a Claim<'a>>,
    out: &mut Vec<Diagnostic>,
) {
    let mut reported: Vec<&Span> = Vec::new();
    for claim in claims.filter(|c| !c.reserved) {
        let (own, Origin { name, span, .. }) = (&claim.generated, claim.origin);
        if let Some(purpose) = generated.get(own)
            && !reported.contains(&span)
        {
            out.push(error(
                span,
                format!(
                    "`{name}` becomes the name `{own}`, which the generator uses for {purpose}"
                ),
            ));
            reported.push(span);
        }
    }
}

/// Two schema names must not become one Rust or Go name, nor a name the
/// generator declares for itself.
fn collisions(types: &[TypeDecl], out: &mut Vec<Diagnostic>) {
    let generated = generated_names();
    let mut rust = Namespace::new("Rust");
    let mut go = Namespace::new("Go");
    for t in types {
        let go_type = naming::go_name(&t.name);
        let type_claim = |generated| Claim::new(generated, &t.name, &t.span, "", TYPE_RESERVED);
        let rust_type = type_claim(naming::rust_type(&t.name));

        // Go declares these in the package's one namespace.
        let mut package: Vec<Claim> = std::iter::once(go_type.clone())
            .chain(naming::go_functions(&go_type))
            .chain(naming::go_codec(&go_type))
            .map(type_claim)
            .collect();
        if matches!(t.kind, TypeKind::Union { .. }) {
            package.push(type_claim(naming::go_unknown(&go_type)));
        }
        let cases = t.kind.cases();
        for case in cases.iter().filter(|c| !c.carries_struct) {
            let go_case = naming::go_case(&go_type, case.name);
            package.push(Claim::new(
                go_case,
                case.name,
                case.span,
                &t.name,
                CASE_RESERVED,
            ));
        }

        generated_clashes(&generated, std::iter::once(&rust_type).chain(&package), out);
        rust.claim(&rust_type, out);
        for claim in &package {
            go.claim(claim, out);
        }

        // Rust names a union's variants and an enum's values inside it.
        let mut rust_cases = Namespace::new("Rust");
        for case in &cases {
            let rust_case = naming::rust_type(case.name);
            let claim = Claim::new(rust_case, case.name, case.span, &t.name, CASE_RESERVED);
            rust_cases.claim(&claim, out);
        }

        // Go names fields inside their struct.
        for fields in t.kind.field_lists() {
            let mut members = Namespace::new("Go");
            for f in fields {
                let claim = Claim::new(
                    naming::go_name(&f.name),
                    &f.name,
                    &f.span,
                    &t.name,
                    FIELD_RESERVED,
                );
                members.claim(&claim, out);
            }
        }
    }
}
