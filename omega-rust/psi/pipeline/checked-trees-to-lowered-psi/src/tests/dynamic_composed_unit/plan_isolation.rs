use super::{DIRECT_DYNAMIC_SOURCE, DIRECT_DYNAMIC_UNIT_SOURCE};
use crate::tests::{checked_source, lower_machine};

#[test]
fn unsupported_scalar_call_does_not_discard_other_machines_dispatch_plans() {
    assert_neighboring_machine_is_isolated(
        DIRECT_DYNAMIC_SOURCE,
        r#"
        machine Main::unsupported(&self) {
            let erased: &dyn Measure = &self.item as &dyn Item::Primary;
            let first: bool = erased.measure();
            let second: bool = erased.measure();
        }
        "#,
    );
}

#[test]
fn unsupported_unit_call_does_not_discard_other_machines_dispatch_plans() {
    assert_neighboring_machine_is_isolated(
        DIRECT_DYNAMIC_UNIT_SOURCE,
        r#"
        machine Main::unsupported(&self) {
            let erased: &dyn Touch = &self.item as &dyn Item::Primary;
            erased.touch();
            erased.touch();
        }
        "#,
    );
}

fn assert_neighboring_machine_is_isolated(supported: &str, unsupported: &str) {
    // Exercise both already-collected plans and calls visited after the omission.
    for source in [
        format!("{supported}\n{unsupported}"),
        format!("{unsupported}\n{supported}"),
    ] {
        let checked = checked_source(&source);
        let lowered = lower_machine(&checked, "Main::run")
            .expect("an unsupported neighboring call must not erase complete dispatch custody");
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("the supported machine retains valid dynamic dispatch");
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
            .produce_artifact()
            .expect("the supported machine produces a checked artifact");
        let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the supported machine round-trips");
        assert_eq!(decoded, lowered.semantic_module);
        assert_supported_artifact_executes(&artifact, &decoded);

        assert!(
            lower_machine(&checked, "Main::unsupported").is_err(),
            "retaining complete call plans must not silently drop unsupported calls"
        );
    }
}

fn assert_supported_artifact_executes(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    module: &terminal_psi::TerminalModule,
) {
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("supported entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("fixture has one structural receiver")
    };
    let [selection] = module.dynamic_dispatch.selections.as_slice() else {
        panic!("fixture has one dynamic selection")
    };
    let boolean_fields = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::BooleanStructuralField { field, .. } => {
                Some(terminal_interpreter::TerminalStructuralBooleanFieldValue {
                    argument_index: 0,
                    path: selection.source.path.clone(),
                    field,
                    value: true,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs {
            arguments: &[argument],
            boolean_fields: &boolean_fields,
            ..Default::default()
        },
    )
    .expect("supported artifact starts");
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                &mut terminal_interpreter::AcceptTerminalEffects,
            )
            .expect("supported artifact executes"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
}
