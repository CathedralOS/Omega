//! A scalar returned by an independent call stays equal to itself. Two calls
//! through the same provider instruction can return different values.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/project.rs"]
mod project;

use std::{fs, path::Path};

#[test]
fn provider_observations_are_specific_to_one_invocation() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_scalar_samples");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    let scratch = project::stage(
        "independent-scalar-samples",
        &[
            ("supplier/main.omg", &read("supplier/main.omg")),
            ("supplier/build.omg", &read("supplier/build.omg")),
            ("application/build.omg", &read("application/build.omg")),
        ],
    );
    let root = scratch.path().join("application");
    let source = read("application/main.omg");
    for (case, program, failure) in [
        ("reuse", source.clone(), None),
        (
            "resample",
            source.replace("wait(@signal, first)", "wait(@signal, second)"),
            Some("cannot prove synchronization progress"),
        ),
        (
            "retry on itself",
            source.replace("sampled != expected", "sampled == expected"),
            Some("synchronization deadlock"),
        ),
        (
            "resample across a backedge",
            read("backedge.omg"),
            Some("cannot prove synchronization progress"),
        ),
    ] {
        project::write(&root, &[("main.omg", &program)]);
        let output = project::omega(
            &root,
            &["--target", "linux_x86_64", "--build-dir", "out", "build.omg"],
        );
        let (accepted, diagnostic) = project::status(&output);
        if let Some(expected) = failure {
            assert!(!accepted && diagnostic.contains(expected), "{case}: {diagnostic}");
        } else {
            assert!(accepted, "{case}: {diagnostic}");
            let installed = fs::read_to_string(root.join("out/omega-components.tsv")).unwrap();
            assert!(installed.contains(".so"), "{case}: {installed}");
            project::assert_native_returns(&root.join("out/omega-program"), case);
        }
    }
}
