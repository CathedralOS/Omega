//! Continuations cross the public target-stage gate and ordinary graph replay.
use super::*;
use abstract_operations_to_target_operations::AbstractToTargetFunctionTranslationDisposition;

fn continuation_source(native: NativeTarget) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = fixture(native);
    let caller = &mut source.functions[0];
    caller.parameters.remove(0);
    caller.block_entries = [(1, 0), (4, 2)]
        .map(|(identity, operation_offset)| AbstractBlockEntry {
            block: block(identity),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operation_offset,
        })
        .to_vec();
    caller.operations = vec![
        call(11, 101),
        jump(12),
        call(13, 101),
        AbstractOperation::ReturnUnit {
            psi_edge: edge(14),
            cleanup_actions: Vec::new(),
        },
    ];
    source
}

fn bind_successor(source: &mut abstract_operations::AbstractOperationPlan) {
    let caller = &mut source.functions[0];
    let scalar_type = caller.parameters[0].scalar_type;
    caller.block_entries[1].parameters = vec![AbstractParameter {
        value: value(102),
        scalar_type,
    }];
    let AbstractOperation::Jump { bindings, .. } = &mut caller.operations[1] else {
        panic!("jump");
    };
    bindings.push(abstract_operations::ValueBinding {
        parameter: value(102),
        argument: value(101),
        scalar_type,
    });
    caller.operations[2] = call(13, 102);
}

#[test]
fn linear_unit_continuations_cross_translation_and_selected_replay() {
    for (native, bind) in targets()
        .into_iter()
        .flat_map(|native| [(native, false), (native, true)])
    {
        let mut source = continuation_source(native);
        if bind {
            bind_successor(&mut source);
        }
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let receipt =
            abstract_operations_to_target_operations::validate_abstract_to_target_translation(
                &source, native, &target,
            )
            .unwrap();
        assert!(matches!(
            receipt.function_roster()[0].translation(),
            AbstractToTargetFunctionTranslationDisposition::Uncovered
        ));
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        assert_eq!(legal.plan().scalar_functions[0].blocks.len(), 2);
        assert_eq!(
            selected.plan().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|instruction| matches!(
                    instruction.kind,
                    selected_instructions::SelectedInstructionKind::CallUnit { .. }
                ))
                .count(),
            2
        );
    }
}

#[test]
fn linear_continuation_corruption_cannot_bypass_mandatory_graph_replay() {
    for native in targets() {
        let mut source = continuation_source(native);
        bind_successor(&mut source);
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..9 {
            let mut changed = target.clone();
            let function = &mut changed.functions[0];
            let TargetOperation::ControlGraph(graph) = &mut function.operation else {
                panic!("graph");
            };
            match mutation {
                0 => {
                    let TargetUnitOperation::Call { callee, .. } =
                        &mut graph.blocks[0].operations[0]
                    else {
                        panic!("call");
                    };
                    *callee = source.entry;
                }
                1 => {
                    graph.blocks[0].operations.clear();
                }
                2..=6 => {
                    let TargetControlTerminator::Jump { successor } =
                        &mut graph.blocks[0].terminator
                    else {
                        panic!("jump");
                    };
                    match mutation {
                        2 => successor.target = block(1),
                        3 => successor.psi_edge = edge(99),
                        4 => successor.bindings[0].argument = value(999),
                        5 => successor.bindings[0].scalar_type = ScalarType::Boolean,
                        _ => successor.cleanup_actions.push(
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                                semantic_vocabulary::PlaceId::new(99).unwrap(),
                            ),
                        ),
                    }
                }
                7 => function.provenance.operations.swap(0, 1),
                _ => graph.blocks[1].parameters[0].scalar_type = ScalarType::Boolean,
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "construction mutation {mutation}: {native:?}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                    .is_err(),
                "replay mutation {mutation}: {native:?}"
            );
        }
    }
}
