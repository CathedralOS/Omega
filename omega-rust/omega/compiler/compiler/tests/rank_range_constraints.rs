use compiler::compile_to_checked;
use std::path::Path;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join("tests/omega").is_dir())
        .expect("repository root")
}

#[test]
fn increasing_rank_range_reaches_checked_trees() {
    let path = repository_root()
        .join("tests/omega/pass/termination/increasing_to_rank_range_compile/main.omg");
    compile_to_checked(&path, None).expect("the view proves its canonical range");
}

#[test]
fn floor_exclusion_reports_the_unproved_rank_range() {
    let path =
        repository_root().join("tests/omega/fail/termination/rank_range_excludes_floor/main.omg");
    let diagnostics = match compile_to_checked(&path, None) {
        Ok(_) => panic!("the distance can reach zero"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove rank range `1..=limit`")),
        "{diagnostics:#?}"
    );
}
