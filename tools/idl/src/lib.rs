//! The schema language and its generator.

use std::fmt;
use std::path::{Path, PathBuf};

mod go;
pub mod model;
mod naming;
mod parse;
mod reference;
mod rust;
mod validate;

pub use model::Schema;
use model::Span;

/// One IDL file: its path, used in diagnostics, and its text.
pub struct Source {
    pub path: String,
    pub text: String,
}

impl Source {
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Source {
            path: path.into(),
            text: text.into(),
        }
    }
}

/// A problem in the schema, where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Span { path, line, column } = &self.span;
        write!(f, "{path}:{line}:{column}: {}", self.message)
    }
}

/// Reads every source as one schema and checks it.
pub fn check(sources: &[Source]) -> Result<Schema, Vec<Diagnostic>> {
    let mut contracts = Vec::new();
    let mut types = Vec::new();
    let mut diagnostics = Vec::new();
    for source in sources {
        match parse::parse(&source.path, &source.text) {
            Ok(file) => {
                contracts.extend(file.contracts);
                types.extend(file.types);
            }
            Err(d) => diagnostics.push(d),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let start = Span {
        path: sources.first().map_or_else(String::new, |s| s.path.clone()),
        line: 1,
        column: 1,
    };
    validate::schema(contracts, types, start)
}

/// One schema and where its generated code goes, all relative to the
/// repository's root. Read from a line of `tools/idl/targets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The directory of `*.idl` files.
    pub schema: PathBuf,
    /// The `src` directory of the Rust crate.
    pub rust: Option<PathBuf>,
    pub go: Option<GoTarget>,
    /// The message reference, a Markdown file.
    pub reference: Option<PathBuf>,
}

/// Where a schema's Go package goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoTarget {
    pub dir: PathBuf,
    /// The package's name, which need not be the directory's.
    pub package: String,
}

impl Target {
    /// Reads `key=value` pairs separated by spaces: `schema`, `rust`, `go`,
    /// `go_package` and `reference`.
    pub fn parse(line: &str) -> Result<Target, String> {
        let mut schema = None;
        let mut target = Target {
            schema: PathBuf::new(),
            rust: None,
            go: None,
            reference: None,
        };
        let mut go = None;
        let mut go_package = None;
        for pair in line.split_whitespace() {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("`{pair}` is not key=value"))?;
            match key {
                "schema" => schema = Some(PathBuf::from(value)),
                "rust" => target.rust = Some(PathBuf::from(value)),
                "go" => go = Some(PathBuf::from(value)),
                "go_package" => {
                    if !naming::is_go_package_name(value) {
                        return Err(format!("`{value}` is not a Go package name"));
                    }
                    go_package = Some(value.to_string());
                }
                "reference" => target.reference = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown key `{key}`")),
            }
        }
        target.schema = schema.ok_or("no `schema=`")?;
        target.go = match (go, go_package) {
            (Some(dir), Some(package)) => Some(GoTarget { dir, package }),
            (None, None) => None,
            _ => return Err("`go=` and `go_package=` go together".to_string()),
        };
        Ok(target)
    }
}

/// Reads and checks the schema in `dir` (under `root`), every `*.idl` file in
/// path order. A failure is the diagnostics, one per line.
pub fn read_schema(root: &Path, dir: &Path) -> Result<Schema, String> {
    let entries =
        std::fs::read_dir(root.join(dir)).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "idl"))
        .collect();
    paths.sort();
    let mut sources = Vec::new();
    for path in paths {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        sources.push(Source::new(shown, text));
    }
    check(&sources).map_err(|ds| {
        ds.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Every file the target's generated code consists of, as path (under the
/// root) and contents.
pub fn generate(schema: &Schema, target: &Target) -> Result<Vec<(PathBuf, String)>, String> {
    let mut files = Vec::new();
    if let Some(dir) = &target.rust {
        for (name, text) in rust::generate(schema)? {
            files.push((dir.join(name), text));
        }
    }
    if let Some(go) = &target.go {
        for (name, text) in go::generate(schema, &go.package) {
            files.push((go.dir.join(name), text));
        }
    }
    if let Some(path) = &target.reference {
        files.push((path.clone(), reference::generate(schema)));
    }
    Ok(files)
}
