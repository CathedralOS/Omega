use std::fs;
use std::path::{Path, PathBuf};

use omega_lexer::Lexer;
use omega_parser::parse_file;

#[test]
fn parses_every_sample_file() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("parser crate should live under compiler/omega-parser");
    let sample_root = repo_root.join("samples");
    let mut sample_files = Vec::new();

    collect_omega_files(&sample_root, &mut sample_files);
    sample_files.sort();

    assert!(
        !sample_files.is_empty(),
        "expected at least one Omega sample file under {}",
        sample_root.display()
    );

    for path in sample_files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let tokens = Lexer::new(&source)
            .tokenize()
            .unwrap_or_else(|error| panic!("failed to tokenize {}: {error:?}", path.display()));

        parse_file(&tokens)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error:?}", path.display()));
    }
}

fn collect_omega_files(path: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let path = entry.path();

        if path.is_dir() {
            collect_omega_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "omg") {
            files.push(path);
        }
    }
}
