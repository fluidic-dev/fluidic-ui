//! `fluidic-ui-idl check` fails when generated code is stale.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A throwaway repository root with one schema and its targets.
fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fluidic-ui-idl-fresh-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("tools/idl")).unwrap();
    std::fs::create_dir_all(root.join("schema")).unwrap();
    std::fs::write(
        root.join("schema/s.idl"),
        "contract 0.1\nstruct point since 0.1 {\n    x: i32 since 0.1\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/idl/targets"),
        "# a comment\nschema=schema rust=rust/src go=go go_package=point reference=reference.md\n",
    )
    .unwrap();
    root
}

fn idl(command: &str, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fluidic-ui-idl"))
        .args([command, "--root"])
        .arg(root)
        .output()
        .unwrap()
}

#[test]
fn check_passes_after_generate() {
    let root = root("passes");
    assert!(idl("generate", &root).status.success());
    for file in [
        "rust/src/lib.rs",
        "rust/src/json.rs",
        "go/schema.go",
        "go/json.go",
        "reference.md",
    ] {
        assert!(root.join(file).exists(), "{file} was not generated");
    }
    let check = idl("check", &root);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn check_names_an_edited_or_missing_file() {
    let root = root("stale");
    assert!(idl("generate", &root).status.success());
    std::fs::write(root.join("go/schema.go"), "package point\n").unwrap();
    std::fs::remove_file(root.join("rust/src/lib.rs")).unwrap();
    let check = idl("check", &root);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(!check.status.success());
    assert!(stderr.contains("go/schema.go is stale"), "{stderr}");
    assert!(stderr.contains("rust/src/lib.rs is stale"), "{stderr}");
    assert!(!stderr.contains("json"), "{stderr}");
}

#[test]
fn check_fails_on_a_schema_error() {
    let root = root("error");
    std::fs::write(
        root.join("schema/s.idl"),
        "contract 0.1\nstruct p since 0.1 { x: nope since 0.1 }\n",
    )
    .unwrap();
    let check = idl("check", &root);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(!check.status.success());
    assert!(
        stderr.contains("schema/s.idl:2:22: field `x` names `nope`, which is not declared"),
        "{stderr}"
    );
}

#[test]
fn a_go_package_name_is_checked() {
    let root = root("package");
    std::fs::write(
        root.join("tools/idl/targets"),
        "schema=schema go=go go_package=type\n",
    )
    .unwrap();
    let check = idl("check", &root);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(!check.status.success());
    assert!(
        stderr.contains("tools/idl/targets:1: `type` is not a Go package name"),
        "{stderr}"
    );
}
