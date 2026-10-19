//! The checked schema: what the generators read.

use std::fmt;

/// A contract version, written `MAJOR.MINOR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Where something was written: a file, a line and a column, both from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub contract: Version,
    /// Doc comment on the `contract` declaration.
    pub doc: Vec<String>,
    /// Every declared type, in file order (files sorted by path).
    pub types: Vec<TypeDecl>,
}

impl Schema {
    pub fn get(&self, name: &str) -> Option<&TypeDecl> {
        self.types.iter().find(|t| t.name == name)
    }

    /// The variants of the union of messages `sender` sends: each carries
    /// its message, in declaration order.
    pub fn message_variants(&self, sender: Direction) -> Vec<Variant> {
        self.types
            .iter()
            .filter(|t| match &t.kind {
                TypeKind::Message { from, .. } => *from == sender || *from == Direction::Either,
                _ => false,
            })
            .map(|m| Variant {
                name: m.name.clone(),
                doc: m.doc.clone(),
                payload: Payload::Struct(m.name.clone()),
                since: m.since,
                span: m.span.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name: String,
    pub doc: Vec<String>,
    pub since: Version,
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Message { from: Direction, fields: Vec<Field> },
    Struct { fields: Vec<Field> },
    Union { variants: Vec<Variant> },
    Enum { values: Vec<EnumValue> },
}

impl TypeKind {
    pub fn keyword(&self) -> &'static str {
        match self {
            TypeKind::Message { .. } => "message",
            TypeKind::Struct { .. } => "struct",
            TypeKind::Union { .. } => "union",
            TypeKind::Enum { .. } => "enum",
        }
    }

    /// Every list of fields: a message's or struct's, and each union
    /// variant's inline fields.
    pub fn field_lists(&self) -> Vec<&[Field]> {
        match self {
            TypeKind::Message { fields, .. } | TypeKind::Struct { fields } => vec![fields],
            TypeKind::Union { variants } => variants
                .iter()
                .filter_map(|v| match &v.payload {
                    Payload::Fields(f) => Some(f.as_slice()),
                    _ => None,
                })
                .collect(),
            TypeKind::Enum { .. } => Vec::new(),
        }
    }

    /// A union's variants or an enum's values.
    pub fn cases(&self) -> Vec<Case<'_>> {
        match self {
            TypeKind::Union { variants } => variants
                .iter()
                .map(|v| Case {
                    name: &v.name,
                    span: &v.span,
                    carries_struct: matches!(v.payload, Payload::Struct(_)),
                })
                .collect(),
            TypeKind::Enum { values } => values
                .iter()
                .map(|v| Case {
                    name: &v.name,
                    span: &v.span,
                    carries_struct: false,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The structs a union's variants carry.
    pub fn carried(&self) -> Vec<&str> {
        match self {
            TypeKind::Union { variants } => variants
                .iter()
                .filter_map(|v| match &v.payload {
                    Payload::Struct(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Which side sends a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    App,
    Host,
    Either,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Direction::App => "app",
            Direction::Host => "host",
            Direction::Either => "either",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub doc: Vec<String>,
    pub optional: bool,
    pub ty: Type,
    pub since: Version,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bool,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    String,
    Version,
    List(Box<Type>),
    /// A map from string keys to this type.
    Map(Box<Type>),
    Named(String),
}

impl Type {
    pub fn scalar(name: &str) -> Option<Type> {
        Some(match name {
            "bool" => Type::Bool,
            "u32" => Type::U32,
            "i32" => Type::I32,
            "u64" => Type::U64,
            "i64" => Type::I64,
            "f32" => Type::F32,
            "f64" => Type::F64,
            "string" => Type::String,
            "version" => Type::Version,
            _ => return None,
        })
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Bool => f.write_str("bool"),
            Type::U32 => f.write_str("u32"),
            Type::I32 => f.write_str("i32"),
            Type::U64 => f.write_str("u64"),
            Type::I64 => f.write_str("i64"),
            Type::F32 => f.write_str("f32"),
            Type::F64 => f.write_str("f64"),
            Type::String => f.write_str("string"),
            Type::Version => f.write_str("version"),
            Type::List(t) => write!(f, "list<{t}>"),
            Type::Map(t) => write!(f, "map<string, {t}>"),
            Type::Named(n) => f.write_str(n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub doc: Vec<String>,
    pub payload: Payload,
    pub since: Version,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    /// Written as the variant's name alone.
    None,
    Fields(Vec<Field>),
    /// Carries the named struct.
    Struct(String),
}

/// A union variant or enum value, seen alike.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Case<'a> {
    pub name: &'a str,
    pub span: &'a Span,
    /// Whether it carries a struct, which then stands for it in Go.
    pub carries_struct: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub doc: Vec<String>,
    pub since: Version,
    pub span: Span,
}
