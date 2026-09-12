use super::*;
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};

#[test]
fn conditional_local_mutation_charges_only_the_selected_invocation() {
    let source = r#"
machine stamp(value: &mut u64, number: u64) -> bool { value = number; true }
machine enter(enabled: bool, number: u64) -> u64 {
    let mut slot: u64 = 201;
    let selected: bool = enabled && stamp(&mut slot, number);
    slot
}
"#;
    let checked = support::checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("conditional scalar-local artifact independently verifies");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(module.machines.len(), 2);
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(matches!(
        root.result,
        terminal_psi::TerminalMachineResult::Scalar(_)
    ));
    assert_eq!(root.parameters.len(), 2);
    assert!(root.structural_parameters.is_empty());
    let stamp = module
        .machines
        .iter()
        .find(|machine| machine.id != module.entry)
        .unwrap();
    let root_operations = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let calls = root_operations.iter().filter(|operation| {
            matches!(operation.kind, OperationKind::CallStructuralScalar { callee, .. } if callee == stamp.id)
        }).collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one authored conditional mutator call");
    for establishment in [false, true] {
        assert_eq!(
            root_operations
                .iter()
                .filter(|operation| if establishment {
                    matches!(
                        operation.kind,
                        OperationKind::EstablishPrimitiveLocal { .. }
                    )
                } else {
                    matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. })
                })
                .count(),
            1,
            "one establishment and one current-value read"
        );
    }
    assert_eq!(
        stamp
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::WriteOnlyPrimitiveStore { .. }
                )
            })
            .count(),
        1,
        "one shared mutator store body"
    );

    for enabled in [false, true] {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
        )
        .expect("reload conditional scalar-local artifact");
        let mut meter = TerminalFuelMeter::with_allowance(0);
        let mut complete = false;
        for _ in 0..128 {
            match execution
                .resume(&mut meter)
                .expect("resume conditional local mutation")
            {
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(
                        result,
                        TerminalExecutionResult::Scalar(unsigned(if enabled { 7 } else { 201 }))
                    );
                    complete = true;
                    break;
                }
                TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                other => panic!("unexpected conditional local outcome: {other:?}"),
            }
        }
        assert!(
            complete,
            "conditional local finishes with one-unit replenishments"
        );
        for operation in &root_operations {
            let usage = meter.usage().at(FuelChargeSite::Operation(operation.id));
            assert!(
                usage.map_or(0, |usage| usage.executions()) <= 1,
                "root operation cannot replay"
            );
            if matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
            ) {
                let usage = usage.expect("local is established and read on either path");
                assert_eq!(usage.executions(), 1);
                assert_eq!(usage.units(), 1);
            }
        }
        for operation in std::iter::once(*calls[0])
            .chain(stamp.blocks.iter().flat_map(|block| &block.operations))
        {
            let usage = meter.usage().at(FuelChargeSite::Operation(operation.id));
            assert_eq!(
                usage.map_or(0, |usage| usage.executions()),
                u64::from(enabled),
                "selected call/body executes once; skipped mutator never executes"
            );
            assert_eq!(
                usage.map_or(0, |usage| usage.units()),
                u64::from(enabled),
                "skipped mutator is not charged"
            );
        }
    }
}
