//! Reads IDL text into declarations. Rules that span declarations are
//! checked afterwards, in `check`.

use crate::Diagnostic;
use crate::model::{
    Direction, EnumValue, Field, Payload, Span, Type, TypeDecl, TypeKind, Variant, Version,
};

/// One file's declarations.
pub(crate) struct File {
    pub contracts: Vec<(Version, Vec<String>, Span)>,
    pub types: Vec<TypeDecl>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Version(Version),
    Doc(String),
    Punct(char),
    End,
}

struct Lexer<'a> {
    path: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    text: &'a str,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    fn new(path: &'a str, text: &'a str) -> Self {
        Lexer {
            path,
            chars: text.char_indices().peekable(),
            text,
            line: 1,
            column: 1,
        }
    }

    fn span(&self) -> Span {
        Span {
            path: self.path.to_string(),
            line: self.line,
            column: self.column,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let (_, c) = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn rest(&mut self) -> &'a str {
        match self.chars.peek() {
            Some(&(i, _)) => &self.text[i..],
            None => "",
        }
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek().filter(|&c| f(c)) {
            s.push(c);
            self.bump();
        }
        s
    }

    fn error(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            span,
            message: message.into(),
        }
    }

    fn tokens(mut self) -> Result<Vec<(Token, Span)>> {
        let mut out = Vec::new();
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            let span = self.span();
            let Some(c) = self.peek() else {
                out.push((Token::End, span));
                return Ok(out);
            };
            if self.rest().starts_with("///") {
                self.bump();
                self.bump();
                self.bump();
                let line = self.take_while(|c| c != '\n');
                let line = line
                    .strip_prefix(' ')
                    .unwrap_or(&line)
                    .trim_end()
                    .to_string();
                if let Some(problem) = doc_problem(&line) {
                    return Err(Self::error(span, problem));
                }
                out.push((Token::Doc(line), span));
            } else if self.rest().starts_with("//") {
                self.take_while(|c| c != '\n');
            } else if c.is_ascii_alphabetic() || c == '_' {
                let word = self.take_while(|c| c.is_ascii_alphanumeric() || c == '_');
                out.push((Token::Ident(word), span));
            } else if c.is_ascii_digit() {
                let text = self.take_while(|c| c.is_ascii_digit() || c == '.');
                let version = parse_version(&text).ok_or_else(|| {
                    Self::error(
                        span.clone(),
                        format!("`{text}` is not a version: write MAJOR.MINOR"),
                    )
                })?;
                out.push((Token::Version(version), span));
            } else if "{}()<>:,".contains(c) {
                self.bump();
                out.push((Token::Punct(c), span));
            } else {
                return Err(Self::error(span, format!("unexpected character `{c}`")));
            }
        }
    }
}

/// Why a doc comment line cannot be copied into generated code as it is.
/// A doc is one plain paragraph, so that neither rustdoc nor gofmt finds
/// structure in it to act on: a code block would run as a test, and gofmt
/// reflows indented lines.
fn doc_problem(line: &str) -> Option<String> {
    const BIDI: &[char] = &[
        '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
        '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ];
    if line.chars().any(|c| c.is_control() && c != '\t') {
        Some("a doc comment cannot hold control characters".to_string())
    } else if line.contains(BIDI) {
        Some("a doc comment cannot hold bidirectional formatting characters".to_string())
    } else if line.contains("[^") {
        Some("a doc comment cannot hold a footnote".to_string())
    } else {
        block_start(line).map(|block| {
            format!("a doc comment is one plain paragraph, and this line starts {block}")
        })
    }
}

/// The Markdown block a line would start, if any.
fn block_start(line: &str) -> Option<&'static str> {
    // After a marker, a space or the end of the line.
    let marker_ends = |rest: &str| rest.is_empty() || rest.starts_with(' ');
    let first = line.chars().next()?;
    let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
    let hashes = line.trim_start_matches('#');
    let numbered =
        digits.len() < line.len() && digits.strip_prefix(['.', ')']).is_some_and(marker_ends);
    if first == ' ' || first == '\t' {
        Some("an indented block")
    } else if line.chars().all(|c| "-=_*:| ".contains(c)) {
        Some("a rule, an underline or a table's delimiter row")
    } else if ("-*+".contains(first) && marker_ends(&line[1..])) || numbered {
        Some("a list")
    } else if line.len() - hashes.len() <= 6 && hashes.len() < line.len() && marker_ends(hashes) {
        Some("a heading")
    } else if line.starts_with("```") || line.starts_with("~~~") {
        Some("a code block")
    } else if line.starts_with('[')
        && line
            .find(']')
            .is_some_and(|end| line[end + 1..].starts_with(':'))
    {
        Some("a link definition")
    } else {
        match first {
            '>' => Some("a quote"),
            '|' => Some("a table"),
            '<' => Some("an HTML block"),
            _ => None,
        }
    }
}

/// `MAJOR.MINOR` in decimal, without leading zeros.
pub(crate) fn parse_version(text: &str) -> Option<Version> {
    let (major, minor) = text.split_once('.')?;
    let part = |s: &str| {
        let canonical = !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s == "0" || !s.starts_with('0'));
        if canonical {
            s.parse::<u32>().ok()
        } else {
            None
        }
    };
    Some(Version {
        major: part(major)?,
        minor: part(minor)?,
    })
}

struct Parser {
    tokens: Vec<(Token, Span)>,
    at: usize,
}

type Result<T> = std::result::Result<T, Diagnostic>;

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.at].0
    }

    fn span(&self) -> Span {
        self.tokens[self.at].1.clone()
    }

    fn next(&mut self) -> (Token, Span) {
        let t = self.tokens[self.at].clone();
        if t.0 != Token::End {
            self.at += 1;
        }
        t
    }

    fn describe(token: &Token) -> String {
        match token {
            Token::Ident(s) => format!("`{s}`"),
            Token::Version(v) => format!("`{v}`"),
            Token::Doc(_) => "a doc comment".to_string(),
            Token::Punct(c) => format!("`{c}`"),
            Token::End => "the end of the file".to_string(),
        }
    }

    fn expected(&self, what: &str) -> Diagnostic {
        Diagnostic {
            span: self.span(),
            message: format!("expected {what}, found {}", Self::describe(self.peek())),
        }
    }

    fn punct(&mut self, c: char) -> Result<()> {
        if *self.peek() == Token::Punct(c) {
            self.next();
            Ok(())
        } else {
            Err(self.expected(&format!("`{c}`")))
        }
    }

    fn eat_punct(&mut self, c: char) -> bool {
        let found = *self.peek() == Token::Punct(c);
        if found {
            self.next();
        }
        found
    }

    fn ident(&mut self, what: &str) -> Result<(String, Span)> {
        match self.next() {
            (Token::Ident(s), span) => Ok((s, span)),
            _ => {
                self.at -= 1;
                Err(self.expected(what))
            }
        }
    }

    fn keyword(&mut self, word: &str) -> Result<()> {
        if *self.peek() == Token::Ident(word.to_string()) {
            self.next();
            Ok(())
        } else {
            Err(self.expected(&format!("`{word}`")))
        }
    }

    fn version(&mut self) -> Result<Version> {
        match self.peek().clone() {
            Token::Version(v) => {
                self.next();
                Ok(v)
            }
            _ => Err(self.expected("a version, MAJOR.MINOR")),
        }
    }

    fn since(&mut self) -> Result<Version> {
        self.keyword("since")?;
        self.version()
    }

    /// A doc comment's lines, without empty lines around them. It is one
    /// paragraph: an empty line inside would let gofmt find a heading.
    fn docs(&mut self) -> Result<Vec<String>> {
        let mut doc = Vec::new();
        while let Token::Doc(line) = self.peek().clone() {
            doc.push((line, self.span()));
            self.next();
        }
        let first = doc
            .iter()
            .position(|(l, _)| !l.is_empty())
            .unwrap_or(doc.len());
        let last = doc
            .iter()
            .rposition(|(l, _)| !l.is_empty())
            .map_or(first, |i| i + 1);
        let doc = &doc[first..last];
        if let Some((_, span)) = doc.iter().find(|(l, _)| l.is_empty()) {
            return Err(Diagnostic {
                span: span.clone(),
                message: "a doc comment is one paragraph, without empty lines".to_string(),
            });
        }
        Ok(doc.iter().map(|(l, _)| l.clone()).collect())
    }

    fn file(&mut self) -> Result<File> {
        let mut file = File {
            contracts: Vec::new(),
            types: Vec::new(),
        };
        loop {
            let doc = self.docs()?;
            let span = self.span();
            let keyword = match self.next() {
                (Token::End, _) if doc.is_empty() => return Ok(file),
                (Token::Ident(k), _) => k,
                _ => {
                    self.at -= 1;
                    return Err(self.expected("`contract`, `message`, `struct`, `union` or `enum`"));
                }
            };
            if keyword == "contract" {
                let version = self.version()?;
                file.contracts.push((version, doc, span));
                continue;
            }
            let kind_keyword = keyword.as_str();
            if !["message", "struct", "union", "enum"].contains(&kind_keyword) {
                self.at -= 1;
                return Err(self.expected("`contract`, `message`, `struct`, `union` or `enum`"));
            }
            let (name, span) = self.ident("a name")?;
            let from = if kind_keyword == "message" {
                self.keyword("from")?;
                Some(match self.ident("`app`, `host` or `either`")? {
                    (s, _) if s == "app" => Direction::App,
                    (s, _) if s == "host" => Direction::Host,
                    (s, _) if s == "either" => Direction::Either,
                    _ => {
                        self.at -= 1;
                        return Err(self.expected("`app`, `host` or `either`"));
                    }
                })
            } else {
                None
            };
            let since = self.since()?;
            self.punct('{')?;
            let kind = match kind_keyword {
                "message" => TypeKind::Message {
                    from: from.unwrap(),
                    fields: self.fields()?,
                },
                "struct" => TypeKind::Struct {
                    fields: self.fields()?,
                },
                "union" => TypeKind::Union {
                    variants: self.variants()?,
                },
                _ => TypeKind::Enum {
                    values: self.values()?,
                },
            };
            file.types.push(TypeDecl {
                name,
                doc,
                since,
                kind,
                span,
            });
        }
    }

    /// Fields up to and including the closing `}`.
    fn fields(&mut self) -> Result<Vec<Field>> {
        let mut fields = Vec::new();
        loop {
            let doc = self.docs()?;
            if doc.is_empty() && self.eat_punct('}') {
                return Ok(fields);
            }
            let (mut name, mut span) = self.ident("a field")?;
            let mut optional = false;
            if name == "optional" && *self.peek() != Token::Punct(':') {
                optional = true;
                (name, span) = self.ident("a field name")?;
            }
            self.punct(':')?;
            let ty = self.ty()?;
            let since = self.since()?;
            fields.push(Field {
                name,
                doc,
                optional,
                ty,
                since,
                span,
            });
        }
    }

    fn ty(&mut self) -> Result<Type> {
        let (name, _) = self.ident("a type")?;
        match name.as_str() {
            "list" => {
                self.punct('<')?;
                let t = self.ty()?;
                self.punct('>')?;
                Ok(Type::List(Box::new(t)))
            }
            "map" => {
                self.punct('<')?;
                let (key, _) = self.ident("`string`")?;
                if key != "string" {
                    self.at -= 1;
                    return Err(Diagnostic {
                        span: self.span(),
                        message: format!("map keys are `string` only, not `{key}`"),
                    });
                }
                self.punct(',')?;
                let t = self.ty()?;
                self.punct('>')?;
                Ok(Type::Map(Box::new(t)))
            }
            "optional" => {
                self.at -= 1;
                Err(Diagnostic {
                    span: self.span(),
                    message: "`optional` goes before a field's name, and only there".to_string(),
                })
            }
            _ => Ok(Type::scalar(&name).unwrap_or(Type::Named(name))),
        }
    }

    fn variants(&mut self) -> Result<Vec<Variant>> {
        let mut variants = Vec::new();
        loop {
            let doc = self.docs()?;
            if doc.is_empty() && self.eat_punct('}') {
                return Ok(variants);
            }
            let (name, span) = self.ident("a variant")?;
            let payload = if self.eat_punct('{') {
                let brace = self.tokens[self.at - 1].1.clone();
                let fields = self.fields()?;
                if fields.is_empty() {
                    return Err(Diagnostic {
                        span: brace,
                        message: format!(
                            "variant `{name}` has an empty body: a variant without data is written as its name alone"
                        ),
                    });
                }
                Payload::Fields(fields)
            } else if self.eat_punct('(') {
                let (carried, _) = self.ident("a struct name")?;
                self.punct(')')?;
                Payload::Struct(carried)
            } else {
                Payload::None
            };
            let since = self.since()?;
            variants.push(Variant {
                name,
                doc,
                payload,
                since,
                span,
            });
        }
    }

    fn values(&mut self) -> Result<Vec<EnumValue>> {
        let mut values = Vec::new();
        loop {
            let doc = self.docs()?;
            if doc.is_empty() && self.eat_punct('}') {
                return Ok(values);
            }
            let (name, span) = self.ident("an enum value")?;
            let since = self.since()?;
            values.push(EnumValue {
                name,
                doc,
                since,
                span,
            });
        }
    }
}

pub(crate) fn parse(path: &str, text: &str) -> std::result::Result<File, Diagnostic> {
    let tokens = Lexer::new(path, text).tokens()?;
    Parser { tokens, at: 0 }.file()
}
