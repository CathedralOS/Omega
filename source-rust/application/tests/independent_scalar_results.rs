//! A checked provider's normal return can determine a later private wait.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use psi_provider_selections::{ProgressScope, invocation_progress_failure};
use std::{fs, path::Path};
use terminal_semantics::synchronization::ProgressFailure::{Deadlock, Unproved};

#[test]
fn a_private_wait_uses_the_selected_scalar_result() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_scalar_results");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    let scratch = project::stage(
        "independent-scalar-results",
        &[
            ("supplier/main.omg", &read("supplier/main.omg")),
            ("supplier/build.omg", &read("supplier/build.omg")),
            ("application/build.omg", &read("application/build.omg")),
        ],
    );
    let supplier = components::inspect(&scratch.path().join("supplier"), "literal");
    let root = scratch.path().join("application");
    let source = read("application/main.omg");
    for input in [0, 1] {
        let application = source.replace("Samples::read(0)", &format!("Samples::read({input})"));
        project::write(&root, &[("main.omg", &application)]);
        let receiver = components::inspect(&root, "main");
        let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
            receiver.semantic_module().entry
        else {
            panic!("expected application entry");
        };
        let unresolved = invocation_progress_failure(
            receiver.view(),
            entry,
            ProgressScope::Executable,
            [],
            |_| None,
        );
        assert_eq!(unresolved.map(|failure| failure.cause), Some(Unproved));
        let candidates = &supplier.semantic_module().interfaces.provider_candidates;
        assert_eq!(candidates.len(), 4);
        for candidate in candidates {
            let name = supplier
                .semantic_module()
                .identities
                .text(candidate.provider_identity)
                .unwrap();
            let blocked = if name.ends_with("::SourceSamples") {
                false
            } else if name.ends_with("::OneSample") {
                true
            } else {
                input == 1
            };
            let installed = components::admit(&receiver, &supplier, candidate);
            let result = invocation_progress_failure(
                receiver.view(),
                entry,
                ProgressScope::Executable,
                [&installed],
                |_| None,
            );
            assert_eq!(
                result.map(|failure| failure.cause),
                blocked.then_some(Deadlock),
                "{input} {name}"
            );
        }
    }
    let (built, diagnostic) = project::status(&project::omega(
        &root,
        &[
            "--target",
            "linux_x86_64",
            "--build-dir",
            "out",
            "build.omg",
        ],
    ));
    assert!(built, "{diagnostic}");
    assert!(
        fs::read_to_string(root.join("out/omega-components.tsv"))
            .unwrap()
            .contains(".so")
    );
    project::assert_native_returns(&root.join("out/omega-program"), "return zero");
    let (ran, diagnostic) = project::status(&project::omega(
        &root,
        &["run", "--target", "linux_x86_64", "build.omg"],
    ));
    assert!(ran, "{diagnostic}");
    let build = read("application/build.omg").replace("::SourceSamples", "::ChoiceSample");
    project::write(&root, &[("build.omg", &build)]);
    let (built, diagnostic) = project::status(&project::omega(
        &root,
        &[
            "--target",
            "linux_x86_64",
            "--build-dir",
            "denied",
            "build.omg",
        ],
    ));
    assert!(
        !built && diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}
