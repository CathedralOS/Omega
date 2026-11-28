//! A normal return carries the writes of the selected provider. Aliased
//! arguments see the writes in the provider's execution order.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use std::{fs, path::Path};

#[test]
fn independently_selected_stores_determine_the_callers_next_wait() {
    let (producer, producer_build, consumer, consumer_build) = sources();
    for (provider, second, stuck) in [
        ("OrderedWrites", "@right", false),
        ("OrderedWrites", "@left", true),
        ("WrongField", "@right", true),
        ("NoWrites", "@right", true),
    ] {
        let project = project::stage(
            "independent-atomic-store",
            &[
                ("supplier/main.omg", &producer),
                ("supplier/build.omg", &producer_build),
                (
                    "application/main.omg",
                    &consumer.replace("@right, 1, input", &format!("{second}, 1, input")),
                ),
                (
                    "application/build.omg",
                    &consumer_build.replace("OrderedWrites", provider),
                ),
            ],
        );
        let root = project.path().join("application");
        let (ok, report) = project::status(&project::omega(
            &root,
            &[
                "--target",
                "linux_x86_64",
                "--build-dir",
                "out",
                "build.omg",
            ],
        ));
        assert_eq!(ok, !stuck, "{provider}, {second}: {report}");
        if stuck {
            assert!(report.contains("synchronization deadlock"), "{report}");
        } else {
            let listing = fs::read_to_string(root.join("out/omega-components.tsv")).unwrap();
            assert!(listing.contains(".so"), "{listing}");
            project::assert_native_returns(&root.join("out/omega-program"), provider);
        }
    }
}

fn sources() -> (String, String, String, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_store");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    (
        read("supplier/main.omg"),
        read("supplier/build.omg"),
        read("application/main.omg"),
        read("application/build.omg"),
    )
}

#[test]
fn the_selected_stores_transport_an_entry_input_and_its_checked_requirement() {
    use psi_provider_selections::{ProgressScope, invocation_progress_failure};
    use terminal_semantics::synchronization::ProgressFailure;
    let (mut producer, producer_build, consumer, consumer_build) = sources();
    // Fourteen declarations denote 16,384 calls to write the same word. The
    // receipt must depend on the checked helper graph and its final store.
    let mut helper = String::from("set_word");
    for level in 0..14 {
        let next = format!("set_twice_{level}");
        producer.push_str(&format!(
            "\nmachine {next}(signal: @Signal, input: u32) {{ {helper}(signal, input); {helper}(signal, input); }}"
        ));
        helper = next;
    }
    producer = producer.replace(
        "set_word(first, input);",
        &format!("{helper}(first, input);"),
    );
    let scratch = project::stage(
        "symbolic-independent-atomic-store",
        &[
            ("supplier/main.omg", &producer),
            ("supplier/build.omg", &producer_build),
            ("application/build.omg", &consumer_build),
        ],
    );
    let supplier = components::inspect(&scratch.path().join("supplier"), "write_pair");
    let candidates = &supplier.semantic_module().interfaces.provider_candidates;
    assert_eq!(candidates.len(), 3);
    let app = scratch.path().join("application");
    for (alias, has_requirement, ordered_failure) in [
        (false, true, None),
        (true, true, Some(ProgressFailure::Deadlock)),
        (false, false, Some(ProgressFailure::Unproved)),
    ] {
        let mut source = consumer.clone();
        if alias {
            source = source.replace("@right, 1, input", "@left, 1, input");
        }
        if !has_requirement {
            source = source.replace("requires input != 1", "");
        }
        project::write(&app, &[("main.omg", &source)]);
        let receiver = components::inspect(&app, "work");
        let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
            receiver.semantic_module().entry
        else {
            panic!("work must be the published entry");
        };
        for candidate in candidates {
            let admitted = components::admit(&receiver, &supplier, candidate);
            let provider = supplier
                .semantic_module()
                .identities
                .text(candidate.provider_identity)
                .unwrap();
            let expected = if provider.ends_with("::OrderedWrites") {
                ordered_failure
            } else {
                Some(ProgressFailure::Deadlock)
            };
            let failure = invocation_progress_failure(
                receiver.view(),
                entry,
                ProgressScope::Executable,
                [&admitted],
                |_| None,
            );
            assert_eq!(
                failure.map(|failure| failure.cause),
                expected,
                "{provider} alias={alias} requirement={has_requirement}"
            );
        }
    }
}
