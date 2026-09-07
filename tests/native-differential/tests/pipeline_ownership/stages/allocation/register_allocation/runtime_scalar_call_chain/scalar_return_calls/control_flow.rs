//! Authored Terminal branches carry actual call results into a shared return.

use super::*;

#[test]
fn branch_calls_and_join_parameters_reach_common_native_publication() {
    for (equal, expected) in [(true, 37), (false, 41)] {
        let (semantic, proof) = branch_call_artifact(equal);
        let mut shuffled = terminal_codec::decode_module(&semantic).unwrap();
        // Keep Terminal's canonical BlockId order, but number the join before
        // both arms. Execution follows edges, not the numeric block order.
        let middle = &mut shuffled.machines[1];
        let join = BlockId::new(28_145).unwrap();
        middle.blocks[3].id = join;
        for arm in &mut middle.blocks[1..3] {
            let Terminator::Jump { target, .. } = &mut arm.terminator else {
                unreachable!()
            };
            *target = join;
        }
        middle.blocks.sort_by_key(|block| block.id);
        let shuffled = terminal_codec::encode_module(&shuffled).unwrap();
        publish_scalar_artifacts(expected, [(semantic, proof.clone()), (shuffled, proof)]);
    }
}

#[test]
fn branch_call_selection_rejects_changed_join_bindings_and_edges() {
    let (semantic, proof) = branch_call_artifact(true);
    let selections = OptimizationSelections::default();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let staged = stage_optimized_instruction_selection(
        lower_optimized_to_target_operations(optimized, NativeTarget::windows_x64()).unwrap(),
    )
    .unwrap();
    for mutation in 0..5 {
        let mut changed = staged.selected().plan().clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.machine.get() == 28_101)
            .unwrap();
        let arm = function
            .blocks
            .iter_mut()
            .find(|block| block.source_block.get() == 28_150)
            .unwrap();
        let SelectedTerminator::Jump { successor, .. } = &mut arm.terminator else {
            unreachable!()
        };
        match mutation {
            0 => successor.bindings[0].semantic.argument = ValueId::new(28_150).unwrap(),
            1 => successor.source_target = BlockId::new(28_160).unwrap(),
            2 => successor.psi_edge = EdgeId::new(28_161).unwrap(),
            3 => {
                successor.bindings[0].transport =
                    selected_instructions::SelectedValueTransport::Unused
            }
            _ => {
                let selected_instructions::SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = &mut successor.bindings[0].transport
                else {
                    panic!("live join transfer")
                };
                std::mem::swap(argument, parameter);
            }
        }
        assert!(validate_raw_selection(&staged, changed).is_err());
    }
}

fn branch_call_artifact(equal: bool) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = artifact(37);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let scalar_type = module.machines[1].parameters[0].scalar_type;
    let value = |raw| ValueId::new(raw).unwrap();
    let block = |raw| BlockId::new(raw).unwrap();
    let edge = |raw| EdgeId::new(raw).unwrap();
    let declaration = |raw| ValueDeclaration {
        id: value(raw),
        scalar_type,
    };
    let constant = |raw, literal| Operation {
        id: OperationId::new(raw).unwrap(),
        result: OperationResult::Scalar(declaration(raw)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(literal),
        },
    };
    let entry = &mut module.machines[0].blocks[0];
    entry
        .operations
        .insert(1, constant(28_040, if equal { 37 } else { 38 }));
    let OperationKind::Call { arguments, .. } = &mut entry.operations[2].kind else {
        unreachable!()
    };
    arguments.push(value(28_040));
    let callee = module.machines[2].id;
    let middle = &mut module.machines[1];
    middle.parameters.push(declaration(28_106));
    let successor = |raw| SuccessorEdge {
        edge: edge(raw),
        target: block(raw),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let arm = |raw, literal| Block {
        id: block(raw),
        parameters: Vec::new(),
        operations: vec![
            constant(raw, literal),
            Operation {
                id: OperationId::new(raw + 1).unwrap(),
                result: OperationResult::Scalar(declaration(raw + 1)),
                kind: OperationKind::Call {
                    callee,
                    arguments: vec![value(raw)],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        ],
        terminator: Terminator::Jump {
            edge: edge(raw + 1),
            target: block(28_170),
            arguments: vec![value(raw + 1)],
            trivial_affine_discards: Vec::new(),
        },
    };
    middle.blocks = vec![
        Block {
            id: middle.entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(28_140).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value(28_140),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::IntegerEqual {
                    left: middle.parameters[0].id,
                    right: value(28_106),
                },
            }],
            terminator: Terminator::Conditional {
                condition: value(28_140),
                when_true: successor(28_150),
                when_false: successor(28_160),
            },
        },
        arm(28_150, 37),
        arm(28_160, 41),
        Block {
            id: block(28_170),
            parameters: vec![declaration(28_170)],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge(28_170),
                value: value(28_170),
                cleanup_actions: Vec::new(),
            },
        },
    ];
    (terminal_codec::encode_module(&module).unwrap(), proof)
}
