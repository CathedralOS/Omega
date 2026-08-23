use std::fs;
use std::path::{Path, PathBuf};

use psi_source_files_to_tokens::Lexer;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn parses_dungeon_sample_project() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect(
            "parser crate should live under bootstrap/onramps/omega-rust/psi/pipeline/psi-tokens-to-syntax-trees",
        );
    let sample_root = repo_root.join("samples/cli/games/dungeon_crawler_cli");
    let mut omega_files = Vec::new();

    collect_omega_files(&sample_root, &mut omega_files);
    omega_files.sort();

    assert!(
        !omega_files.is_empty(),
        "expected at least one Omega source file under {}",
        sample_root.display()
    );

    for path in omega_files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let tokens = Lexer::new(&source)
            .tokenize()
            .unwrap_or_else(|error| panic!("failed to tokenize {}: {error:?}", path.display()));

        parse_syntax_trees(&tokens)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error:?}", path.display()));
    }
}

#[test]
fn sample_projects_ignore_local_build_output() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect(
            "parser crate should live under bootstrap/onramps/omega-rust/psi/pipeline/psi-tokens-to-syntax-trees",
        );
    let sample_root = repo_root.join("samples");
    let mut sample_projects = Vec::new();

    collect_project_roots(&sample_root, &mut sample_projects);
    sample_projects.sort();

    assert!(
        !sample_projects.is_empty(),
        "expected at least one sample project under {}",
        sample_root.display()
    );

    for project_root in sample_projects {
        let gitignore_path = project_root.join(".gitignore");
        let gitignore = fs::read_to_string(&gitignore_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", gitignore_path.display()));

        assert!(
            gitignore.lines().any(|line| line.trim() == "/build/"),
            "sample project {} should ignore its local build directory",
            project_root.display()
        );
    }
}

#[test]
fn canaries_ignore_local_build_output() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect(
            "parser crate should live under bootstrap/onramps/omega-rust/psi/pipeline/psi-tokens-to-syntax-trees",
        );
    let gitignore_path = repo_root.join("canaries/.gitignore");
    let gitignore = fs::read_to_string(&gitignore_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", gitignore_path.display()));

    // Depth-anchored on purpose (see canaries/.gitignore's comment):
    // `**/build/` also swallowed the fail/build/ CATEGORY (the build.omg
    // canaries), silently keeping committed FAIL_CANARIES entries' files out
    // of git. The canary tree is uniformly <kind>/<category>/<name>/.
    assert!(
        gitignore.lines().any(|line| line.trim() == "*/*/*/build/"),
        "canaries should ignore generated build directories at the uniform canary depth"
    );
}

fn collect_omega_files(path: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|file_name| file_name == "build")
            {
                continue;
            }
            collect_omega_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "omg") {
            files.push(path);
        }
    }
}

fn collect_project_roots(path: &Path, projects: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            if path.join("main.omg").is_file() {
                projects.push(path);
            } else {
                collect_project_roots(&path, projects);
            }
        }
    }
}
