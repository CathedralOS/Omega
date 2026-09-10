//! Only the selected arm constructs and calls; its scalar continuation can rejoin.

use super::*;

fn selected_module(dimensions: &[u64], leaves: &[TerminalScalarValue]) -> TerminalModule {
    let mut module = module(dimensions, false, false, leaves);
    let caller = &mut module.machines[0];
    caller.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(9000),
        scalar_type: ScalarType::Boolean,
    });
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(9001),
        scalar_type: ScalarType::Boolean,
    });
    caller
        .structural_places
        .retain(|place| !matches!(place.kind, semantic_vocabulary::StructuralPlaceKind::Result));
    let operations = std::mem::take(&mut caller.blocks[0].operations);
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(9000),
        when_true: SuccessorEdge {
            edge: edge_id(10000),
            target: block_id(10001),
            arguments: vec![],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
        when_false: SuccessorEdge {
            edge: edge_id(10001),
            target: block_id(10002),
            arguments: vec![],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
    };
    for (identity, operations) in [(10001, operations), (10002, vec![])] {
        caller.blocks.push(Block {
            id: block_id(identity),
            parameters: vec![],
            structural_parameters: vec![],
            operations,
            terminator: Terminator::Jump {
                edge: edge_id(identity + 1),
                target: block_id(10003),
                arguments: vec![],
                structural_arguments: vec![],
                trivial_affine_discards: vec![],
                residual_affine_discards: vec![],
            },
        });
    }
    caller.blocks.push(Block {
        id: block_id(10003),
        parameters: vec![],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: edge_id(10004),
            value: value_id(9000),
            cleanup_actions: vec![],
        },
    });
    module
}

#[test]
fn selected_array_calls_rejoin_and_resume_without_running_the_skipped_arm() {
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for dimensions in [vec![2], vec![2, 2], vec![0], vec![1, 0], vec![0, 2]] {
        let leaves = (0..dimensions.iter().product::<u64>())
            .map(|position| byte(7 + position as u8))
            .collect::<Vec<_>>();
        let module = selected_module(&dimensions, &leaves);
        let semantic = encode_module(&module).unwrap();
        assert_eq!(decode_module(&semantic).unwrap(), module);
        for selected in [false, true] {
            let arguments = [TerminalScalarValue::Boolean(selected)];
            let expected = TerminalExecutionResult::Scalar(arguments[0]);
            let measured = interpret_terminal_artifact_measured(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &arguments,
            )
            .expect("selected branch-local arrays execute after independent verification");
            assert_eq!(measured.value(), expected);
            if selected {
                assert!(measured.usage().total_units() > 3);
            } else {
                assert_eq!(
                    measured.usage().total_units(),
                    3,
                    "only conditional, jump and return execute"
                );
            }
            let mut execution = TerminalExecution::start_artifact(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &arguments,
            )
            .unwrap();
            let mut meter = TerminalFuelMeter::with_allowance(0);
            for _ in 0..measured.usage().total_units() {
                assert!(matches!(
                    execution.resume(&mut meter).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                meter.replenish(1).unwrap();
            }
            assert_eq!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(expected)
            );
            assert_eq!(meter.usage(), measured.usage());
        }
    }
}
