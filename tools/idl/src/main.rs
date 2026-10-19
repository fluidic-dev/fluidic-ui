//! `fluidic-ui-idl generate` writes the generated code for every target in
//! `tools/idl/targets`; `fluidic-ui-idl check` fails if any of it is stale.

use fluidic_ui_idl::{Target, generate, read_schema};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: fluidic-ui-idl generate|check [--root DIR]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, root) = match args.as_slice() {
        [command] => (
            command.as_str(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        ),
        [command, flag, root] if flag == "--root" => (command.as_str(), PathBuf::from(root)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if command != "generate" && command != "check" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match run(command == "check", &root) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Returns whether everything was already up to date, when checking.
fn run(check: bool, root: &std::path::Path) -> Result<bool, String> {
    let manifest = root.join("tools/idl/targets");
    let text =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut fresh = true;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let target =
            Target::parse(line).map_err(|e| format!("tools/idl/targets:{}: {e}", n + 1))?;
        let schema = read_schema(root, &target.schema)?;
        for (path, text) in generate(&schema, &target)? {
            let full = root.join(&path);
            // The same on every OS, for logs and tests.
            let shown = path.to_string_lossy().replace('\\', "/");
            let current = std::fs::read_to_string(&full).ok();
            if current.as_deref() == Some(text.as_str()) {
                continue;
            }
            if check {
                eprintln!("{shown} is stale: run `cargo run -p fluidic-ui-idl -- generate`");
                fresh = false;
            } else {
                if let Some(dir) = full.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                std::fs::write(&full, text).map_err(|e| format!("{}: {e}", full.display()))?;
                println!("wrote {shown}");
            }
        }
    }
    Ok(fresh)
}
