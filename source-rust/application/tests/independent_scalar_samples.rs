//! A scalar returned by an independent call stays equal to itself. Two calls
//! through the same provider instruction can return different values.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
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
    let clamp = read("clamp.omg");
    for (case, program, failure) in [
        ("reuse", source.clone(), None),
        ("clamp two observations", clamp.clone(), None),
        (
            "select an unrelated observation",
            clamp.replace(
                "clamp(@signal, first, second, first)",
                "clamp(@signal, first, second, second)",
            ),
            Some("synchronization deadlock"),
        ),
        (
            "retry after either branch",
            clamp.replace("sampled < expected", "sampled >= expected"),
            Some("synchronization deadlock"),
        ),
        (
            "resample",
            source.replace("wait(@signal, first)", "wait(@signal, second)"),
            Some("synchronization deadlock"),
        ),
        (
            "retry on itself",
            source.replace("sampled != expected", "sampled == expected"),
            Some("synchronization deadlock"),
        ),
        (
            "resample across a backedge",
            read("backedge.omg"),
            Some("synchronization deadlock"),
        ),
    ] {
        project::write(&root, &[("main.omg", &program)]);
        let output = project::omega(
            &root,
            &[
                "--target",
                "linux_x86_64",
                "--build-dir",
                "out",
                "build.omg",
            ],
        );
        let (accepted, diagnostic) = project::status(&output);
        if let Some(expected) = failure {
            assert!(
                !accepted && diagnostic.contains(expected),
                "{case}: {diagnostic}"
            );
        } else {
            assert!(accepted, "{case}: {diagnostic}");
            let installed = fs::read_to_string(root.join("out/omega-components.tsv")).unwrap();
            assert!(installed.contains(".so"), "{case}: {installed}");
            project::assert_native_returns(&root.join("out/omega-program"), case);
        }
    }
}

#[test]
fn a_selected_atomic_read_returns_the_word_at_that_instruction() {
    use psi_provider_selections::{ProgressScope, invocation_progress_failure};
    use terminal_semantics::synchronization::ProgressFailure::{Deadlock, Unproved};
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_scalar_samples");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    let supplier = read("supplier/main.omg");
    let scratch = project::stage(
        "independent-ordered-atomic-results",
        &[
            ("supplier/build.omg", &read("supplier/build.omg")),
            ("application/main.omg", &read("ordered.omg")),
            ("application/build.omg", &read("application/build.omg")),
        ],
    );
    let application = scratch.path().join("application");
    let provider = scratch.path().join("supplier");
    // The old value of the first observation is the entry input. The last
    // observation reads 3, and its update leaves 5 in the caller's word.
    for (returned, selected, concrete) in [
        ("first", Unproved, None),
        ("last", Deadlock, Some(Deadlock)),
    ] {
        let body = supplier.replace(
            "let previous: u32 = source.word.swap(1, ReceivePublish);\n    previous",
            &format!("let first: u32 = source.word.load(Receive);\n    source.word.store(3, Publish);\n    let last: u32 = source.word.fetch_add(2, ReceivePublish);\n    {returned}"),
        );
        project::write(&provider, &[("main.omg", &body)]);
        let supplier = components::inspect(&provider, "read_value");
        for (entry_name, expected) in [("work", Some(selected)), ("main", concrete)] {
            let receiver = components::inspect(&application, entry_name);
            let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
                receiver.semantic_module().entry
            else {
                panic!("expected selected {entry_name}");
            };
            let [candidate] = supplier
                .semantic_module()
                .interfaces
                .provider_candidates
                .as_slice()
            else {
                panic!("expected one independently supplied provider");
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
                result.map(|error| error.cause),
                expected,
                "{returned} from {entry_name}"
            );
        }
    }
}
