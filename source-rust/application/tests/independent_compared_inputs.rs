//! A provider clamps one selected entry input against another. Portable
//! execution must settle the wait for every loosely constrained pair.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use psi_provider_selections::{ProgressScope, invocation_progress_failure};
use std::{fs, path::Path};
use terminal_semantics::synchronization::ProgressFailure::{Deadlock, Unproved};

#[test]
fn a_wait_uses_the_order_of_its_own_resident_and_pivot() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_compared_inputs");
    let read = |file: &str| fs::read_to_string(fixture.join(file)).unwrap();
    let app = read("application/main.omg");
    let scratch = project::stage(
        "independent-compared-inputs",
        &[
            ("supplier/main.omg", &read("supplier/main.omg")),
            ("supplier/build.omg", &read("supplier/build.omg")),
            ("application/build.omg", &read("application/build.omg")),
        ],
    );
    let supplier = components::inspect(&scratch.path().join("supplier"), "adjust");
    let application = scratch.path().join("application");
    for (requirement, wait_predicate, actual, example, clamped, untouched) in [
        (
            "",
            "sampled < pivot",
            "pivot",
            "1, 4, 9",
            None,
            Some(Unproved),
        ),
        (
            "",
            "pivot > sampled",
            "pivot",
            "1, 4, 9",
            None,
            Some(Unproved),
        ),
        (
            "",
            "sampled >= pivot",
            "pivot",
            "1, 4, 9",
            Some(Deadlock),
            Some(Unproved),
        ),
        (
            "",
            "sampled < pivot",
            "other",
            "1, 4, 9",
            Some(Unproved),
            Some(Unproved),
        ),
        (
            " requires input != pivot",
            "sampled == pivot",
            "pivot",
            "1, 4, 9",
            Some(Unproved),
            None,
        ),
        (
            " requires input == pivot",
            "sampled == pivot",
            "pivot",
            "4, 4, 9",
            Some(Deadlock),
            Some(Deadlock),
        ),
        (
            " requires input >= pivot",
            "sampled < pivot",
            "pivot",
            "8, 4, 9",
            None,
            None,
        ),
    ] {
        let source = app
            .replace("sampled < pivot", wait_predicate)
            .replace("wait(@signal, pivot)", &format!("wait(@signal, {actual})"))
            .replace(
                "crashes none {\n    let mutable",
                &format!("crashes none{requirement} {{\n    let mutable"),
            )
            .replace("work(1, 4, 9);", &format!("work({example});"));
        project::write(&application, &[("main.omg", &source)]);
        let receiver = components::inspect(&application, "work");
        let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
            receiver.semantic_module().entry
        else {
            panic!("expected work")
        };
        let candidates = &supplier.semantic_module().interfaces.provider_candidates;
        assert_eq!(candidates.len(), 2);
        for candidate in candidates {
            let name = supplier
                .semantic_module()
                .identities
                .text(candidate.provider_identity)
                .unwrap();
            let expected = if name.ends_with("::KeepInput") {
                untouched
            } else {
                clamped
            };
            let installed = components::admit(&receiver, &supplier, candidate);
            let actual = invocation_progress_failure(
                receiver.view(),
                entry,
                ProgressScope::Executable,
                [&installed],
                |_| None,
            );
            assert_eq!(
                actual.map(|error| error.cause),
                expected,
                "{name} {requirement} {wait_predicate}"
            );
        }
    }
}
