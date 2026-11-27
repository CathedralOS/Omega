//! A selected provider can establish different normal returns on the same
//! borrowed words. The receiving activation decides which ones are feasible.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use psi_provider_selections::{ProgressScope, invocation_progress_failure};
use std::{fs, path::Path};
use terminal_semantics::synchronization::ProgressFailure;

#[test]
fn the_callers_selector_and_actual_words_determine_progress() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_conditional_store");
    let read = |file: &str| fs::read_to_string(fixture.join(file)).unwrap();
    let application = read("application/main.omg");
    let supplier = read("supplier/main.omg");
    let application_build = read("application/build.omg");
    let supplier_build = read("supplier/build.omg");
    for (parameter_type, comparison, first, other, yes, no) in [
        ("u32", "choose == 7", "choose == 7", "choose != 7", "7", "8"),
        (
            "bool",
            "choose",
            "choose == true",
            "choose == false",
            "true",
            "false",
        ),
    ] {
        let supplier = supplier
            .replace("choose: u32", &format!("choose: {parameter_type}"))
            .replace("choose == 7", comparison)
            .replace(
                "choose != 7",
                if parameter_type == "bool" {
                    "!choose"
                } else {
                    "choose != 7"
                },
            );
        let scratch = project::stage(
            "conditional-provider-store",
            &[
                ("supplier/main.omg", &supplier),
                ("supplier/build.omg", &supplier_build),
                ("application/build.omg", &application_build),
            ],
        );
        let supplier = components::inspect(&scratch.path().join("supplier"), "clear_selected");
        let candidates = &supplier.semantic_module().interfaces.provider_candidates;
        assert_eq!(candidates.len(), 2);
        for (requirement, actual, entry_argument, expected) in [
            (first, "@right", yes, None),
            (other, "@right", no, Some(ProgressFailure::Deadlock)),
            ("", "@right", yes, Some(ProgressFailure::Unproved)),
            ("", "@left", yes, None),
        ] {
            let mut source = application
                .replace("choose: u32", &format!("choose: {parameter_type}"))
                .replace("@right, choose", &format!("{actual}, choose"))
                .replace("work(7)", &format!("work({entry_argument})"));
            if !requirement.is_empty() {
                source = source.replacen(
                    "crashes none {",
                    &format!("crashes none requires {requirement} {{"),
                    1,
                );
            }
            let app = scratch.path().join("application");
            project::write(&app, &[("main.omg", &source)]);
            let receiver = components::inspect(&app, "work");
            let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
                receiver.semantic_module().entry
            else {
                panic!("expected the selected work entry");
            };
            for provider in candidates {
                let name = supplier
                    .semantic_module()
                    .identities
                    .text(provider.provider_identity)
                    .unwrap();
                let expected = if name.ends_with("::AlwaysClearsFirst") {
                    None
                } else {
                    expected
                };
                let admitted = components::admit(&receiver, &supplier, provider);
                let failure = invocation_progress_failure(
                    receiver.view(),
                    entry,
                    ProgressScope::Executable,
                    [&admitted],
                    |_| None,
                );
                assert_eq!(
                    failure.map(|error| error.cause),
                    expected,
                    "{comparison} {requirement} {actual}"
                );
            }
        }
    }
}
