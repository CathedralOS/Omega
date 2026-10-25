//! The complete source Build entry, with real host binding effects and helper
//! writeback. Product bodies stay inert; fuel pauses resume the same activation.

use build_evaluation::terminal_bindings::TerminalBuildBindingHost;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffectHandler, TerminalEntryStorageError, TerminalEntryStorageValue,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalStructuralInputs,
    TerminalStructuralValue,
};

pub(super) fn observe(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    bindings: &psi_compiler::TerminalBuildBindings,
    slots: &str,
    entry: &str,
    identifier: &str,
) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exercise(
            artifact,
            bindings,
            &slots.split(',').collect::<Vec<_>>(),
            entry,
            if identifier == "-" { "" } else { identifier },
        )
    }))
    .map_err(|failure| {
        failure
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                failure
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| "full Build binding provision failed".to_owned())
    })
}

fn exercise(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    bindings: &psi_compiler::TerminalBuildBindings,
    slots: &[&str],
    entry: &str,
    expected_identifier: &str,
) {
    let identifier = [super::mutable_boundary::field(artifact, "identifier")];
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).expect("external artifact");
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        let original = TerminalStructuralValue::minted(
            951,
            program.entry().unwrap().structural_parameters[0].structural_type,
            Vec::new(),
        );
        for foreign in [false, true] {
            let host = TerminalBuildBindingHost::new(artifact, bindings, original.clone()).unwrap();
            let mut receiver = original.clone();
            if foreign {
                receiver.opaque_identity += 1;
            }
            let mut instance = program
                .instantiate(
                    &[],
                    TerminalStructuralInputs {
                        arguments: std::slice::from_ref(&receiver),
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                )
                .unwrap();
            let imports = instance.unbound_imports().collect::<Vec<_>>();
            assert_eq!(
                imports.len(),
                2,
                "prepare and commit, not product execution"
            );
            for boundary in &imports {
                let declaration = artifact
                    .semantic_module()
                    .boundary_machines
                    .iter()
                    .find(|item| item.id == *boundary)
                    .unwrap();
                instance
                    .bind_import(*boundary, host.adapter(declaration.into()))
                    .unwrap();
            }
            if foreign {
                let error = instance
                    .invoke(&mut TerminalFuelMeter::unbounded())
                    .unwrap_err();
                assert!(format!("{error:?}").contains("original compiler-issued Build"));
                assert_eq!(
                    instance.observe_borrowed_entry(0, &identifier),
                    Err(TerminalEntryStorageError::NormalReturnRequired)
                );
                drop(instance);
                assert!(host.into_executed().unwrap().is_empty());
                continue;
            }
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            let mut paused_after_effect = false;
            for _ in 0..1_000 {
                match instance.invoke(&mut fuel).unwrap() {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        paused_after_effect |= !instance.effects().is_empty();
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
                    status => panic!("unexpected full Build result: {status:?}"),
                }
            }
            assert!(instance.result().is_some());
            assert!(paused_after_effect);
            assert_eq!(
                instance.observe_borrowed_entry(0, &identifier).unwrap(),
                TerminalEntryStorageValue::Bytes(expected_identifier.as_bytes().to_vec())
            );
            assert_eq!(
                instance.effects().len(),
                slots.len() * 2,
                "one preparation and commit per executed binding"
            );
            let commit = instance.effects().iter().find(|effect| matches!(effect, terminal_interpreter::TerminalEffect::BoundaryCall { structural_arguments, .. } if structural_arguments.is_empty())).unwrap().clone();
            drop(instance);
            let terminal_interpreter::TerminalEffect::BoundaryCall { boundary, .. } = &commit
            else {
                panic!("boundary")
            };
            let declaration = artifact
                .semantic_module()
                .boundary_machines
                .iter()
                .find(|item| item.id == *boundary)
                .unwrap();
            let rejection = host
                .adapter(declaration.into())
                .handle_effect_result(&commit)
                .unwrap_err();
            assert!(
                rejection.reason.contains("consumed"),
                "a replayed ticket must refuse"
            );
            let mut fabricated = commit;
            let terminal_interpreter::TerminalEffect::BoundaryCall { operation, .. } =
                &mut fabricated
            else {
                panic!("boundary")
            };
            *operation = semantic_vocabulary::OperationId::new(u64::MAX).unwrap();
            let rejection = host
                .adapter(declaration.into())
                .handle_effect_result(&fabricated)
                .unwrap_err();
            assert!(rejection.reason.contains("no source-issued"));
            let requests = host.into_executed().unwrap();
            assert_eq!(
                requests
                    .iter()
                    .map(|request| request.slot.as_str())
                    .collect::<Vec<_>>(),
                slots
            );
            assert!(requests.iter().all(|request| matches!(&request.implementation, language_semantics::declaration_selection::RootBindingSelection::AuthoredPath(path) if path == entry)));
            eprintln!(
                "Terminal full Build: {} bindings once, identifier={expected_identifier}, original-receiver and ticket controls enforced",
                slots.len()
            );
        }
    }
}
