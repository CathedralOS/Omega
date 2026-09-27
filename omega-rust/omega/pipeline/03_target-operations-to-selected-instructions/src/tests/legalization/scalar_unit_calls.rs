//! True Unit calls preserve scalar arguments, continuation and independent replay.
use abstract_operations::{AbstractOperation, AbstractParameter};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetUnitOperation, TargetUnitScalarArgumentSource};

use crate::{legalize_target_operations, validate_legalized_operations};

fn targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ]
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
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, targeted, unit)
}

#[test]
fn scalar_unit_call_target_and_legalized_replay_reject_transport_substitution() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    for native in targets() {
        let (source, target, unit) = fixture(native, scalar_type);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..9 {
            let mut changed = target.clone();
            let body = &mut changed.functions[0].graph;
            if mutation == 8 {
                body.blocks[0].operations.clear();
            } else {
                let TargetUnitOperation::Call {
                    psi_operation,
                    callee,
                    call_plan,
                    scalar_arguments,
                    ..
                } = &mut body.blocks[0].operations[0]
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
