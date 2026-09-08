//! Raw target projection retains a derived producer; proof admission is separate.
use super::*;

#[test]
fn block_descriptor_call_retains_block_identity_without_fabricated_producer() {
    let mut plan = fixture();
    let caller = &mut plan.functions[0];
    let block = BlockId::new(30).unwrap();
    let place = PlaceId::new(31).unwrap();
    let mut declaration = caller.structural_parameters[0].clone();
    declaration.place = place;
    for operation in &mut caller.operations {
        if let AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        } = operation
        {
            structural_arguments[0].place = place;
        }
    }
    caller.operations.insert(
        0,
        AbstractOperation::Jump {
            psi_edge: EdgeId::new(32).unwrap(),
            target: block,
            bindings: Vec::new(),
            structural_bindings: vec![abstract_operations::AbstractStructuralBinding {
                parameter: place,
                argument: StructuralArgument {
                    place: caller.structural_parameters[0].place,
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                },
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    caller.block_entries.push(AbstractBlockEntry {
        block,
        parameters: Vec::new(),
        structural_parameters: vec![declaration],
        operation_offset: 1,
    });
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &lowered.functions[0].operation
        else {
            panic!("scalar call");
        };
        let TargetIntegerExpression::StructuralCall {
            structural_arguments,
            ..
        } = expression
        else {
            panic!("structural call");
        };
        assert_eq!(
            structural_arguments[0].source,
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place }
        );
    }
}

fn subslice_calls() -> AbstractOperationPlan {
    let mut plan = fixture();
    let caller = &mut plan.functions[0];
    let parameter = caller.structural_parameters[0].place;
    let structural_type = caller.structural_parameters[0].structural_type;
    let length = ValueId::new(20).unwrap();
    let place = PlaceId::new(21).unwrap();
    for operation in &mut caller.operations {
        if let AbstractOperation::CallStructuralScalar {
            structural_arguments,
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

#[test]
fn repeated_calls_retain_exact_subslice_producer_without_a_parameter_home() {
    let plan = subslice_calls();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &lowered.functions[0].operation
        else {
            panic!("scalar call result");
        };
        let TargetIntegerExpression::StructuralCall {
            structural_arguments,
            ..
        } = expression
        else {
            panic!("ordinary shared call");
        };
        assert_eq!(structural_arguments[0].place, PlaceId::new(21).unwrap());
        assert_eq!(
            structural_arguments[0].source,
            target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: OperationId::new(21).unwrap(),
            }
        );
        assert_eq!(
            lowered.functions[0]
                .mixed_structural_scalar_abi
                .as_ref()
                .unwrap()
                .structural_parameters
                .len(),
            1
        );
        for mutation in 0..3 {
            let mut changed = plan.clone();
            let AbstractOperation::ByteSequenceSubslice { result, .. } =
                &mut changed.functions[0].operations[1]
            else {
                panic!("subslice fixture");
            };
            match mutation {
                0 => result.multiplicity = StructuralMultiplicity::Affine,
                1 => result.structural_type = StructuralTypeId::new(99).unwrap(),
                _ => result
                    .claims
                    .push(terminal_psi::StructuralResultClaimBinding {
                        claim: semantic_vocabulary::ClaimId::new(99).unwrap(),
                        path: Vec::new(),
                    }),
            }
            assert!(
                lower_to_target_operations(&changed, target).is_err(),
                "source mutation {mutation}"
            );
        }
    }
}

fn unit_subslice_calls() -> AbstractOperationPlan {
    let mut plan = subslice_calls();
    for function in &mut plan.functions {
        function.result = AbstractFunctionResult::Unit;
        for operation in &mut function.operations {
            *operation = match operation.clone() {
                AbstractOperation::CallStructuralScalar {
                    psi_operation,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => AbstractOperation::CallUnit {
                    psi_operation,
                    callee,
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
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
fn unit_calls_retain_once_only_length_and_subslice_establishment() {
    let plan = unit_subslice_calls();
    let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    let TargetOperation::UnitGraph(graph) = &lowered.functions[0].operation else {
        panic!("Unit graph");
    };
    let operations = &graph.blocks[0].operations;
    assert!(matches!(
        operations[0],
        target_operations::TargetUnitOperation::ScalarDefinition { .. }
    ));
    assert!(matches!(
        operations[1],
        target_operations::TargetUnitOperation::ByteSequenceSubslice { .. }
    ));
    for call in &operations[2..] {
        let target_operations::TargetUnitOperation::Call { arguments, .. } = call else {
            panic!("Unit call");
        };
        assert_eq!(
            arguments[0].source,
            target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: OperationId::new(21).unwrap(),
            }
        );
    }
    assert_eq!(
        lowered.functions[0].provenance.operations,
        vec![
            OperationId::new(20).unwrap(),
            OperationId::new(21).unwrap(),
            OperationId::new(3).unwrap(),
            OperationId::new(4).unwrap()
        ]
    );
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
            lower_to_target_operations(&plan, NativeTarget::linux_x64()).is_err(),
            "mutation {mutation}"
        );
    }
}
