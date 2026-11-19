use std::path::Path;

/// Resolve repository-owned fixtures from the compiler checkout, regardless of the test process's cwd.
pub fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| {
            path.join("tests/fixtures").is_dir() && path.join("source/library/std").is_dir()
        })
        .expect("the compiler checkout contains the fixture corpus and bundled standard library")
}
