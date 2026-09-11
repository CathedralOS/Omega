//! Ordinary repeated shared-view calls retain scalar and referent identities.
use super::*;
mod established_views;

fn fixture() -> AbstractOperationPlan {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let structural_type = StructuralTypeId::new(1).unwrap();
    let parameter = |raw| StructuralParameterDeclaration {
        place: PlaceId::new(raw).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let function = |raw| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: vec![AbstractParameter {
            value: ValueId::new(raw + 10).unwrap(),
            scalar_type: integer,
        }],
        structural_parameters: vec![parameter(raw)],
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: ValueId::new(raw + 100).unwrap(),
            scalar_type: integer,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: Vec::new(),
    };
    let mut caller = function(1);
    let mut callee = function(2);
    for raw in [3, 4] {
        caller
            .operations
            .push(AbstractOperation::CallStructuralScalar {
                psi_operation: OperationId::new(raw).unwrap(),
                result: AbstractResult {
                    value: ValueId::new(raw).unwrap(),
                    scalar_type: integer,
                },
                callee: callee.machine,
                arguments: vec![caller.parameters[0].value],
                structural_arguments: vec![StructuralArgument {
                    place: caller.structural_parameters[0].place,
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            });
    }
    caller.operations.push(AbstractOperation::Return {
        psi_edge: EdgeId::new(1).unwrap(),
        result: ValueId::new(101).unwrap(),
        value: ValueId::new(4).unwrap(),
        scalar_type: integer,
        cleanup_actions: Vec::new(),
    });
    callee.operations.push(AbstractOperation::Return {
        psi_edge: EdgeId::new(2).unwrap(),
        result: ValueId::new(102).unwrap(),
        value: callee.parameters[0].value,
        scalar_type: integer,
        cleanup_actions: Vec::new(),
    });
    AbstractOperationPlan {
        psi: identity(),
        entry: caller.machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    }
}

#[test]
fn scalar_graph_rejects_unimplemented_shared_view_call_transport() {
    let source = fixture();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        assert!(lower_to_target_operations(&source, target).is_err());
    }
}

fn unit_fixture() -> AbstractOperationPlan {
    let mut source = fixture();
    let caller = &mut source.functions[0];
    caller.result = AbstractFunctionResult::Unit;
    caller.parameters.push(AbstractParameter {
        value: ValueId::new(12).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let mut helper = caller.clone();
    helper.machine = MachineId::new(3).unwrap();
    helper.entry = BlockId::new(3).unwrap();
    helper.block_entries[0].block = helper.entry;
    helper.structural_parameters[0].place = PlaceId::new(3).unwrap();
    helper.operations.truncate(1);
    if let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut helper.operations[0]
    {
        structural_arguments[0].place = PlaceId::new(3).unwrap();
    }
    helper.operations.push(AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(3).unwrap(),
        cleanup_actions: Vec::new(),
    });
    let call = |raw, boolean| AbstractOperation::CallUnit {
        psi_operation: OperationId::new(raw).unwrap(),
        callee: helper.machine,
        arguments: vec![ValueId::new(11).unwrap(), ValueId::new(boolean).unwrap()],
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    caller.operations = vec![
        call(10, 12),
        AbstractOperation::BooleanConstant {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(13).unwrap(),
            value: true,
        },
        call(12, 13),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(helper);
    source
}

#[test]
fn repeated_unit_view_calls_preserve_boolean_sources_and_have_no_result() {
    let source = unit_fixture();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&source, target).unwrap();
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("genuine Unit body");
        };
        assert!(body.call_plan.result.is_none());
        for (position, expected_operation) in [(0, 10), (2, 12)] {
            let target_operations::TargetUnitOperation::Call {
                psi_operation,
                call_plan,
                scalar_arguments,
                arguments,
                ..
            } = &body.operations[position]
            else {
                panic!("ordinary Unit call");
            };
            assert_eq!(psi_operation.get(), expected_operation);
            assert!(call_plan.result.is_none());
            assert_eq!(
                arguments[0].source,
                body.parameters[0].placement.clone().into()
            );
            assert_eq!(arguments[0].destination, call_plan.parameters[2]);
            assert!(matches!(
                arguments[0].destination.locations.as_slice(),
                [ValueLocation::Indirect {
                    copy_stack_byte_offset: None,
                    ..
                }]
            ));
            match &scalar_arguments[1].source {
                target_operations::TargetUnitScalarArgumentSource::Parameter {
                    source_value,
                    scalar_type: ScalarType::Boolean,
                    ..
                } => assert_eq!(source_value.get(), 12),
                target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                    source_value,
                    value: true,
                    ..
                } => assert_eq!(source_value.get(), 13),
                other => panic!("exact Boolean source: {other:?}"),
            }
        }
        let TargetOperation::UnitBody(helper) = &lowered.functions[2].operation else {
            panic!("Unit helper");
        };
        assert!(helper.call_plan.result.is_none());
        assert!(matches!(
            helper.operations[0],
            target_operations::TargetUnitOperation::StructuralScalarCall { .. }
        ));
        assert!(matches!(
            helper.operations[1],
            target_operations::TargetUnitOperation::Return { .. }
        ));
    }
}

#[test]
fn unit_view_calls_reject_substituted_referents_access_and_boolean_actuals() {
    for mutation in 0..4 {
        let mut source = unit_fixture();
        let AbstractOperation::CallUnit {
            arguments,
            structural_arguments,
            ..
        } = &mut source.functions[0].operations[0]
        else {
            unreachable!();
        };
        match mutation {
            0 => structural_arguments[0].place = PlaceId::new(99).unwrap(),
            1 => structural_arguments[0].access = StructuralAccess::MutableBorrow,
            2 => arguments[1] = ValueId::new(11).unwrap(),
            _ => arguments.pop().map(|_| ()).unwrap(),
        }
        assert!(lower_to_target_operations(&source, NativeTarget::macos_arm64()).is_err());
    }
}

#[test]
fn scalar_graph_rejects_unimplemented_literal_descriptor_calls() {
    let mut source = fixture();
    let literal_type = source.structural_types[0].clone();
    let caller = &mut source.functions[0];
    caller.structural_parameters.clear();
    caller.operations.insert(
        0,
        AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation: OperationId::new(10).unwrap(),
            place: terminal_psi::StructuralPlaceDeclaration {
                id: PlaceId::new(1).unwrap(),
                kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type: literal_type.id,
                },
            },
            structural_type: literal_type,
            bytes: vec![0, 0x80, 0xff],
        },
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        assert!(lower_to_target_operations(&source, target).is_err());
    }
    let producer = source.functions[0].operations.remove(0);
    source.functions[0].operations.insert(2, producer);
    assert!(lower_to_target_operations(&source, NativeTarget::macos_arm64()).is_err());
}
