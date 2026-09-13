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
fn scalar_graph_lowers_shared_view_call_transport() {
    let source = fixture();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&source, target).unwrap();
        let caller = &lowered.functions[0];
        let placement = caller.graph.parameters[0].placement.clone();
        for operation in &caller.graph.blocks[0].operations[..2] {
            let target_operations::TargetUnitOperation::StructuralScalarCall { arguments, .. } =
                operation
            else {
                panic!("shared-view call");
            };
            assert_eq!(
                arguments[0].source,
                target_operations::TargetStructuralArgumentSource::Placement(placement.clone())
            );
        }
    }
}

#[test]
fn scalar_graph_lowers_literal_descriptor_calls() {
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
        let lowered = lower_to_target_operations(&source, target).unwrap();
        let operations = &lowered.functions[0].graph.blocks[0].operations;
        assert!(matches!(
            operations[0],
            target_operations::TargetUnitOperation::EstablishByteSequenceLiteral { .. }
        ));
        for operation in &operations[1..3] {
            let target_operations::TargetUnitOperation::StructuralScalarCall { arguments, .. } =
                operation
            else {
                panic!("literal view call");
            };
            assert_eq!(
                arguments[0].source,
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: OperationId::new(10).unwrap(),
                }
            );
        }
    }
    let producer = source.functions[0].operations.remove(0);
    source.functions[0].operations.insert(2, producer);
    assert!(lower_to_target_operations(&source, NativeTarget::macos_arm64()).is_err());
}
