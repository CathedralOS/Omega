//! A nonzero field offset and sign extension must survive scaled addressing.
//! Exercise the same source with each rule selected separately and together.
#![cfg(all(
    target_arch = "x86_64",
    any(target_os = "linux", target_os = "windows")
))]

use platform_custody::temporary_directory::TemporaryDirectory;
#[path = "support/observations.rs"]
mod observations;

#[test]
fn optimized_texture_reads_preserve_scalar_values_and_remainder() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/texture_lookup/run/natural_widths");
    let build = std::fs::read_to_string(fixture.join("build.omg")).unwrap();
    for omitted in [
        &["UnsignedPowerOfTwoRemainderMask", "X86ScaledIndexedLoad"][..],
        &["X86ScaledIndexedLoad"],
        &["UnsignedPowerOfTwoRemainderMask"],
        &[],
    ] {
        let selected = build
            .lines()
            .filter(|line| !omitted.iter().any(|name| line.contains(name)))
            .collect::<Vec<_>>()
            .join("\n");
        observations::assert_silent_return(
            || {
                let scratch = TemporaryDirectory::create("omega-texture-lookup", false).unwrap();
                std::fs::copy(fixture.join("main.omg"), scratch.path().join("main.omg")).unwrap();
                std::fs::write(scratch.path().join("build.omg"), &selected).unwrap();
                scratch
            },
            &[
                "--build-input",
                "build.omg",
                "--build-input",
                "main.omg",
                "build.omg",
            ],
        );
    }
}
