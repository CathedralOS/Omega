//! The same guarded scalar write must establish the same postcondition in
//! the local caller and after receiving a separately compiled provider.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use psi_provider_selections::{ProgressScope, invocation_progress_failure};
use std::{fs, path::Path};
use terminal_semantics::synchronization::ProgressFailure;

#[test]
fn a_stored_scalar_retains_the_condition_of_its_own_return() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_guarded_word");
    let read = |file: &str| fs::read_to_string(fixture.join(file)).unwrap();
    let application = read("application/main.omg");
    let supplier = read("supplier/main.omg");
    for (name, condition, on_match, otherwise, expected) in [
        (
            "rewrite input",
            "input == 1",
            "0",
            "signal.word.store(input, Publish);",
            None,
        ),
        ("keep prior input", "input == 1", "0", "", None),
        (
            "wrong comparison",
            "input == 0",
            "0",
            "",
            Some(ProgressFailure::Unproved),
        ),
        (
            "held by own branch",
            "input == 1",
            "input",
            "signal.word.store(1, Publish);",
            Some(ProgressFailure::Deadlock),
        ),
        (
            "held on each arm",
            "input == 1",
            "1",
            "signal.word.store(1, Publish);",
            Some(ProgressFailure::Deadlock),
        ),
    ] {
        let supplier = supplier
            .replace("input == 1", condition)
            .replace("signal.word.store(input, Publish);", otherwise)
            .replace("store(0, Publish)", &format!("store({on_match}, Publish)"));
        let supplier = if on_match == "input" {
            supplier
                .replace("clear(signal)", "clear(signal, input)")
                .replace(
                    "clear(signal: @Signal)",
                    "clear(signal: @Signal, input: u32)",
                )
        } else {
            supplier
        };
        let scratch = project::stage(
            "guarded-word-effects",
            &[
                ("supplier/main.omg", &supplier),
                ("supplier/build.omg", &read("supplier/build.omg")),
                ("application/build.omg", &read("application/build.omg")),
            ],
        );
        let app = scratch.path().join("application");
        let provider = components::inspect(&scratch.path().join("supplier"), "adjust");
        for local in [false, true] {
            // Inspection of a closed local failure can reject the source before
            // publication. The independent execution controls exercise both causes.
            if local && expected.is_some() {
                continue;
            }
            let source = if local {
                application.replace("|signals|::Clear::normalize", "|signals|::adjust")
            } else {
                application.clone()
            };
            let result = judgment(&app, &source, (!local).then_some(&provider));
            assert_eq!(result, expected, "{name} (local {local})");
        }
        if name == "keep prior input" {
            // A different root input has the same type (and the same local
            // number when forwarded through a helper). It cannot discharge
            // a requirement about the original word.
            let source = application
                .replace("work(input: u32)", "work(input: u32, selector: u32)")
                .replace("normalize(@signal, input)", "normalize(@signal, selector)")
                .replace("work(1)", "work(1, 0)");
            assert_eq!(
                judgment(&app, &source, Some(&provider)),
                Some(ProgressFailure::Unproved)
            );
        }
    }
}

fn judgment(
    application: &Path,
    text: &str,
    provider: Option<&terminal_codec::CanonicalTerminalArtifact>,
) -> Option<ProgressFailure> {
    project::write(application, &[("main.omg", text)]);
    let receiver = components::inspect(application, "work");
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
        receiver.semantic_module().entry
    else {
        panic!("expected work");
    };
    let installed = provider.map(|supplier| {
        components::admit(
            &receiver,
            supplier,
            &supplier.semantic_module().interfaces.provider_candidates[0],
        )
    });
    invocation_progress_failure(
        receiver.view(),
        entry,
        ProgressScope::Executable,
        installed.iter(),
        |_| None,
    )
    .map(|error| error.cause)
}
