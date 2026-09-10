//! True Unit calls preserve scalar arguments, continuation and independent replay.
use abstract_operations::{AbstractOperation, AbstractParameter};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetOperation, TargetUnitOperation, TargetUnitScalarArgumentSource};

use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ]
}

fn scalar_types() -> Vec<ScalarType> {
    std::iter::once(ScalarType::Boolean)
        .chain([8, 16, 32, 64].into_iter().flat_map(|bits| {
            [IntegerSign::Signed, IntegerSign::Unsigned]
                .into_iter()
                .map(move |sign| ScalarType::Integer(IntegerType::new(sign, bits).unwrap()))
        }))
        .collect()
}

fn fixture(
    native: NativeTarget,
    scalar_type: ScalarType,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let mut callee = source.functions[0].clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(2).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters = [200, 201]
        .map(|value| AbstractParameter {
            value: ValueId::new(value).unwrap(),
            scalar_type,
        })
        .to_vec();
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    let caller = &mut source.functions[0];
    caller.parameters = [100, 101]
        .map(|value| AbstractParameter {
            value: ValueId::new(value).unwrap(),
            scalar_type,
        })
        .to_vec();
    // Both caller inputs remain live across the first call. Reordering and
    // duplication here are authored arguments, not corrupted transport proposals.
    caller.operations.splice(
        0..0,
        [[100, 101], [101, 100], [100, 100]]
            .into_iter()
            .enumerate()
            .map(|(call_index, arguments)| AbstractOperation::CallUnit {
                psi_operation: OperationId::new(10 + call_index as u64).unwrap(),
                callee: callee.machine,
                arguments: arguments.map(|value| ValueId::new(value).unwrap()).to_vec(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }),
    );
    source.functions.push(callee);
    let targeted =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, targeted, unit)
}

#[test]
fn scalar_unit_calls_forward_all_fixed_integer_and_boolean_parameters() {
    for native in targets() {
        for scalar_type in scalar_types() {
            let (source, target, unit) = fixture(native, scalar_type);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let caller = &legal.plan().scalar_functions[0];
            assert_eq!(caller.blocks[0].instructions.len(), 3);
            for (row, arguments) in
                caller.blocks[0]
                    .instructions
                    .iter()
                    .zip([[100, 101], [101, 100], [100, 100]])
            {
                assert!(row.result.is_none());
                let legalized_operations::LegalizedScalarInstructionKind::Call(call) = &row.kind
                else {
                    panic!("ordinary Unit call");
                };
                assert!(call.result_placement.is_none());
                assert_eq!(
                    call.arguments
                        .iter()
                        .map(|argument| argument.scalar_source().unwrap().get())
                        .collect::<Vec<_>>(),
                    arguments
                );
            }
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap_or_else(|error| {
                panic!("scalar Unit selection {native:?}, {scalar_type:?}: {error:?}")
            });
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let caller = &selected.plan().functions[0];
            let calls = caller.blocks[0]
                .instructions
                .iter()
                .filter(|row| {
                    matches!(
                        row.kind,
                        selected_instructions::SelectedInstructionKind::CallUnit { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 3);
            for call in calls {
                assert_eq!(
                    call.operands.len(),
                    2,
                    "Unit has arguments only, never a dummy result"
                );
                assert_eq!(call.constraint, constraints.keys.call_unit[2]);
            }
            assert!(caller.local_storage_slots.is_empty());
            assert!(caller.outgoing_arguments.is_empty());
        }
    }
}

#[test]
fn scalar_unit_call_target_and_legalized_replay_reject_transport_substitution() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    for native in targets() {
        let (source, target, unit) = fixture(native, scalar_type);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..9 {
            let mut changed = target.clone();
            let TargetOperation::UnitBody(body) = &mut changed.functions[0].operation else {
                panic!("Unit body");
            };
            if mutation == 8 {
                body.operations.swap(0, 1);
            } else {
                let TargetUnitOperation::Call {
                    psi_operation,
                    callee,
                    call_plan,
                    scalar_arguments,
                    ..
                } = &mut body.operations[0]
                else {
                    panic!("Unit call");
                };
                match mutation {
                    0 => *psi_operation = OperationId::new(99).unwrap(),
                    1 => *callee = MachineId::new(99).unwrap(),
                    2 => {
                        scalar_arguments.pop();
                    }
                    3 => scalar_arguments.swap(0, 1),
                    4 => scalar_arguments[1].source = scalar_arguments[0].source,
                    5 => {
                        let TargetUnitScalarArgumentSource::Parameter { scalar_type, .. } =
                            &mut scalar_arguments[0].source
                        else {
                            panic!("parameter");
                        };
                        *scalar_type = ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        );
                    }
                    6 => call_plan.parameters.swap(0, 1),
                    _ => call_plan.shadow_bytes = if call_plan.shadow_bytes == 0 { 32 } else { 0 },
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}, {native:?}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                    .is_err()
            );
        }
        for mutation in 0..9 {
            let mut changed = legal.plan().clone();
            let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
            if mutation == 8 {
                rows.swap(0, 1);
            } else {
                let row = &mut rows[0];
                let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
                    &mut row.kind
                else {
                    panic!("Unit call");
                };
                match mutation {
                    0 => row.operation = OperationId::new(99).unwrap(),
                    1 => call.callee = MachineId::new(99).unwrap(),
                    2 => {
                        call.arguments.pop();
                    }
                    3 => call.arguments.swap(0, 1),
                    4 => {
                        let source = call.arguments[0].scalar_source().unwrap();
                        let legalized_operations::LegalizedScalarArgument::Scalar {
                            source: destination,
                            ..
                        } = &mut call.arguments[1]
                        else {
                            panic!("scalar argument");
                        };
                        *destination = source;
                    }
                    5 => {
                        row.result = Some(legalized_operations::LegalizedValueDefinition {
                            value: ValueId::new(999).unwrap(),
                            scalar_type,
                            definition_site: optimization_unit::ValueDefinitionSite::Node {
                                block: source.functions[0].entry,
                                node: 0,
                            },
                        })
                    }
                    6 => call.result_placement = Some(call.call_plan.parameters[0].clone()),
                    _ => row.fuel.clear(),
                }
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "legalized mutation {mutation}, {native:?}"
            );
        }
    }
}

#[test]
fn scalar_unit_selected_replay_binds_no_result_abi_arguments_order_and_clobbers() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for native in targets() {
        let (source, target, unit) = fixture(native, scalar_type);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
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
        let positions = selected.plan().functions[0].blocks[0]
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(position, row)| {
                matches!(
                    row.kind,
                    selected_instructions::SelectedInstructionKind::CallUnit { .. }
                )
                .then_some(position)
            })
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 3);
        for mutation in 0..8 {
            let mut changed = selected.plan().clone();
            let rows = &mut changed.functions[0].blocks[0].instructions;
            if mutation == 7 {
                rows.swap(positions[0], positions[1]);
            } else {
                let row = &mut rows[positions[0]];
                match mutation {
                    0 => {
                        row.kind = selected_instructions::SelectedInstructionKind::CallScalar {
                            callee: MachineId::new(2).unwrap(),
                        }
                    }
                    1 => {
                        row.kind = selected_instructions::SelectedInstructionKind::CallUnit {
                            callee: MachineId::new(99).unwrap(),
                        }
                    }
                    2 => row.operands.swap(0, 1),
                    3 => row.operands[1] = row.operands[0],
                    4 => row.constraint = constraints.keys.call_unit[1],
                    5 => row.clobbers.clear(),
                    _ => row.provenance.operations.clear(),
                }
            }
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "selected mutation {mutation}, {native:?}"
            );
        }
    }
}
