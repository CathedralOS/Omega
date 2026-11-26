#[path = "support/observations.rs"]
mod observations;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;

fn project(short_input: bool) -> TemporaryDirectory {
    let root = support::repository_root();
    let fixture = root.join("source-rust/application/tests/fixtures/simd/positive/wrapping_add");
    let scratch = TemporaryDirectory::create("packed-integer", false).unwrap();
    let source = fs::read_to_string(fixture.join("main.omg")).unwrap();
    let source = if short_input {
        let short = source
            .replace("left: [i16; 8]", "left: [i16; 7]")
            .replace("5i16, -6i16];", "5i16];");
        assert_ne!(source, short);
        short
    } else {
        source
    };
    fs::write(scratch.path().join("main.omg"), source).unwrap();
    let build = fs::read_to_string(fixture.join("build.omg")).unwrap();
    let library = root
        .join("source/library/simd")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        scratch.path().join("build.omg"),
        build.replace("../../../../../../../source/library/simd", &library),
    )
    .unwrap();
    scratch
}

#[test]
fn dependent_additions_wrap_each_lane() {
    observations::assert_silent_return(|| project(false), &["build.omg"]);
}

#[test]
fn loading_eight_lanes_requires_eight_elements() {
    let (output, report) =
        observations::run(&project(true), &["run", "--engine", "psi"], &["build.omg"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("expects an array with 8 elements but the supplied array has 7"),
        "{report}"
    );
}
