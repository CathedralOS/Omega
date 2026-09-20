use std::path::{Path, PathBuf};

use compiler::{CheckedCompileRequest, compile_to_checked};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega")
        .join(relative)
        .join("main.omg")
}

#[test]
fn domain_requires_rejects_an_integer_member() {
    let root = fixture("fail/domains/nonboolean_member_predicate");
    let Err(diagnostics) = compile_to_checked(CheckedCompileRequest::new(&root, None)) else {
        panic!("an integer field is not a domain predicate");
    };
    let expected = std::fs::read_to_string(root.with_file_name("expected.txt")).unwrap();
    for expected in expected.lines().filter(|line| !line.trim().is_empty()) {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.to_string().contains(expected.trim())),
            "missing {expected:?}: {diagnostics:#?}"
        );
    }
}

#[test]
fn domain_requires_accepts_a_boolean_member() {
    let root = fixture("pass/domains/boolean_member_predicate");
    compile_to_checked(CheckedCompileRequest::new(&root, None)).unwrap();
}
