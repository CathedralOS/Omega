use super::super::{LoweringError, TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, BlockId, EdgeId, IntegerSign,
    IntegerType, IntegerValue, MachineId, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeId,
    TargetUnitOperation, TargetUnitScalarArgumentSource, ValueId, identity,
};

#[test]
fn boolean_call_result_field_store_keeps_its_dominating_home() {
    let source = super::structural_borrows::source_plan(
        r#"
        data Helper { seen: i32; }
        machine Helper::check(&mut self, value: i32) -> bool {
            self.seen = value;
            transition self.seen == 1 { true -> yes() _ -> no() }
            state yes(&mut self) -> bool { true }
            state no(&mut self) -> bool { false }
        }
        data Main { helper: Helper; flag: bool; before: i32; after: i32; }
        machine Main::run(&mut self) {
            self.before = 1;
            self.flag = self.helper.check(1);
            self.after = 3;
        }
        "#,
    );
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = lower_to_target_operations(&source, TargetLoweringRequest::new(native))
            .expect("a Boolean call result remains available to its field store");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let mut checked_stores = 0;
        for function in &target.functions {
            for block in &function.graph.blocks {
                for (position, operation) in block.operations.iter().enumerate() {
                    let TargetUnitOperation::StructuralScalarFieldStore { source, .. } = operation
                    else {
                        continue;
                    };
                    match source {
                        TargetUnitScalarArgumentSource::Home(home)
                            if home.scalar_type == ScalarType::Boolean =>
                        {
                            assert!(
                                block.operations[..position].iter().any(|producer| matches!(
                                    producer,
                                    TargetUnitOperation::Call {
                                        result: target_operations::TargetCallResult::Scalar(result),
                                        ..
                                    } if result == home
                                )),
                                "the store must consume the preceding call's exact result home"
                            );
                        }
                        // Ordered receiver-call evaluation hands the result to
                        // its continuation block. Every edge into that block
                        // must bind the parameter to the exact result of a call
                        // in the edge's source block.
                        TargetUnitScalarArgumentSource::BlockParameter(parameter)
                            if parameter.scalar_type == ScalarType::Boolean =>
                        {
                            assert_eq!(parameter.block, block.block);
                            let incoming = incoming_successors(&function.graph.blocks, block.block);
                            assert!(!incoming.is_empty(), "the continuation has a predecessor");
                            for (predecessor, successor) in incoming {
                                let binding = successor
                                    .bindings
                                    .iter()
                                    .find(|binding| binding.parameter == parameter.value)
                                    .expect("each incoming edge binds the stored parameter");
                                assert!(
                                    predecessor.operations.iter().any(|producer| {
                                        is_boolean_call_result(producer, binding.argument)
                                    }),
                                    "the edge must carry its source block's exact call result"
                                );
                            }
                        }
                        _ => continue,
                    }
                    checked_stores += 1;
                }
            }
        }
        assert_eq!(checked_stores, 1);
    }
}

fn is_boolean_call_result(operation: &TargetUnitOperation, value: ValueId) -> bool {
    matches!(
        operation,
        TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Scalar(result),
            ..
        } if result.source_value == value && result.scalar_type == ScalarType::Boolean
    )
}

/// Scalar successors entering `target`. A structural case edge binds payload
/// fields, never a call result, so it cannot enter the store's block.
fn incoming_successors(
    blocks: &[target_operations::TargetControlBlock],
    target: BlockId,
) -> Vec<(
    &target_operations::TargetControlBlock,
    &target_operations::TargetControlSuccessor,
)> {
    let mut incoming = Vec::new();
    for block in blocks {
        match &block.terminator {
            target_operations::TargetControlTerminator::Jump { successor } => {
                incoming.push((block, successor));
            }
            target_operations::TargetControlTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                incoming.push((block, when_true));
                incoming.push((block, when_false));
            }
            target_operations::TargetControlTerminator::StructuralCase { cases, .. } => {
                assert!(cases.iter().all(|case| case.target != target));
            }
            _ => {}
        }
    }
    incoming.retain(|(_, successor)| successor.target == target);
    incoming
}

fn fixed_integer(bits: u16) -> IntegerType {
    IntegerType::new(IntegerSign::Signed, bits).expect("fixed integer")
}

fn scalar_parameter(value: u64, scalar_type: IntegerType) -> AbstractParameter {
    AbstractParameter {
        value: ValueId::new(value).expect("parameter value"),
        scalar_type: ScalarType::Integer(scalar_type),
    }
}

pub(super) fn attached_unit_scalar_call_plan() -> AbstractOperationPlan {
    let attached_machine = MachineId::new(1).expect("attached machine");
    let scalar_machine = MachineId::new(2).expect("scalar machine");
    let attachment = StructuralTypeId::new(1).expect("attachment");
    let integer_type = fixed_integer(32);
    let scalar_type = ScalarType::Integer(integer_type);
    let constant = ValueId::new(10).expect("constant");
    let first_result = ValueId::new(11).expect("first result");
    let second_result = ValueId::new(12).expect("second result");
    let callee_parameter = scalar_parameter(20, integer_type);
    let callee_result = ValueId::new(21).expect("callee result");

    AbstractOperationPlan {
        psi: identity(),
        entry: attached_machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: attached_machine,
                attachment: Some(attachment),
                entry: BlockId::new(1).expect("attached entry"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: BlockId::new(1).expect("attached entry"),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).expect("constant operation"),
                        result: constant,
                        scalar_type,
                        value: IntegerValue::Signed(17),
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(11).expect("first call"),
                        result: first_result,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![constant],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(12).expect("second call"),
                        result: second_result,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![first_result],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).expect("return edge"),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: scalar_machine,
                attachment: None,
                entry: BlockId::new(2).expect("callee entry"),
                parameters: vec![callee_parameter],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: callee_result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: BlockId::new(2).unwrap(),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::Return {
                    psi_edge: EdgeId::new(2).expect("callee return"),
                    result: callee_result,
                    value: callee_parameter.value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

#[test]
fn unit_float_literal_calls_retain_raw_bits_and_prior_call_results() {
    use semantic_vocabulary::IeeeFloatValue;
    for literal in [
        IeeeFloatValue::Binary32(0xffc1_2345),
        IeeeFloatValue::Binary64(0xfff8_1234_5678_9abc),
    ] {
        let scalar_type = ScalarType::IeeeFloat(literal.format());
        let mut source = attached_unit_scalar_call_plan();
        source.functions[0].operations[0] = AbstractOperation::IeeeFloatConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: ValueId::new(10).unwrap(),
            value: literal,
        };
        for operation in &mut source.functions[0].operations {
            if let AbstractOperation::Call {
                scalar_type: actual,
                ..
            } = operation
            {
                *actual = scalar_type;
            }
        }
        let callee = &mut source.functions[1];
        callee.parameters[0].scalar_type = scalar_type;
        let AbstractFunctionResult::Scalar(result) = &mut callee.result else {
            panic!("scalar")
        };
        result.scalar_type = scalar_type;
        let AbstractOperation::Return {
            scalar_type: returned,
            ..
        } = &mut callee.operations[0]
        else {
            panic!("return")
        };
        *returned = scalar_type;
        callee.block_entries = vec![AbstractBlockEntry {
            block: callee.entry,
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }];
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let lowered =
                lower_to_target_operations(&source, TargetLoweringRequest::new(native)).unwrap();
            let body = &lowered.functions[0].graph;
            let TargetUnitOperation::Call {
                scalar_arguments: arguments,
                ..
            } = &body.blocks[0].operations[1]
            else {
                panic!("first call")
            };
            assert_eq!(
                arguments[0].source,
                target_operations::TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                    defining_operation: OperationId::new(10).unwrap(),
                    source_value: ValueId::new(10).unwrap(),
                    value: literal,
                }
            );
            let TargetUnitOperation::Call {
                scalar_arguments: arguments,
                ..
            } = &body.blocks[0].operations[2]
            else {
                panic!("second call")
            };
            assert!(
                matches!(arguments[0].source, target_operations::TargetUnitScalarArgumentSource::Home(home)
                if home.source_value == ValueId::new(11).unwrap() && home.scalar_type == scalar_type)
            );
            let mut missing = source.clone();
            missing.functions[0].operations.remove(0);
            assert!(matches!(
                lower_to_target_operations(&missing, TargetLoweringRequest::new(native)),
                Err(LoweringError::UnknownValue(_))
            ));
        }
    }
}

#[test]
fn unit_scalar_calls_preserve_free_callers_and_serviceful_scalar_callees() {
    let mut unattached = attached_unit_scalar_call_plan();
    unattached.functions[0].attachment = None;
    let lowered = lower_to_target_operations(
        &unattached,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .expect("a free caller does not need a fabricated attachment");
    assert_eq!(lowered.functions[0].attachment, None);
    let body = &lowered.functions[0].graph;
    assert_eq!(
        body.blocks[0]
            .operations
            .iter()
            .filter(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .count(),
        2
    );

    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let mut serviceful = attached_unit_scalar_call_plan();
        let baseline =
            lower_to_target_operations(&serviceful, TargetLoweringRequest::new(native)).unwrap();
        for function in &mut serviceful.functions {
            function
                .published_service_ceiling
                .push(semantic_vocabulary::ServiceId::new(1).unwrap());
        }
        let lowered = lower_to_target_operations(&serviceful, TargetLoweringRequest::new(native))
            .expect("service reach is independent of scalar ABI layout");
        crate::validate_abstract_to_target_translation(&serviceful, native, &lowered).unwrap();
        assert!(lowered.functions[1].scalar_abi.is_some());
        assert_eq!(
            baseline.functions[1].scalar_abi,
            lowered.functions[1].scalar_abi
        );
    }
}

#[test]
fn unit_scalar_calls_reject_wrong_arity_type_and_unknown_values() {
    let mut wrong_arity = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { arguments, .. } = &mut wrong_arity.functions[0].operations[1]
    else {
        unreachable!()
    };
    arguments.clear();
    assert_eq!(
        lower_to_target_operations(
            &wrong_arity,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::CallArgumentCountMismatch {
            callee: MachineId::new(2).unwrap(),
            expected: 1,
            actual: 0,
        })
    );

    let mut wrong_type = attached_unit_scalar_call_plan();
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let AbstractOperation::IntegerConstant {
        scalar_type, value, ..
    } = &mut wrong_type.functions[0].operations[0]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(unsigned);
    *value = IntegerValue::Unsigned(17);
    assert_eq!(
        lower_to_target_operations(
            &wrong_type,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::CallArgumentTypeMismatch {
            callee: MachineId::new(2).unwrap(),
            argument: ValueId::new(10).unwrap(),
        })
    );

    let mut unknown = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { arguments, .. } = &mut unknown.functions[0].operations[1] else {
        unreachable!()
    };
    arguments[0] = ValueId::new(99).expect("unknown value");
    assert_eq!(
        lower_to_target_operations(
            &unknown,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnknownValue(ValueId::new(99).unwrap()))
    );
}

#[test]
fn unit_scalar_calls_reject_address_wide_structural_and_mismatched_result_types() {
    let mut address = attached_unit_scalar_call_plan();
    let address_type = IntegerType::address(64).expect("address carrier");
    let AbstractOperation::Call { scalar_type, .. } = &mut address.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(address_type);
    address.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Integer(address_type),
    });
    assert_eq!(
        lower_to_target_operations(
            &address,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            ValueId::new(11).unwrap()
        ))
    );

    let mut boolean = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { scalar_type, .. } = &mut boolean.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Boolean;
    boolean.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    assert_eq!(
        lower_to_target_operations(
            &boolean,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee: MachineId::new(2).unwrap(),
            result: ValueId::new(12).unwrap()
        })
    );

    let mut wide = attached_unit_scalar_call_plan();
    let wide_type = fixed_integer(128);
    let AbstractOperation::Call { scalar_type, .. } = &mut wide.functions[0].operations[1] else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(wide_type);
    wide.functions[1].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(21).unwrap(),
        scalar_type: ScalarType::Integer(wide_type),
    });
    assert_eq!(
        lower_to_target_operations(&wide, TargetLoweringRequest::new(NativeTarget::linux_x64())),
        Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            ValueId::new(11).unwrap()
        ))
    );

    let mut structural = attached_unit_scalar_call_plan();
    structural.functions[1]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    assert_eq!(
        lower_to_target_operations(
            &structural,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnitScalarCallTargetShapeUnsupported(
            MachineId::new(2).unwrap()
        ))
    );
}

#[test]
fn unit_scalar_calls_reject_declared_result_and_argument_carrier_drift() {
    let mut result_mismatch = attached_unit_scalar_call_plan();
    let AbstractOperation::Call { scalar_type, .. } =
        &mut result_mismatch.functions[0].operations[1]
    else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(fixed_integer(64));
    assert_eq!(
        lower_to_target_operations(
            &result_mismatch,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee: MachineId::new(2).unwrap(),
            result: ValueId::new(11).unwrap(),
        })
    );

    let mut address_argument = attached_unit_scalar_call_plan();
    let address_type = IntegerType::address(64).unwrap();
    address_argument.functions[1].parameters[0].scalar_type = ScalarType::Integer(address_type);
    assert_eq!(
        lower_to_target_operations(
            &address_argument,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        ),
        Err(LoweringError::UnitScalarCallTargetShapeUnsupported(
            MachineId::new(2).unwrap()
        ))
    );
}

/// A Unit call whose structural arguments all resolve against the parameter
/// roster passes a Boolean from a caller parameter, an immediate or a block
/// parameter. A computed Boolean home is refused as an unknown value; the
/// unified direct-call lowering keeps that route's rule rather than widening it.
#[test]
fn unit_roster_calls_pass_boolean_immediates_and_refuse_computed_booleans() {
    let unit_machine = MachineId::new(3).unwrap();
    let constant = ValueId::new(10).unwrap();
    let flag = ValueId::new(13).unwrap();
    let mut source = attached_unit_scalar_call_plan();
    source.functions[0].operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: constant,
            scalar_type: ScalarType::Integer(fixed_integer(32)),
            value: IntegerValue::Signed(17),
        },
        AbstractOperation::IntegerEqual {
            psi_operation: OperationId::new(13).unwrap(),
            result: flag,
            left: constant,
            right: constant,
        },
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(14).unwrap(),
            callee: unit_machine,
            arguments: vec![flag],
            structural_arguments: Vec::new(),
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            result: abstract_operations::AbstractStructuralCallResult::Unit,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(AbstractFunction {
        machine: unit_machine,
        attachment: None,
        entry: BlockId::new(3).unwrap(),
        parameters: vec![AbstractParameter {
            value: ValueId::new(30).unwrap(),
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: BlockId::new(3).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(3).unwrap(),
            cleanup_actions: Vec::new(),
        }],
    });
    let native = NativeTarget::linux_x64();
    assert_eq!(
        lower_to_target_operations(&source, TargetLoweringRequest::new(native)),
        Err(LoweringError::UnknownValue(flag))
    );

    source.functions[0].operations[1] = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(13).unwrap(),
        result: flag,
        value: true,
    };
    let lowered = lower_to_target_operations(&source, TargetLoweringRequest::new(native)).unwrap();
    crate::validate_abstract_to_target_translation(&source, native, &lowered).unwrap();
    let TargetUnitOperation::Call {
        scalar_arguments, ..
    } = &lowered.functions[0].graph.blocks[0].operations[2]
    else {
        panic!("unit call")
    };
    assert_eq!(
        scalar_arguments[0].source,
        TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: OperationId::new(13).unwrap(),
            source_value: flag,
            value: true,
        }
    );
}
