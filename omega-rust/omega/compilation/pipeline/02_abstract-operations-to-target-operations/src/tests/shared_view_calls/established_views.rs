//! Raw target projection retains a derived producer; proof admission is separate.

use super::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractParameter, AbstractResult, BlockId, EdgeId, NativeTarget, OperationId, PlaceId,
    ScalarType, StructuralMultiplicity, TargetLoweringRequest, ValueId, fixture,
    lower_to_target_operations,
};
use abstract_operations::AbstractStructuralCallResult;
fn subslice_calls() -> AbstractOperationPlan {
    let mut plan = fixture();
    let caller = &mut plan.functions[0];
    let parameter = caller.structural_parameters[0].place;
    let structural_type = caller.structural_parameters[0].structural_type;
    let length = ValueId::new(20).unwrap();
    let place = PlaceId::new(21).unwrap();
    for operation in &mut caller.operations {
        if let AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Scalar(_),
            ..
        } = operation
        {
            structural_arguments[0].place = place;
        }
    }
    caller.operations.insert(
        0,
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(20).unwrap(),
            result: AbstractResult {
                value: length,
                scalar_type: caller.parameters[0].scalar_type,
            },
            source: parameter,
        },
    );
    caller.operations.insert(
        1,
        AbstractOperation::ByteSequenceSubslice {
            psi_operation: OperationId::new(21).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            source: parameter,
            start: caller.parameters[0].value,
            end: length,
            length,
            obligation: semantic_vocabulary::ObligationId::new(21).unwrap(),
        },
    );
    plan
}

fn unit_subslice_calls() -> AbstractOperationPlan {
    let mut plan = subslice_calls();
    for function in &mut plan.functions {
        function.result = AbstractFunctionResult::Unit;
        for operation in &mut function.operations {
            *operation = match operation.clone() {
                AbstractOperation::CallStructural {
                    psi_operation,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    result: AbstractStructuralCallResult::Scalar(_),
                    ..
                } => AbstractOperation::CallStructural {
                    psi_operation,
                    callee,
                    arguments,
                    structural_arguments,
                    dynamic_arguments: Vec::new(),
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    result: AbstractStructuralCallResult::Unit,
                },
                AbstractOperation::Return {
                    psi_edge,
                    cleanup_actions,
                    ..
                } => AbstractOperation::ReturnUnit {
                    psi_edge,
                    cleanup_actions,
                },
                other => other,
            };
        }
    }
    plan
}

#[test]
fn unit_calls_reject_future_duplicate_and_sibling_view_producers() {
    for mutation in 0..3 {
        let mut plan = unit_subslice_calls();
        let caller = &mut plan.functions[0];
        match mutation {
            0 => caller.operations.swap(1, 2),
            1 => caller.operations.insert(2, caller.operations[1].clone()),
            _ => {
                let condition = ValueId::new(90).unwrap();
                caller.parameters.push(AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                });
                let successor = |edge, block| abstract_operations::AbstractSuccessor {
                    structural_bindings: Vec::new(),
                    psi_edge: EdgeId::new(edge).unwrap(),
                    target: BlockId::new(block).unwrap(),
                    bindings: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                };
                caller.operations.insert(
                    1,
                    AbstractOperation::Conditional {
                        condition,
                        when_true: successor(30, 30),
                        when_false: successor(31, 31),
                    },
                );
                caller.operations.insert(
                    3,
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(32).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                );
                caller.block_entries = [(1, 0), (30, 2), (31, 4)]
                    .into_iter()
                    .map(|(block, operation_offset)| AbstractBlockEntry {
                        structural_parameters: Vec::new(),
                        block: BlockId::new(block).unwrap(),
                        parameters: Vec::new(),
                        operation_offset,
                    })
                    .collect();
            }
        }
        assert!(
            lower_to_target_operations(
                &plan,
                TargetLoweringRequest::new(NativeTarget::linux_x64())
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn qualified_subslice_calls_retain_descriptor_geometry_and_exact_source() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for mut source in [subslice_calls(), unit_subslice_calls()] {
            let plain = lower_to_target_operations(&source, TargetLoweringRequest::new(native))
                .expect("plain subslice call");
            // Raw target projection does not decide membership. Its semantic
            // caller has proved the no-NUL formal from the exact backing;
            // the derived result remains unannotated, with the same descriptor.
            let domain = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
            for function in &mut source.functions {
                function.structural_parameters[0]
                    .qualifications
                    .push(domain);
            }
            let qualified = lower_to_target_operations(&source, TargetLoweringRequest::new(native))
                .expect("qualified contents preserve descriptor geometry");
            crate::validate_abstract_to_target_translation(&source, native, &qualified).unwrap();
            let calls = |plan: &target_operations::TargetOperationPlan| {
                plan.functions[0]
                    .graph
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match operation {
                        target_operations::TargetUnitOperation::Call { arguments, .. } => {
                            Some(arguments.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(calls(&plain), calls(&qualified));
            assert!(!calls(&qualified).is_empty());
            let mut forged = qualified;
            let arguments = forged.functions[0]
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    target_operations::TargetUnitOperation::Call { arguments, .. } => {
                        Some(arguments)
                    }
                    _ => None,
                })
                .unwrap();
            arguments[0].place = source.functions[0].structural_parameters[0].place;
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &forged).is_err()
            );
        }
    }
}
