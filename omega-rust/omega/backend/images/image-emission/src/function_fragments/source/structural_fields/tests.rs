use super::{indexed_store_retained, replacement_retained, retained};
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target_operations::{
    TargetControlBlock, TargetControlGraph, TargetControlTerminator, TargetFunction,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment,
};

#[test]
fn projected_primitive_publication_retains_path_and_requires_native_replay() {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let (mut function, mut target) = field_fixture(scalar);
    let parameter = &function.structural_parameters[0];
    let identity = OperationId::new(1).unwrap();
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: scalar,
    };
    let path = vec![Segment::FixedIndex(1)];
    function.operations[0] = AbstractOperation::PrimitiveScalarRead {
        psi_operation: identity,
        result,
        source: parameter.place,
        path: path.clone(),
    };
    target.graph.blocks[0].operations[0] = TargetUnitOperation::PrimitiveScalarRead {
        psi_operation: identity,
        result,
        source: parameter.place,
        path,
    };
    let element = StructuralTypeId::new(2).unwrap();
    target.graph.structural_types = vec![
        StructuralTypeDeclaration {
            id: parameter.structural_type,
            identity: "test::Bytes".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 2 },
        },
        StructuralTypeDeclaration {
            id: element,
            identity: "test::Byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        },
    ]
    .into();
    assert!(retained(&function, &function.operations[0], &target));
    assert!(super::super::requires_graph_storage_replay(
        &function.operations
    ));
    for replacement in [0, 2, u64::MAX] {
        let mut changed = target.clone();
        let TargetUnitOperation::PrimitiveScalarRead { path, .. } =
            &mut changed.graph.blocks[0].operations[0]
        else {
            panic!("read");
        };
        *path = vec![Segment::FixedIndex(replacement)];
        assert!(!retained(&function, &function.operations[0], &changed));
    }
    let mut invalid = function.clone();
    let AbstractOperation::PrimitiveScalarRead { path, .. } = &mut invalid.operations[0] else {
        panic!("read");
    };
    *path = vec![Segment::FixedIndex(2)];
    let TargetUnitOperation::PrimitiveScalarRead { path, .. } =
        &mut target.graph.blocks[0].operations[0]
    else {
        panic!("read");
    };
    *path = vec![Segment::FixedIndex(2)];
    assert!(
        !retained(&invalid, &invalid.operations[0], &target),
        "matching invalid indices do not establish a valid primitive subject"
    );
}

fn field_fixture(scalar_type: ScalarType) -> (AbstractFunction, TargetFunction) {
    // These fixtures isolate source membership, not object/load replay certificates.
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let psi_operation = OperationId::new(1).unwrap();
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type,
    };
    let field = StructuralFieldId::new(1).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: true,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let operation = match scalar_type {
        ScalarType::Boolean => AbstractOperation::BooleanStructuralField {
            psi_operation,
            result: result.value,
            path: Vec::new(),
            source: parameter.place,
            field,
        },
        ScalarType::Integer(_) => AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            path: Vec::new(),
            source: parameter.place,
            field,
        },
        ScalarType::IeeeFloat(_) => panic!("fixture covers integer and Boolean field reads"),
    };
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let target_parameter = TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: vec![],
        shape,
        placement: call_plan.parameters[0].clone(),
    };
    let target_operation = TargetUnitOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        source: StructuralArgument {
            place: parameter.place,
            access: parameter.access,
            path: vec![],
        },
        field,
    };
    let function = AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: vec![],
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Unit,
        entry_claims: vec![],
        published_service_ceiling: vec![],
        block_entries: vec![],
        operations: vec![
            operation,
            AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: vec![],
            },
        ],
    };
    let target = TargetFunction {
        machine,
        attachment: None,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![psi_operation],
            edges: vec![edge],
        },
        graph: TargetControlGraph {
            structural_types: vec![].into(),
            call_plan,
            scalar_parameters: vec![],
            parameters: vec![target_parameter],
            dynamic_parameters: vec![],
            entry: block,
            blocks: vec![TargetControlBlock {
                block,
                parameters: vec![],
                structural_parameters: vec![],
                operations: vec![target_operation],
                terminator: TargetControlTerminator::Return {
                    psi_edge: edge,
                    cleanup_actions: vec![],
                },
            }],
        },
    };
    (function, target)
}

fn field_types() -> [ScalarType; 3] {
    [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
    ]
}

#[test]
fn byte_length_membership_preserves_metadata_kind_and_exact_borrowed_subject() {
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (mut function, mut target) = field_fixture(scalar);
        function.structural_parameters[0].access = access;
        target.graph.parameters[0].access = access;
        let psi_operation = OperationId::new(1).unwrap();
        let result = AbstractResult {
            value: ValueId::new(1).unwrap(),
            scalar_type: scalar,
        };
        let source = StructuralArgument {
            place: function.structural_parameters[0].place,
            access,
            path: vec![terminal_psi::StructuralPathSegment::Field("inner".into())],
        };
        let field = StructuralFieldId::new(1).unwrap();
        function.operations[0] = AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            source: source.place,
            path: source.path.clone(),
            field,
        };
        target.graph.blocks[0].operations[0] =
            TargetUnitOperation::StructuralByteSequenceFieldLength {
                psi_operation,
                result,
                source: source.clone(),
                field,
            };
        assert!(retained(&function, &function.operations[0], &target));
        assert!(super::super::requires_graph_storage_replay(
            &function.operations
        ));
        for mutation in 0..9 {
            let mut changed = target.clone();
            let operation = &mut changed.graph.blocks[0].operations[0];
            let TargetUnitOperation::StructuralByteSequenceFieldLength {
                psi_operation,
                result,
                source,
                field,
            } = operation
            else {
                panic!("metadata")
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => result.value = ValueId::new(2).unwrap(),
                2 => result.scalar_type = ScalarType::Boolean,
                3 => source.place = PlaceId::new(2).unwrap(),
                4 => source.path.clear(),
                5 => *field = StructuralFieldId::new(2).unwrap(),
                6 => source.access = StructuralAccess::Owned,
                7 => {
                    *operation = TargetUnitOperation::StructuralScalarFieldRead {
                        psi_operation: *psi_operation,
                        result: *result,
                        source: source.clone(),
                        field: *field,
                    }
                }
                _ => {
                    let duplicate = operation.clone();
                    changed.graph.blocks[0].operations.push(duplicate);
                }
            }
            assert!(
                !retained(&function, &function.operations[0], &changed),
                "mutation {mutation}"
            );
        }
        // Even matching metadata operands cannot supply owned root admission.
        function.structural_parameters[0].access = StructuralAccess::Owned;
        target.graph.parameters[0].access = StructuralAccess::Owned;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

#[test]
fn byte_replacement_membership_retains_every_operand_and_writable_access() {
    use semantic_vocabulary::ObligationId;
    use target_operations::TargetUnitScalarArgumentSource;
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (mut function, mut target) = field_fixture(scalar);
        function.structural_parameters[0].access = access;
        target.graph.parameters[0].access = access;
        let psi_operation = OperationId::new(1).unwrap();
        let source = PlaceId::new(2).unwrap();
        let length = ValueId::new(1).unwrap();
        let obligation = ObligationId::new(1).unwrap();
        let destination = StructuralArgument {
            place: function.structural_parameters[0].place,
            access,
            path: vec![StructuralPathSegment::Field("inner".into())],
        };
        let field = StructuralFieldId::new(1).unwrap();
        function.operations[0] = AbstractOperation::StructuralByteSequenceFieldStore {
            psi_operation,
            destination: destination.place,
            path: destination.path.clone(),
            field,
            source,
            length,
            obligation,
        };
        target.graph.blocks[0].operations[0] =
            TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation,
                destination,
                field,
                source,
                length: TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value: length,
                    scalar_type: scalar,
                },
                obligation,
            };
        assert!(replacement_retained(
            &function,
            &function.operations[0],
            &target
        ));
        assert!(super::super::requires_graph_storage_replay(
            &function.operations
        ));
        for mutation in 0..10 {
            let mut changed = target.clone();
            let operation = &mut changed.graph.blocks[0].operations[0];
            let TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation,
                destination,
                field,
                source,
                length,
                obligation,
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => destination.place = PlaceId::new(3).unwrap(),
                2 => destination.path.clear(),
                3 => destination.access = StructuralAccess::SharedBorrow,
                4 => *field = StructuralFieldId::new(2).unwrap(),
                5 => *source = PlaceId::new(3).unwrap(),
                6 => *obligation = ObligationId::new(2).unwrap(),
                7 => {
                    *length = TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(2).unwrap(),
                        scalar_type: scalar,
                    }
                }
                8 => {
                    *length = TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    }
                }
                _ => {
                    let duplicate = operation.clone();
                    changed.graph.blocks[0].operations.push(duplicate);
                }
            }
            assert!(
                !replacement_retained(&function, &function.operations[0], &changed),
                "mutation {mutation}"
            );
        }
        // Equal producer/target operands do not turn a shared loan into write authority.
        function.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        target.graph.parameters[0].access = StructuralAccess::SharedBorrow;
        let TargetUnitOperation::StructuralByteSequenceFieldStore { destination, .. } =
            &mut target.graph.blocks[0].operations[0]
        else {
            unreachable!()
        };
        destination.access = StructuralAccess::SharedBorrow;
        assert!(!replacement_retained(
            &function,
            &function.operations[0],
            &target
        ));
    }
}

#[test]
fn indexed_byte_publication_retains_every_operand_and_writable_access() {
    use semantic_vocabulary::ObligationId;
    use target_operations::TargetUnitScalarArgumentSource;
    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let byte_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let scalar_input = |source_value, scalar_type| TargetUnitScalarArgumentSource::Parameter {
        parameter_index: 0,
        source_value,
        scalar_type,
    };
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (mut function, mut target) = field_fixture(count_type);
        function.structural_parameters[0].access = access;
        target.graph.parameters[0].access = access;
        let psi_operation = OperationId::new(1).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let obligation = ObligationId::new(1).unwrap();
        let destination = StructuralArgument {
            place: function.structural_parameters[0].place,
            access,
            path: vec![StructuralPathSegment::Field("inner".into())],
        };
        function.operations[0] = AbstractOperation::StructuralByteSequenceFieldByteStore {
            psi_operation,
            destination: destination.place,
            path: destination.path.clone(),
            field,
            index,
            value,
            length,
            obligation,
        };
        target.graph.blocks[0].operations[0] =
            TargetUnitOperation::StructuralByteSequenceFieldByteStore {
                psi_operation,
                destination,
                field,
                index: scalar_input(index, count_type),
                value: scalar_input(value, byte_type),
                length,
                obligation,
            };
        assert!(indexed_store_retained(
            &function,
            &function.operations[0],
            &target
        ));
        assert!(super::super::requires_graph_storage_replay(
            &function.operations
        ));
        assert!(!super::indexed_store_footprint_retained(
            &function.operations[0],
            &[]
        ));
        for corruption in 0..12 {
            let mut changed = target.clone();
            let operation = &mut changed.graph.blocks[0].operations[0];
            let TargetUnitOperation::StructuralByteSequenceFieldByteStore {
                psi_operation,
                destination,
                field,
                index,
                value,
                length,
                obligation,
            } = operation
            else {
                unreachable!();
            };
            match corruption {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => destination.place = PlaceId::new(2).unwrap(),
                2 => destination.path.clear(),
                3 => destination.access = StructuralAccess::SharedBorrow,
                4 => *field = StructuralFieldId::new(2).unwrap(),
                5 => *index = scalar_input(ValueId::new(4).unwrap(), count_type),
                6 => *value = scalar_input(ValueId::new(4).unwrap(), byte_type),
                7 => *length = ValueId::new(4).unwrap(),
                8 => *obligation = ObligationId::new(2).unwrap(),
                9 => *index = scalar_input(index.source_value(), byte_type),
                10 => *value = scalar_input(value.source_value(), count_type),
                _ => {
                    let duplicate = operation.clone();
                    changed.graph.blocks[0].operations.push(duplicate);
                }
            }
            assert!(
                !indexed_store_retained(&function, &function.operations[0], &changed),
                "corruption {corruption}"
            );
        }
        // Matching source and target pointer authority must still be writable.
        for access in [StructuralAccess::Owned, StructuralAccess::SharedBorrow] {
            function.structural_parameters[0].access = access;
            target.graph.parameters[0].access = access;
            let TargetUnitOperation::StructuralByteSequenceFieldByteStore { destination, .. } =
                &mut target.graph.blocks[0].operations[0]
            else {
                unreachable!();
            };
            destination.access = access;
            assert!(!indexed_store_retained(
                &function,
                &function.operations[0],
                &target
            ));
        }
    }
}

#[test]
fn nested_field_membership_reconstructs_parent_local_carrier_identity() {
    for scalar in field_types() {
        let (mut function, mut target) = field_fixture(scalar);
        let carrier_field = StructuralFieldId::new(1).unwrap();
        let child = StructuralTypeId::new(2).unwrap();
        match &mut function.operations[0] {
            AbstractOperation::BooleanStructuralField { path, .. }
            | AbstractOperation::IntegerStructuralField { path, .. } => path.push(
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(carrier_field),
            ),
            _ => unreachable!(),
        }
        target.graph.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
            id: function.structural_parameters[0].structural_type,
            identity: "Root".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: carrier_field,
                    identity: "child".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(child),
                }],
            },
        }]
        .into();
        target
            .graph
            .structural_types
            .make_mut()
            .push(terminal_psi::StructuralTypeDeclaration {
                id: child,
                identity: "Child".into(),
                shape: terminal_psi::StructuralTypeShape::Record {
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        id: carrier_field,
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                    }],
                },
            });
        let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
            &mut target.graph.blocks[0].operations[0]
        else {
            unreachable!()
        };
        source
            .path
            .push(StructuralPathSegment::Field("child".into()));
        assert!(retained(&function, &function.operations[0], &target));
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field("other".into())],
        ] {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
                &mut changed.graph.blocks[0].operations[0]
            else {
                unreachable!()
            };
            source.path = path;
            assert!(!retained(&function, &function.operations[0], &changed));
        }
        let terminal_psi::StructuralTypeShape::Record { fields } =
            &mut target.graph.structural_types.make_mut()[0].shape
        else {
            unreachable!()
        };
        fields[0].relevance = terminal_psi::BindingRelevance::Erased;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

fn owned_field_fixture(
    scalar_type: ScalarType,
    multiplicity: StructuralMultiplicity,
) -> (AbstractFunction, TargetFunction) {
    let (mut function, mut target) = field_fixture(scalar_type);
    let parameter = &mut function.structural_parameters[0];
    parameter.is_self = false;
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = multiplicity;
    target.graph.parameters[0].access = StructuralAccess::Owned;
    target.graph.parameters[0].multiplicity = multiplicity;
    let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
        &mut target.graph.blocks[0].operations[0]
    else {
        unreachable!()
    };
    source.access = StructuralAccess::Owned;
    (function, target)
}

#[test]
fn owned_parameter_field_membership_keeps_exact_value_access_and_graph_replay() {
    for scalar_type in field_types() {
        for multiplicity in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Unrestricted,
        ] {
            let (function, target) = owned_field_fixture(scalar_type, multiplicity);
            assert!(retained(&function, &function.operations[0], &target));
            assert!(super::super::requires_graph_storage_replay(
                &function.operations
            ));
            for mutation in 0..6 {
                let mut changed = target.clone();
                let TargetUnitOperation::StructuralScalarFieldRead {
                    result,
                    source,
                    field,
                    ..
                } = &mut changed.graph.blocks[0].operations[0]
                else {
                    unreachable!()
                };
                match mutation {
                    0 => source.access = StructuralAccess::SharedBorrow,
                    1 => changed.graph.parameters[0].access = StructuralAccess::SharedBorrow,
                    2 => source.place = PlaceId::new(2).unwrap(),
                    3 => source
                        .path
                        .push(StructuralPathSegment::Field("nested".into())),
                    4 => result.value = ValueId::new(2).unwrap(),
                    _ => *field = StructuralFieldId::new(2).unwrap(),
                }
                assert!(!retained(&function, &function.operations[0], &changed));
            }
        }
        // Even matching declarations cannot grant access to a linear or write-only root.
        let (function, target) = owned_field_fixture(scalar_type, StructuralMultiplicity::Linear);
        assert!(!retained(&function, &function.operations[0], &target));
        let (mut function, mut target) =
            owned_field_fixture(scalar_type, StructuralMultiplicity::Affine);
        function.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        target.graph.parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
            &mut target.graph.blocks[0].operations[0]
        else {
            unreachable!()
        };
        source.access = StructuralAccess::WriteOnlyBorrow;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

#[test]
fn observed_owned_parameter_discard_keeps_exact_scalar_exit_accounting() {
    use super::super::control_flow;
    use target_operations::{
        TargetBooleanExpression, TargetScalarExpression, TargetUnitScalarHomeRequirement,
    };
    use terminal_psi::TerminalAffineCleanupAction;

    let (mut function, mut target) =
        owned_field_fixture(ScalarType::Boolean, StructuralMultiplicity::Affine);
    let edge = EdgeId::new(1).unwrap();
    let value = ValueId::new(1).unwrap();
    let actions = vec![TerminalAffineCleanupAction::DiscardRoot(
        function.structural_parameters[0].place,
    )];
    function.operations[1] = AbstractOperation::Return {
        psi_edge: edge,
        result: value,
        value,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: actions.clone(),
    };
    target.graph.blocks[0].terminator = TargetControlTerminator::ReturnScalar {
        psi_edge: edge,
        source_value: value,
        expression: TargetScalarExpression::Boolean(TargetBooleanExpression::ScalarHome(
            TargetUnitScalarHomeRequirement {
                defining_operation: OperationId::new(1).unwrap(),
                source_value: value,
                scalar_type: ScalarType::Boolean,
                shape: ValueShape::integer(1, 1),
            },
        )),
        cleanup_actions: actions,
    };
    assert!(retained(&function, &function.operations[0], &target));
    assert!(control_flow::retained(&function.operations[1], &target));
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetControlTerminator::ReturnScalar {
            psi_edge,
            source_value,
            cleanup_actions,
            ..
        } = &mut changed.graph.blocks[0].terminator
        else {
            unreachable!()
        };
        match mutation {
            0 => cleanup_actions.clear(),
            1 => {
                cleanup_actions[0] =
                    TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(2).unwrap())
            }
            2 => *psi_edge = EdgeId::new(2).unwrap(),
            3 => *source_value = ValueId::new(2).unwrap(),
            _ => changed.graph.blocks.push(changed.graph.blocks[0].clone()),
        }
        assert!(!control_flow::retained(&function.operations[1], &changed));
    }
}

#[test]
fn structural_field_membership_requires_exact_source_result_type_access_path_and_field() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        assert!(retained(&function, operation, &target));
        for mutation in 0..7 {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                field,
            } = &mut changed.graph.blocks[0].operations[0]
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => result.value = ValueId::new(2).unwrap(),
                2 => {
                    result.scalar_type = if scalar_type == ScalarType::Boolean {
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                    } else {
                        ScalarType::Boolean
                    }
                }
                3 => source.place = PlaceId::new(2).unwrap(),
                4 => source.access = StructuralAccess::MutableBorrow,
                5 => source
                    .path
                    .push(StructuralPathSegment::Field("nested".to_owned())),
                _ => *field = StructuralFieldId::new(2).unwrap(),
            }
            assert!(
                !retained(&function, operation, &changed),
                "{scalar_type:?}: mutation {mutation}"
            );
        }
    }
}

#[test]
fn structural_field_membership_requires_one_matching_read_across_all_blocks() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        let mut changed = target.clone();
        changed.graph.blocks[0].operations.clear();
        assert!(!retained(&function, operation, &changed));
        let mut second_block = target.graph.blocks[0].clone();
        second_block.block = BlockId::new(2).unwrap();
        changed.graph.blocks.push(second_block);
        assert!(retained(&function, operation, &changed));
        changed.graph.blocks[0].operations = target.graph.blocks[0].operations.clone();
        assert!(!retained(&function, operation, &changed));
        let TargetUnitOperation::StructuralScalarFieldRead { field, .. } =
            &mut changed.graph.blocks[1].operations[0]
        else {
            unreachable!()
        };
        *field = StructuralFieldId::new(2).unwrap();
        assert!(!retained(&function, operation, &changed));
        let TargetUnitOperation::StructuralScalarFieldRead { psi_operation, .. } =
            &mut changed.graph.blocks[1].operations[0]
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(2).unwrap();
        assert!(retained(&function, operation, &changed));
    }
}

#[test]
fn structural_field_membership_requires_unique_exact_parameter_declaration() {
    for scalar_type in field_types() {
        let (function, target) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        let mut missing = function.clone();
        missing.structural_parameters.clear();
        assert!(!retained(&missing, operation, &target));
        let mut duplicate = function.clone();
        duplicate
            .structural_parameters
            .push(function.structural_parameters[0].clone());
        assert!(!retained(&duplicate, operation, &target));
        duplicate.structural_parameters[1].access = StructuralAccess::MutableBorrow;
        assert!(!retained(&duplicate, operation, &target));
        if let AbstractOperation::IntegerStructuralField { .. } = operation {
            for mutation in 0..4 {
                let mut changed = function.clone();
                let source = &mut changed.structural_parameters[0];
                match mutation {
                    0 => source.structural_type = StructuralTypeId::new(2).unwrap(),
                    1 => source.access = StructuralAccess::Owned,
                    2 => source.multiplicity = StructuralMultiplicity::Affine,
                    _ => source.position = 1,
                }
                assert!(
                    !retained(&changed, operation, &target),
                    "mutation {mutation}"
                );
            }
        }
        assert!(!retained(&function, &function.operations[1], &target));
    }
}

#[test]
fn structural_field_reads_require_graph_storage_replay() {
    let unrelated = AbstractOperation::BooleanConstant {
        psi_operation: OperationId::new(2).unwrap(),
        result: ValueId::new(2).unwrap(),
        value: true,
    };
    assert!(!super::super::requires_graph_storage_replay(
        std::slice::from_ref(&unrelated)
    ));
    for scalar_type in field_types() {
        let (function, _) = field_fixture(scalar_type);
        let operation = &function.operations[0];
        assert!(super::super::requires_graph_storage_replay(
            std::slice::from_ref(operation)
        ));
        assert!(super::super::requires_graph_storage_replay(&[
            unrelated.clone(),
            operation.clone()
        ]));
    }
}
