use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn backend_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("compiler/backend");
    let forbidden = ["omega-abstract-syntax-tree", "omega-parser", "omega-lexer"];

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        for crate_name in forbidden {
            assert!(
                !contents.contains(crate_name),
                "{} must not depend on frontend crate `{crate_name}`",
                cargo_toml.display()
            );
        }
    }
}

#[test]
fn backend_crates_do_not_depend_on_lowering_crates() {
    let repo_root = repo_root();
    let backend_root = repo_root.join("compiler/backend");

    for cargo_toml in cargo_tomls_under(&backend_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        assert!(
            !contents.contains("../../lowering/"),
            "{} must not depend on lowering crates; orchestration should pass lowered IR forward",
            cargo_toml.display()
        );
    }
}

#[test]
fn representation_crates_do_not_depend_on_frontend_crates() {
    let repo_root = repo_root();
    let representations_root = repo_root.join("compiler/representations");
    let forbidden = ["omega-abstract-syntax-tree", "omega-parser", "omega-lexer"];

    for cargo_toml in cargo_tomls_under(&representations_root) {
        let contents = fs::read_to_string(&cargo_toml)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", cargo_toml.display()));

        for crate_name in forbidden {
            assert!(
                !contents.contains(crate_name),
                "{} must not depend on frontend crate `{crate_name}`; put parsing/lowering edges under compiler/lowering instead",
                cargo_toml.display()
            );
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn cargo_tomls_under(root: &Path) -> Vec<PathBuf> {
    let mut cargo_tomls = Vec::new();
    collect_cargo_tomls(root, &mut cargo_tomls);
    cargo_tomls.sort();
    cargo_tomls
}

fn collect_cargo_tomls(path: &Path, cargo_tomls: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            collect_cargo_tomls(&path, cargo_tomls);
        } else if path
            .file_name()
            .is_some_and(|file_name| file_name == "Cargo.toml")
        {
            cargo_tomls.push(path);
        }
    }
}
