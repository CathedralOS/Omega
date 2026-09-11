use super::*;

fn diamond(stored: bool) -> AbstractOperationPlan {
    let mut plan = if stored {
        stored_descriptor_plan()
    } else {
        abstract_plan()
    };
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    caller.operations.pop().unwrap();
    let result = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamicScalar { result, .. }
            | AbstractOperation::CallStoredDynamicScalar { result, .. } => Some(*result),
            _ => None,
        })
        .unwrap();
    let condition = ValueId::new(90000).unwrap();
    if stored {
        caller.operations.push(AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(90000).unwrap(),
            result: condition,
            operand: result.value,
        });
    } else {
        caller.operations.extend([
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(90001).unwrap(),
                result: ValueId::new(90001).unwrap(),
                scalar_type: result.scalar_type,
                value: IntegerValue::Signed(0),
            },
            AbstractOperation::IntegerEqual {
                psi_operation: OperationId::new(90000).unwrap(),
                result: condition,
                left: result.value,
                right: ValueId::new(90001).unwrap(),
            },
        ]);
    }
    let successor = |number| AbstractSuccessor {
        psi_edge: EdgeId::new(number).unwrap(),
        target: BlockId::new(number).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.operations.push(AbstractOperation::Conditional {
        condition,
        when_true: successor(90002),
        when_false: successor(90003),
    });
    for number in [90003, 90002] {
        caller.block_entries.push(AbstractBlockEntry {
            block: BlockId::new(number).unwrap(),
            operation_offset: caller.operations.len(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        });
        caller.operations.extend([
            AbstractOperation::BooleanNot {
                psi_operation: OperationId::new(number).unwrap(),
                result: ValueId::new(number).unwrap(),
                operand: condition,
            },
            AbstractOperation::ReturnUnit {
                psi_edge: EdgeId::new(number + 2).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ]);
    }
    plan
}

#[test]
fn dynamic_results_compose_with_computations_and_reordered_arms() {
    for stored in [false, true] {
        let mut source = diamond(stored);
        let caller = source
            .functions
            .iter_mut()
            .find(|function| function.machine == source.entry)
            .unwrap();
        caller.operations.insert(
            0,
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(89999).unwrap(),
                result: ValueId::new(89999).unwrap(),
                value: false,
            },
        );
        for entry in &mut caller.block_entries[1..] {
            entry.operation_offset += 1;
        }
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let lowered =
                lower_to_target_operations(&source, target).expect("ordinary dynamic-call graph");
            let caller = lowered
                .functions
                .iter()
                .find(|function| function.machine == source.entry)
                .unwrap();
            let TargetOperation::ControlGraph(graph) = &caller.operation else {
                panic!("ordinary control graph")
            };
            assert_eq!(graph.blocks[1].block, BlockId::new(90003).unwrap());
            assert!(matches!(
                graph.blocks[0].terminator,
                target_operations::TargetControlTerminator::Conditional { .. }
            ));
            assert!(graph.blocks[0].operations.iter().any(|operation| matches!(
                operation,
                TargetUnitOperation::DynamicScalarCall { .. }
                    | TargetUnitOperation::StoredDynamicScalarCall { .. }
            )));
        }
    }
}

#[test]
fn stored_descriptor_availability_uses_dominance_not_authored_order() {
    let mut source = diamond(true);
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let store = caller.operations.remove(0);
    let old_entry = caller.entry;
    caller.entry = BlockId::new(90010).unwrap();
    for entry in &mut caller.block_entries[1..] {
        entry.operation_offset -= 1;
    }
    caller.block_entries.push(AbstractBlockEntry {
        block: caller.entry,
        operation_offset: caller.operations.len(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
    });
    caller.operations.extend([
        store,
        AbstractOperation::Jump {
            psi_edge: EdgeId::new(90010).unwrap(),
            target: old_entry,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    ]);
    lower_to_target_operations(&source, NativeTarget::linux_x64())
        .expect("descriptor store dominates call despite authored order");

    // A store in one branch does not dominate the call in the entry block.
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    caller.entry = old_entry;
    let store_block = caller.block_entries.pop().unwrap();
    caller.operations.truncate(store_block.operation_offset);
    let store = match &source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap()
        .operations[0]
    {
        AbstractOperation::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => AbstractOperation::StoreDynamicDescriptor {
            psi_operation: dynamic_dispatch.stored.descriptor.establishment_operation,
            stored: dynamic_dispatch.stored.clone(),
        },
        _ => panic!("stored call"),
    };
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let arm_offset = caller.block_entries[1].operation_offset;
    caller.operations.insert(arm_offset, store);
    caller.block_entries[2].operation_offset += 1;
    assert!(matches!(
        lower_to_target_operations(&source, NativeTarget::linux_x64()),
        Err(LoweringError::InvalidDynamicDispatch { .. })
    ));
}

fn with_exits(mut source: AbstractOperationPlan) -> AbstractOperationPlan {
    let boundary = BoundaryMachineId::new(90000).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    source.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "Console::exit_process(i32)->Unit".into(),
        attachment: None,
        scalar_parameters: vec![scalar_type],
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    for position in (1..caller.block_entries.len()).rev() {
        let insertion = caller.block_entries[position].operation_offset + 1;
        let number = 90100 + position as u64;
        let value = ValueId::new(number).unwrap();
        caller.operations.splice(
            insertion..insertion,
            [
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(number).unwrap(),
                    result: value,
                    scalar_type,
                    value: IntegerValue::Signed(i128::from(number)),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: OperationId::new(number + 10).unwrap(),
                    result: abstract_operations::AbstractBoundaryResult::Unit,
                    boundary,
                    arguments: vec![value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            ],
        );
        for entry in &mut caller.block_entries[position + 1..] {
            entry.operation_offset += 2;
        }
    }
    source
}

fn lower_exits(
    source: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<target_operations::TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements(
        source,
        target,
        &[BoundarySettlementBinding {
            boundary: BoundaryMachineId::new(90000).unwrap(),
            execution: target_operations::ProviderExecutionBinding::from_execution_record(
                target_operations::ProviderPlanReportIdentity::new(90000).unwrap(),
                90001,
                90002,
                90003,
                90004,
            )
            .unwrap()
            .into(),
            realization: target_operations::HostedExitProcessI32Realization.into(),
        }],
    )
}

#[test]
fn dynamic_conditional_exits_preserve_nominal_returns_and_nonreturning_rules() {
    for stored in [false, true] {
        let source = with_exits(diamond(stored));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let lowered = lower_exits(&source, target)
                .expect("dynamic conditional exits use the ordinary graph");
            let caller = lowered
                .functions
                .iter()
                .find(|function| function.machine == source.entry)
                .unwrap();
            let TargetOperation::ControlGraph(graph) = &caller.operation else {
                panic!("ordinary graph")
            };
            for block in &graph.blocks[1..] {
                assert!(matches!(
                    block.operations.last(),
                    Some(TargetUnitOperation::BoundarySettlement { .. })
                ));
                assert!(matches!(
                    block.terminator,
                    target_operations::TargetControlTerminator::Return { .. }
                ));
            }
        }
        let mut corrupted = source;
        let caller = corrupted
            .functions
            .iter_mut()
            .find(|function| function.machine == corrupted.entry)
            .unwrap();
        let last = caller.operations.len() - 1;
        caller.operations.insert(
            last,
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(90200).unwrap(),
                result: ValueId::new(90200).unwrap(),
                value: true,
            },
        );
        assert!(matches!(
            lower_exits(&corrupted, NativeTarget::linux_x64()),
            Err(LoweringError::InvalidHostedExitProcessShape(_))
        ));
    }
}

#[test]
fn dynamic_graph_rejects_duplicate_results_and_corrupt_descriptor_custody() {
    for stored in [false, true] {
        for duplicate_result in [false, true] {
            let mut source = diamond(stored);
            let caller = source
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            for operation in &mut caller.operations {
                match operation {
                    AbstractOperation::CallDynamicScalar {
                        result,
                        dynamic_dispatch,
                        ..
                    } => {
                        if duplicate_result {
                            result.value = ValueId::new(90000).unwrap();
                        } else {
                            dynamic_dispatch.dispatch.descriptor_ordinal += 1;
                        }
                    }
                    AbstractOperation::CallStoredDynamicScalar {
                        result,
                        dynamic_dispatch,
                        ..
                    } => {
                        if duplicate_result {
                            result.value = ValueId::new(90000).unwrap();
                        } else {
                            dynamic_dispatch.dispatch.descriptor_ordinal += 1;
                        }
                    }
                    _ => {}
                }
            }
            assert!(lower_to_target_operations(&source, NativeTarget::linux_x64()).is_err());
        }
    }
}

#[test]
fn dynamic_graph_keeps_independent_reference_header_validation() {
    let source = diamond(false);
    let target = NativeTarget::linux_x64();
    let mut lowered = lower_to_target_operations(&source, target).unwrap();
    crate::validate_abstract_to_target_translation(&source, target, &lowered)
        .expect("structural headers retain independent validation");
    let caller = lowered
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let TargetOperation::ControlGraph(graph) = &mut caller.operation else {
        panic!("ordinary graph")
    };
    graph.parameters[0].access = StructuralAccess::Owned;
    assert!(crate::validate_abstract_to_target_translation(&source, target, &lowered).is_err());
}

#[test]
fn dynamic_graph_borrow_support_does_not_admit_owned_or_linear_receivers() {
    for owned in [false, true] {
        let mut source = diamond(false);
        let caller = source
            .functions
            .iter_mut()
            .find(|function| function.machine == source.entry)
            .unwrap();
        if owned {
            caller.structural_parameters[0].access = StructuralAccess::Owned;
        } else {
            caller.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
        }
        assert!(matches!(
            lower_to_target_operations(&source, NativeTarget::linux_x64()),
            Err(LoweringError::UnsupportedControlFlow(_))
        ));
    }
}

#[test]
fn original_stored_boolean_exit_diamond_also_uses_ordinary_graph() {
    let mut source = with_exits(diamond(true));
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let original = std::mem::take(&mut caller.operations);
    let result = match &original[1] {
        AbstractOperation::CallStoredDynamicScalar { result, .. } => result.value,
        _ => panic!("stored call"),
    };
    let ranges = caller
        .block_entries
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            entry.operation_offset
                ..caller
                    .block_entries
                    .get(position + 1)
                    .map_or(original.len(), |next| next.operation_offset)
        })
        .collect::<Vec<_>>();
    for (entry, range) in caller.block_entries.iter_mut().zip(ranges) {
        entry.operation_offset = caller.operations.len();
        for operation in &original[range] {
            if matches!(operation, AbstractOperation::BooleanNot { .. }) {
                continue;
            }
            let mut operation = operation.clone();
            if let AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            } = &mut operation
            {
                *condition = result;
                std::mem::swap(when_true, when_false);
            }
            caller.operations.push(operation);
        }
    }
    assert_eq!(caller.operations.len(), 9);
    assert_eq!(caller.block_entries[1].operation_offset, 3);
    assert_eq!(caller.block_entries[2].operation_offset, 6);
    let lowered = lower_exits(&source, NativeTarget::linux_x64())
        .expect("original bounded diamond remains admitted");
    assert!(matches!(
        lowered
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap()
            .operation,
        TargetOperation::ControlGraph(_)
    ));
}
