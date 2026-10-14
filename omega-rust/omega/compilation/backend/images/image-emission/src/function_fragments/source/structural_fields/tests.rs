use super::{
    indexed_store_retained, replacement_footprints_retained, replacement_retained, retained,
};
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
    AbstractStructuralCallResult,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target_operations::{
    TargetCallResult, TargetControlBlock, TargetControlGraph, TargetControlTerminator,
    TargetFunction, TargetReferenceResult, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// A `Structural` call result with empty transfer and evidence custody —
/// the shape every fixture's authored call declares.
fn structural_call_result(
    result: terminal_psi::StructuralOperationResult,
) -> AbstractStructuralCallResult {
    AbstractStructuralCallResult::Structural {
        result,
        selected_evidence: Vec::new(),
    }
}

#[test]
fn borrowed_block_reads_retain_declarations_independently_of_native_carrier() {
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let (mut function, mut target) = field_fixture(scalar);
    let block = BlockId::new(2).unwrap();
    let parameter = function.structural_parameters.remove(0);
    let place = parameter.place;
    let identity = parameter.structural_type;
    function
        .block_entries
        .push(abstract_operations::AbstractBlockEntry {
            block,
            structural_parameters: vec![parameter.clone()],
            parameters: Vec::new(),
            operation_offset: 0,
        });
    target.graph.parameters.clear();
    target.graph.blocks[0].block = block;
    target.graph.blocks[0].structural_parameters = vec![parameter];
    target.graph.structural_types = vec![StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: identity,
        identity: terminal_psi::TerminalIdentityIndex(1),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
            capacity: 48,
            access: Some(StructuralAccess::SharedBorrow),
        }),
    }]
    .into();
    let read = |function: &AbstractFunction, target: &TargetFunction| {
        super::read_access(function, target, place, super::AccessPurpose::Field)
    };
    assert_eq!(
        read(&function, &target),
        Some((StructuralAccess::SharedBorrow, identity))
    );
    let mut changed = target.clone();
    changed.graph.blocks[0].structural_parameters[0].position = 1;
    assert_eq!(
        read(&function, &changed),
        None,
        "the target must retain the exact declaration"
    );
    for (access, multiplicity) in [
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralMultiplicity::Unrestricted,
        ),
        (
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Linear,
        ),
    ] {
        let mut changed_function = function.clone();
        let mut changed_target = target.clone();
        let parameter = &mut changed_function.block_entries[0].structural_parameters[0];
        parameter.access = access;
        parameter.multiplicity = multiplicity;
        changed_target.graph.blocks[0].structural_parameters[0] = parameter.clone();
        assert_eq!(read(&changed_function, &changed_target), None);
    }
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
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    assert!(
        !matches!(scalar_type, ScalarType::IeeeFloat(_)),
        "fixture covers integer and Boolean field reads"
    );
    let operation = AbstractOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        path: Vec::new(),
        source: parameter.place,
        field,
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
        case: None,
        field,
    };
    let function = AbstractFunction {
        machine,
        attachment: None,
        nominal_cleanup_subject: None,
        entry: block,
        parameters: vec![],
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Unit,
        entry_claims: vec![],
        block_entry_claims: vec![],
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
        nominal_cleanup_subject: None,
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

fn scalar_field_read_source_mut(
    graph: &mut TargetControlGraph,
    operation: usize,
) -> &mut StructuralArgument {
    let TargetUnitOperation::StructuralScalarFieldRead { source, .. } =
        &mut graph.blocks[0].operations[operation]
    else {
        unreachable!()
    };
    source
}

fn structural_call_reference_results_mut(
    graph: &mut TargetControlGraph,
    operation: usize,
) -> &mut Vec<TargetReferenceResult> {
    let TargetUnitOperation::Call {
        result: TargetCallResult::Structural {
            reference_results, ..
        },
        ..
    } = &mut graph.blocks[0].operations[operation]
    else {
        unreachable!()
    };
    reference_results
}

fn structural_call_result_home_mut(
    graph: &mut TargetControlGraph,
    operation: usize,
) -> &mut Option<TargetStructuralHomeRequirement> {
    let TargetUnitOperation::Call {
        result: TargetCallResult::Structural { result_home, .. },
        ..
    } = &mut graph.blocks[0].operations[operation]
    else {
        unreachable!()
    };
    result_home
}

fn byte_store_destination_mut(
    graph: &mut TargetControlGraph,
    operation: usize,
) -> Option<&mut StructuralArgument> {
    let TargetUnitOperation::StructuralByteSequenceFieldStore { destination, .. } =
        &mut graph.blocks[0].operations[operation]
    else {
        return None;
    };
    Some(destination)
}

fn record_fields_mut(
    declaration: &mut StructuralTypeDeclaration,
) -> Option<&mut Vec<StructuralFieldDeclaration>> {
    let StructuralTypeShape::Record { fields, .. } = &mut declaration.shape else {
        return None;
    };
    Some(fields)
}

fn reference_access_mut(
    declaration: &mut StructuralTypeDeclaration,
) -> Option<&mut StructuralAccess> {
    let StructuralTypeShape::Reference { access, .. } = &mut declaration.shape else {
        return None;
    };
    Some(access)
}

fn zeroed_structural_home_mut(
    graph: &mut TargetControlGraph,
    operation: usize,
) -> Option<&mut TargetStructuralHomeRequirement> {
    let TargetUnitOperation::EstablishZeroedStructural { result_home, .. } =
        &mut graph.blocks[0].operations[operation]
    else {
        return None;
    };
    Some(result_home)
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
            path: vec![terminal_psi::StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(6),
            )],
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
                indices: Vec::new(),
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
                ..
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
                6 => source
                    .path
                    .push(terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: ValueId::new(99).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(99).unwrap(),
                    }),
                7 => {
                    *operation = TargetUnitOperation::StructuralScalarFieldRead {
                        psi_operation: *psi_operation,
                        result: *result,
                        source: source.clone(),
                        case: None,
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
        // Publication cannot silently change the source declaration; permission
        // of a matching declaration is independently checked by native replay.
        target.graph.parameters[0].access = StructuralAccess::Owned;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

#[test]
fn local_byte_length_membership_requires_the_exact_owned_home() {
    use target_operations::{
        TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    };
    use terminal_psi::StructuralOperationResult;

    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let (mut function, mut target) = field_fixture(scalar);
        let parameter = function.structural_parameters.remove(0);
        target.graph.parameters.clear();
        let producer = OperationId::new(2).unwrap();
        let owner = StructuralOperationResult {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        };
        let observation = OperationId::new(1).unwrap();
        let result = AbstractResult {
            value: ValueId::new(1).unwrap(),
            scalar_type: scalar,
        };
        let source = StructuralArgument {
            place: owner.place,
            access: StructuralAccess::Owned,
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(6),
            )],
        };
        let field = StructuralFieldId::new(1).unwrap();
        function.operations[0] = AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation: observation,
            result,
            source: owner.place,
            path: source.path.clone(),
            field,
        };
        function.operations.insert(
            0,
            AbstractOperation::EstablishZeroedStructural {
                psi_operation: producer,
                result: owner.clone(),
            },
        );
        target.graph.blocks[0].operations[0] =
            TargetUnitOperation::StructuralByteSequenceFieldLength {
                psi_operation: observation,
                result,
                source,
                field,
                indices: Vec::new(),
            };
        target.graph.blocks[0].operations.insert(
            0,
            TargetUnitOperation::EstablishZeroedStructural {
                psi_operation: producer,
                result_home: TargetStructuralHomeRequirement {
                    origin: TargetStructuralHomeOrigin::OperationResult {
                        operation: producer,
                        result: owner.clone(),
                    },
                    layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(8, 8)),
                },
            },
        );
        assert!(retained(&function, &function.operations[1], &target));
        assert!(
            super::read_access(
                &function,
                &target,
                owner.place,
                super::AccessPurpose::ByteOwnerReplacement
            )
            .is_none()
        );
        // The same home can receive a whole-field replacement only for an
        // unrestricted record. This remains distinct from whole-byte-owner
        // replacement and from indexed writes through borrowed formals.
        let mut stored_function = function.clone();
        let mut stored_target = target.clone();
        stored_target.graph.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: owner.structural_type,
            identity: terminal_psi::TerminalIdentityIndex(10),
            shape: terminal_psi::StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    storage: None,
                    id: field,
                    identity: terminal_psi::TerminalIdentityIndex(11),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                }],
            },
        }]
        .into();
        stored_function.operations[1] = AbstractOperation::StructuralByteSequenceFieldStore {
            psi_operation: observation,
            destination: owner.place,
            path: Vec::new(),
            field,
            source: PlaceId::new(2).unwrap(),
            length: result.value,
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        };
        stored_target.graph.blocks[0].operations[1] =
            TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation: observation,
                destination: StructuralArgument {
                    place: owner.place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                },
                field,
                source: PlaceId::new(2).unwrap(),
                length: target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value: result.value,
                    scalar_type: scalar,
                },
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            };
        assert_eq!(
            replacement_retained(
                &stored_function,
                &stored_function.operations[1],
                &stored_target
            ),
            multiplicity == StructuralMultiplicity::Unrestricted
        );
        if multiplicity == StructuralMultiplicity::Unrestricted {
            for corruption in 0..4 {
                let mut changed = stored_target.clone();
                match corruption {
                    0 => {
                        changed.graph.blocks[0].operations.remove(0);
                    }
                    1 => {
                        let result_home =
                            zeroed_structural_home_mut(&mut changed.graph, 0).expect("owned home");
                        let TargetStructuralHomeOrigin::OperationResult { operation, .. } =
                            &mut result_home.origin
                        else {
                            panic!("owned origin")
                        };
                        *operation = OperationId::new(99).unwrap();
                    }
                    2 | 3 => {
                        let destination =
                            byte_store_destination_mut(&mut changed.graph, 1).expect("replacement");
                        if corruption == 2 {
                            destination.access = StructuralAccess::MutableBorrow;
                        } else {
                            destination.place = PlaceId::new(2).unwrap();
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !replacement_retained(
                        &stored_function,
                        &stored_function.operations[1],
                        &changed
                    ),
                    "forged owned replacement {corruption}"
                );
            }
        }
        for mutation in 0..5 {
            let mut changed = target.clone();
            match mutation {
                0 => {
                    changed.graph.blocks[0].operations.remove(0);
                }
                1 | 2 => {
                    let result_home =
                        zeroed_structural_home_mut(&mut changed.graph, 0).expect("owned home");
                    let TargetStructuralHomeOrigin::OperationResult { operation, result } =
                        &mut result_home.origin
                    else {
                        panic!("owned producer")
                    };
                    if mutation == 1 {
                        *operation = observation;
                    } else {
                        result.place = PlaceId::new(2).unwrap();
                    }
                }
                3 => {
                    let TargetUnitOperation::StructuralByteSequenceFieldLength { source, .. } =
                        &mut changed.graph.blocks[0].operations[1]
                    else {
                        panic!("metadata")
                    };
                    source
                        .path
                        .push(terminal_psi::StructuralPathSegment::FixedIndex(99));
                }
                _ => {
                    let duplicate = changed.graph.blocks[0].operations[1].clone();
                    changed.graph.blocks[0].operations.push(duplicate);
                }
            }
            assert!(
                !retained(&function, &function.operations[1], &changed),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn whole_byte_replacement_rejoins_qualified_entry_and_arrival_storage() {
    use semantic_vocabulary::{ObligationId, StructuralDomainId};
    use target_operations::TargetUnitScalarArgumentSource;
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape};

    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for arrival in [false, true] {
        let (mut function, mut target) = field_fixture(scalar);
        let parameter = &mut function.structural_parameters[0];
        parameter.access = StructuralAccess::MutableBorrow;
        parameter.qualifications = vec![StructuralDomainId::new(1).unwrap()];
        target.graph.parameters[0].access = parameter.access;
        target.graph.structural_types = vec![StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: parameter.structural_type,
            identity: terminal_psi::TerminalIdentityIndex(12),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 16,
            }),
        }]
        .into();
        let destination = parameter.place;
        let source = PlaceId::new(2).unwrap();
        let length = ValueId::new(1).unwrap();
        let obligation = ObligationId::new(1).unwrap();
        let psi_operation = OperationId::new(1).unwrap();
        function.operations[0] = AbstractOperation::ByteSequenceStore {
            psi_operation,
            destination,
            source,
            length,
            obligation,
        };
        target.graph.blocks[0].operations[0] = TargetUnitOperation::ByteSequenceStore {
            psi_operation,
            destination,
            source,
            length: TargetUnitScalarArgumentSource::Parameter {
                parameter_index: 0,
                source_value: length,
                scalar_type: scalar,
            },
            obligation,
        };
        if arrival {
            let block = BlockId::new(2).unwrap();
            let parameter = function.structural_parameters.remove(0);
            target.graph.parameters.clear();
            target.graph.blocks[0].block = block;
            target.graph.blocks[0].structural_parameters = vec![parameter.clone()];
            function
                .block_entries
                .push(abstract_operations::AbstractBlockEntry {
                    block,
                    structural_parameters: vec![parameter],
                    parameters: Vec::new(),
                    operation_offset: 0,
                });
        }
        assert!(replacement_retained(
            &function,
            &function.operations[0],
            &target
        ));
        for corruption in ["access", "type", "position", "qualification"] {
            let mut changed = target.clone();
            if arrival {
                let parameter = &mut changed.graph.blocks[0].structural_parameters[0];
                match corruption {
                    "access" => parameter.access = StructuralAccess::SharedBorrow,
                    "type" => parameter.structural_type = StructuralTypeId::new(2).unwrap(),
                    "position" => parameter.position = 1,
                    "qualification" => parameter.qualifications.clear(),
                    _ => unreachable!(),
                }
            } else {
                // The entry ABI omits semantic root qualifications; they remain
                // checked by mandatory source replay, not fabricated ABI fields.
                let parameter = &mut changed.graph.parameters[0];
                match corruption {
                    "access" => parameter.access = StructuralAccess::SharedBorrow,
                    "type" => parameter.structural_type = StructuralTypeId::new(2).unwrap(),
                    "position" => parameter.place = PlaceId::new(3).unwrap(),
                    "qualification" => changed.graph.structural_types = Vec::new().into(),
                    _ => unreachable!(),
                }
            }
            assert!(
                !replacement_retained(&function, &function.operations[0], &changed),
                "arrival={arrival}, {corruption}"
            );
        }
        // Even matching source/target declarations cannot grant shared storage
        // write authority. The mandatory replay still owns liveness and proofs.
        if arrival {
            function.block_entries[0].structural_parameters[0].access =
                StructuralAccess::SharedBorrow;
            target.graph.blocks[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        } else {
            function.structural_parameters[0].access = StructuralAccess::SharedBorrow;
            target.graph.parameters[0].access = StructuralAccess::SharedBorrow;
        }
        assert!(!replacement_retained(
            &function,
            &function.operations[0],
            &target
        ));
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
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(6),
            )],
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
        let destination = byte_store_destination_mut(&mut target.graph, 0).unwrap();
        destination.access = StructuralAccess::SharedBorrow;
        assert!(!replacement_retained(
            &function,
            &function.operations[0],
            &target
        ));
    }
}

#[test]
fn returned_reference_byte_store_retains_exact_pointer_home_and_candidates() {
    use semantic_vocabulary::ObligationId;
    use target_operations::{
        TargetCallResult, TargetReferenceResult, TargetStructuralHomeLayout,
        TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
        TargetUnitScalarArgumentSource,
    };
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
        StructuralOperationResult, StructuralReferenceResultSource, StructuralResultDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let (mut function, mut target) = field_fixture(count_type);
    let root = function.structural_parameters[0].place;
    let root_type = StructuralTypeId::new(1).unwrap();
    let reference_type = StructuralTypeId::new(2).unwrap();
    let holder_type = StructuralTypeId::new(3).unwrap();
    let carrier = PlaceId::new(2).unwrap();
    let source = PlaceId::new(3).unwrap();
    let call_operation = OperationId::new(1).unwrap();
    let store_operation = OperationId::new(2).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let length = ValueId::new(1).unwrap();
    let obligation = ObligationId::new(1).unwrap();
    let holder_path = vec![StructuralPathSegment::Field(
        terminal_psi::TerminalIdentityIndex(13),
    )];
    let result = StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: carrier,
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    function.structural_parameters[0].structural_type = root_type;
    function.operations = vec![
        AbstractOperation::CallStructural {
            psi_operation: call_operation,
            callee: MachineId::new(2).unwrap().into(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            result: structural_call_result(result.clone()),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::StructuralByteSequenceFieldStore {
            psi_operation: store_operation,
            destination: carrier,
            path: vec![StructuralPathSegment::Referent],
            field,
            source,
            length,
            obligation,
        },
    ];
    target.graph.parameters[0].access = StructuralAccess::MutableBorrow;
    target.graph.parameters[0].structural_type = root_type;
    target.graph.structural_types = vec![
        StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: root_type,
            identity: terminal_psi::TerminalIdentityIndex(14),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    storage: None,
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(13),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(holder_type),
                }],
            },
        },
        StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: reference_type,
            identity: terminal_psi::TerminalIdentityIndex(15),
            shape: StructuralTypeShape::Reference {
                referent: holder_type,
                access: StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: holder_type,
            identity: terminal_psi::TerminalIdentityIndex(16),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    storage: None,
                    id: field,
                    identity: terminal_psi::TerminalIdentityIndex(17),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 16 },
                    ),
                }],
            },
        },
    ]
    .into();
    let call_plan = target.graph.call_plan.clone();
    let declaration = StructuralResultDeclaration {
        place: PlaceId::new(20).unwrap(),
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: Vec::new(),
            sources: vec![
                StructuralArgument {
                    place: PlaceId::new(21).unwrap(),
                    access: StructuralAccess::MutableBorrow,
                    path: Vec::new(),
                }
                .into(),
            ],
        }],
    };
    target.graph.blocks[0].operations = vec![
        TargetUnitOperation::Call {
            origin: target_operations::NativeCallOrigin::Authored,
            psi_operation: call_operation,
            target: target_operations::NativeCallTarget::Machine(MachineId::new(2).unwrap()),
            call_plan,
            result: TargetCallResult::Structural {
                result: result.clone(),
                callee_result: target_operations::TargetStructuralResultContract::Machine(
                    declaration,
                ),
                result_home: Some(TargetStructuralHomeRequirement {
                    origin: TargetStructuralHomeOrigin::OperationResult {
                        operation: call_operation,
                        result: result.clone(),
                    },
                    layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(8, 8)),
                }),
                reference_results: vec![TargetReferenceResult {
                    path: Vec::new(),
                    origins: vec![target_operations::TargetReferenceOrigin {
                        root,
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                        path: holder_path
                            .iter()
                            .cloned()
                            .map(terminal_psi::ReferenceOriginProjection::Exact)
                            .collect(),
                    }],
                }],
            },
            scalar_arguments: Vec::new(),
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        TargetUnitOperation::StructuralByteSequenceFieldStore {
            psi_operation: store_operation,
            destination: StructuralArgument {
                place: carrier,
                access: StructuralAccess::MutableBorrow,
                path: vec![StructuralPathSegment::Referent],
            },
            field,
            source,
            length: TargetUnitScalarArgumentSource::Parameter {
                parameter_index: 0,
                source_value: length,
                scalar_type: count_type,
            },
            obligation,
        },
    ];
    assert!(replacement_retained(
        &function,
        &function.operations[1],
        &target
    ));

    // The selected pointer, not the first candidate, remains the write subject.
    let mut grouped = target.clone();
    let fields = record_fields_mut(&mut grouped.graph.structural_types.make_mut()[0]).unwrap();
    fields.push(StructuralFieldDeclaration {
        storage: None,
        id: StructuralFieldId::new(3).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(18),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(holder_type),
    });
    let reference_results = structural_call_reference_results_mut(&mut grouped.graph, 0);
    let root = reference_results[0].origins[0].root;
    reference_results[0]
        .origins
        .push(target_operations::TargetReferenceOrigin {
            root,
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            path: vec![terminal_psi::ReferenceOriginProjection::Exact(
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(18)),
            )],
        });
    assert!(replacement_retained(
        &function,
        &function.operations[1],
        &grouped
    ));
    assert_reference_field_read_retains_carrier(&function, &grouped, holder_type);
    let reference_results = structural_call_reference_results_mut(&mut grouped.graph, 0);
    reference_results[0].origins.swap(0, 1);
    assert!(!replacement_retained(
        &function,
        &function.operations[1],
        &grouped
    ));

    use selected_instructions::{
        SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessOrigin as Origin,
        SelectedMemoryAccessRole as Role,
    };
    let accepted_fact = || {
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"reference field copy",
        )
    };
    let access = |instruction, place, byte_offset, byte_count, role| SelectedMemoryAccess {
        instruction: SelectedInstructionId(instruction),
        origin: Origin::Operation(store_operation),
        place,
        byte_offset,
        byte_count,
        role,
    };
    let footprints = vec![
        access(0, carrier, 0, 8, Role::ReadPlace),
        access(
            1,
            source,
            0,
            0,
            Role::ReadByteSpan {
                length,
                obligation,
                accepted_fact: accepted_fact(),
            },
        ),
        access(
            1,
            carrier,
            8,
            0,
            Role::WriteByteSpan {
                length,
                obligation,
                accepted_fact: accepted_fact(),
            },
        ),
        access(2, carrier, 0, 8, Role::WritePlace),
    ];
    assert!(replacement_footprints_retained(
        &function,
        &function.operations[1],
        Some(&target),
        &footprints,
        &target.graph.structural_types
    ));
    for corruption in 0..5 {
        let mut changed = footprints.clone();
        match corruption {
            0 => {
                changed.remove(0);
            }
            1 => changed[0].place = root,
            2 => changed[0].byte_offset = 8,
            3 => changed[0].instruction = SelectedInstructionId(2),
            _ => changed.insert(0, changed[0].clone()),
        }
        assert!(
            !replacement_footprints_retained(
                &function,
                &function.operations[1],
                Some(&target),
                &changed,
                &target.graph.structural_types
            ),
            "pointer footprint corruption {corruption}"
        );
    }

    for corruption in 0..8 {
        let mut changed_function = function.clone();
        let mut changed_target = target.clone();
        match corruption {
            0 => {
                structural_call_reference_results_mut(&mut changed_target.graph, 0)[0].origins[0]
                    .root = PlaceId::new(9).unwrap();
            }
            1 => {
                structural_call_reference_results_mut(&mut changed_target.graph, 0)[0].origins[0]
                    .path
                    .clear();
            }
            2 => {
                *reference_access_mut(&mut changed_target.graph.structural_types.make_mut()[1])
                    .unwrap() = StructuralAccess::SharedBorrow;
            }
            3 => {
                let AbstractOperation::StructuralByteSequenceFieldStore { path, .. } =
                    &mut changed_function.operations[1]
                else {
                    unreachable!()
                };
                path.clear();
            }
            4..=6 => {
                let result_home = structural_call_result_home_mut(&mut changed_target.graph, 0);
                *result_home = Some(TargetStructuralHomeRequirement {
                    origin: TargetStructuralHomeOrigin::OperationResult {
                        operation: if corruption == 5 {
                            OperationId::new(99).unwrap()
                        } else {
                            call_operation
                        },
                        result: result.clone(),
                    },
                    layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(
                        if corruption == 6 { 16 } else { 8 },
                        8,
                    )),
                });
                if corruption == 4 {
                    *result_home = None;
                }
            }
            _ => {
                let destination = byte_store_destination_mut(&mut changed_target.graph, 1).unwrap();
                destination.place = root;
                destination.path = holder_path.clone();
            }
        }
        assert!(
            !replacement_retained(
                &changed_function,
                &changed_function.operations[1],
                &changed_target,
            ),
            "corruption {corruption}"
        );
    }
}

fn assert_reference_field_read_retains_carrier(
    function: &AbstractFunction,
    target: &TargetFunction,
    holder_type: StructuralTypeId,
) {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    let mut function = function.clone();
    let mut target = target.clone();
    let AbstractOperation::StructuralByteSequenceFieldStore {
        psi_operation,
        destination: carrier,
        field,
        ..
    } = function.operations[1]
    else {
        panic!("reference store fixture");
    };
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let result = AbstractResult {
        value: ValueId::new(2).unwrap(),
        scalar_type: scalar,
    };
    let declaration = target
        .graph
        .structural_types
        .make_mut()
        .iter_mut()
        .find(|declaration| declaration.id == holder_type)
        .unwrap();
    let fields = record_fields_mut(declaration).expect("record referent");
    fields[0].field_type = StructuralFieldType::Scalar(scalar);
    function.operations[1] = AbstractOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        source: carrier,
        path: Vec::new(),
        field,
    };
    target.graph.blocks[0].operations[1] = TargetUnitOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        source: StructuralArgument {
            place: carrier,
            access: StructuralAccess::MutableBorrow,
            path: vec![StructuralPathSegment::Referent],
        },
        case: None,
        field,
    };
    assert!(retained(&function, &function.operations[1], &target));
    for corruption in [
        "missing crossing",
        "static origin",
        "invented attenuation",
        "producer",
        "write-only",
    ] {
        let mut changed = target.clone();
        match corruption {
            "missing crossing" | "static origin" | "invented attenuation" => {
                let source = scalar_field_read_source_mut(&mut changed.graph, 1);
                if corruption == "missing crossing" {
                    source.path.clear();
                } else if corruption == "invented attenuation" {
                    source.access = StructuralAccess::SharedBorrow;
                } else {
                    source.place = function.structural_parameters[0].place;
                    source.path = vec![StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(13),
                    )];
                }
            }
            "producer" => {
                let result_home = structural_call_result_home_mut(&mut changed.graph, 0);
                let Some(home) = result_home.as_mut() else {
                    panic!("reference result home");
                };
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    ..
                } = &mut home.origin
                else {
                    panic!("operation result");
                };
                *operation = OperationId::new(99).unwrap();
            }
            _ => {
                let reference = changed
                    .graph
                    .structural_types
                    .make_mut()
                    .iter_mut()
                    .find(|declaration| {
                        matches!(declaration.shape, StructuralTypeShape::Reference { .. })
                    })
                    .unwrap();
                *reference_access_mut(reference).expect("reference type") =
                    StructuralAccess::WriteOnlyBorrow;
            }
        }
        assert!(
            !retained(&function, &function.operations[1], &changed),
            "{corruption}"
        );
    }
}

#[test]
fn returned_and_parameter_bounded_owners_use_inline_payload_offset() {
    use selected_instructions::{
        SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
        SelectedMemoryAccessRole,
    };
    use semantic_vocabulary::{ObligationId, StructuralDomainId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralOperationResult, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let (mut function, mut target) = field_fixture(count_type);
    function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    target.graph.parameters[0].access = StructuralAccess::MutableBorrow;
    let destination = function.structural_parameters[0].place;
    let source = PlaceId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(2).unwrap();
    let call_operation = OperationId::new(1).unwrap();
    let store_operation = OperationId::new(2).unwrap();
    let length = ValueId::new(1).unwrap();
    let obligation = ObligationId::new(1).unwrap();
    let result = StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: source,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![StructuralDomainId::new(1).unwrap()],
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    function.operations = vec![
        AbstractOperation::CallStructural {
            psi_operation: call_operation,
            callee: MachineId::new(2).unwrap().into(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            result: structural_call_result(result),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        AbstractOperation::StructuralByteSequenceFieldStore {
            psi_operation: store_operation,
            destination,
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
            source,
            length,
            obligation,
        },
    ];
    let structural_types = vec![StructuralTypeDeclaration {
        quotient_carrier: None,
        no_code_affine_cleanup: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: structural_type,
        identity: terminal_psi::TerminalIdentityIndex(19),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
            capacity: 16,
        }),
    }];
    let accepted_fact =
        || optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(b"copy bound");
    let access = |instruction, place, byte_offset, byte_count, role| SelectedMemoryAccess {
        instruction: SelectedInstructionId(instruction),
        origin: SelectedMemoryAccessOrigin::Operation(store_operation),
        place,
        byte_offset,
        byte_count,
        role,
    };
    let accesses = vec![
        access(
            0,
            source,
            8,
            0,
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation,
                accepted_fact: accepted_fact(),
            },
        ),
        access(
            0,
            destination,
            8,
            0,
            SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact: accepted_fact(),
            },
        ),
        access(1, destination, 0, 8, SelectedMemoryAccessRole::WritePlace),
    ];
    assert!(replacement_footprints_retained(
        &function,
        &function.operations[1],
        Some(&target),
        &accesses,
        &structural_types,
    ));

    let mut descriptor_offset = accesses.clone();
    descriptor_offset[0].byte_offset = 0;
    assert!(!replacement_footprints_retained(
        &function,
        &function.operations[1],
        Some(&target),
        &descriptor_offset,
        &structural_types,
    ));

    let borrowed = [StructuralTypeDeclaration {
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
        ..structural_types[0].clone()
    }];
    assert!(!replacement_footprints_retained(
        &function,
        &function.operations[1],
        Some(&target),
        &accesses,
        &borrowed,
    ));

    // The same bytes can arrive as an ordinary owned formal. Removing the
    // producer ensures the footprint comes from the parameter's own contract.
    function.operations.remove(0);
    function
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 1,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: vec![StructuralDomainId::new(1).unwrap()],
            projected_qualifications: Vec::new(),
        });
    assert!(replacement_footprints_retained(
        &function,
        &function.operations[0],
        Some(&target),
        &accesses,
        &structural_types,
    ));
    assert!(!replacement_footprints_retained(
        &function,
        &function.operations[0],
        Some(&target),
        &descriptor_offset,
        &structural_types,
    ));
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        function.structural_parameters[1].access = access;
        assert!(!replacement_footprints_retained(
            &function,
            &function.operations[0],
            Some(&target),
            &accesses,
            &structural_types,
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
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(6),
            )],
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
            AbstractOperation::StructuralScalarFieldRead { path, .. } => path.push(
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(carrier_field),
            ),
            _ => unreachable!(),
        }
        target.graph.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: function.structural_parameters[0].structural_type,
            identity: terminal_psi::TerminalIdentityIndex(20),
            shape: terminal_psi::StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    storage: None,
                    id: carrier_field,
                    identity: terminal_psi::TerminalIdentityIndex(21),
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
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: child,
                identity: terminal_psi::TerminalIdentityIndex(22),
                shape: terminal_psi::StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        storage: None,
                        id: carrier_field,
                        identity: terminal_psi::TerminalIdentityIndex(23),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                    }],
                },
            });
        let source = scalar_field_read_source_mut(&mut target.graph, 0);
        source.path.push(StructuralPathSegment::Field(
            terminal_psi::TerminalIdentityIndex(21),
        ));
        assert!(retained(&function, &function.operations[0], &target));
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(24),
            )],
        ] {
            let mut changed = target.clone();
            let source = scalar_field_read_source_mut(&mut changed.graph, 0);
            source.path = path;
            assert!(!retained(&function, &function.operations[0], &changed));
        }
        let fields = record_fields_mut(&mut target.graph.structural_types.make_mut()[0]).unwrap();
        fields[0].relevance = terminal_psi::BindingRelevance::Erased;
        assert!(!retained(&function, &function.operations[0], &target));
    }
}

#[test]
fn indexed_field_membership_reconstructs_fixed_array_carrier_identity() {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    for scalar in field_types() {
        let (mut function, mut target) = field_fixture(scalar);
        let maps_field = StructuralFieldId::new(2).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let element = StructuralTypeId::new(3).unwrap();
        let leaf = match &mut function.operations[0] {
            AbstractOperation::StructuralScalarFieldRead { path, field, .. } => {
                *path = vec![Segment::Field(maps_field), Segment::FixedIndex(1)];
                *field
            }
            _ => unreachable!(),
        };
        target.graph.structural_types = vec![
            terminal_psi::StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: function.structural_parameters[0].structural_type,
                identity: terminal_psi::TerminalIdentityIndex(20),
                shape: terminal_psi::StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        storage: None,
                        id: maps_field,
                        identity: terminal_psi::TerminalIdentityIndex(25),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Structural(array),
                    }],
                },
            },
            terminal_psi::StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: array,
                identity: terminal_psi::TerminalIdentityIndex(26),
                shape: terminal_psi::StructuralTypeShape::FixedArray { element, length: 2 },
            },
            terminal_psi::StructuralTypeDeclaration {
                quotient_carrier: None,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: element,
                identity: terminal_psi::TerminalIdentityIndex(27),
                shape: terminal_psi::StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        storage: None,
                        id: leaf,
                        identity: terminal_psi::TerminalIdentityIndex(23),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                    }],
                },
            },
        ]
        .into();
        let source = scalar_field_read_source_mut(&mut target.graph, 0);
        source.path = vec![
            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(25)),
            StructuralPathSegment::FixedIndex(1),
        ];
        assert!(retained(&function, &function.operations[0], &target));
        // A differing retained path cannot stand in for the exact literal
        // element projection, in either segment kind.
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(25),
            )],
            vec![
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(25)),
                StructuralPathSegment::FixedIndex(0),
            ],
            vec![
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(25)),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(28)),
            ],
            vec![StructuralPathSegment::FixedIndex(1)],
        ] {
            let mut changed = target.clone();
            let source = scalar_field_read_source_mut(&mut changed.graph, 0);
            source.path = path;
            assert!(!retained(&function, &function.operations[0], &changed));
        }
        // Every malformed canonical carrier stays fail-closed: an index
        // outside the declared extent, a leading or mid-path index, a case
        // segment, and a field carrier that is not a fixed array.
        let mut out_of_bounds = function.clone();
        match &mut out_of_bounds.operations[0] {
            AbstractOperation::StructuralScalarFieldRead { path, .. } => {
                *path = vec![Segment::Field(maps_field), Segment::FixedIndex(2)];
            }
            _ => unreachable!(),
        }
        assert!(!retained(
            &out_of_bounds,
            &out_of_bounds.operations[0],
            &target
        ));
        for path in [
            vec![Segment::FixedIndex(0), Segment::Field(maps_field)],
            vec![
                Segment::Field(maps_field),
                Segment::FixedIndex(0),
                Segment::Field(maps_field),
            ],
            vec![
                Segment::Field(maps_field),
                Segment::Case(semantic_vocabulary::StructuralCaseId::new(1).unwrap()),
            ],
        ] {
            let mut invalid = function.clone();
            match &mut invalid.operations[0] {
                AbstractOperation::StructuralScalarFieldRead { path: actual, .. } => {
                    *actual = path;
                }
                _ => unreachable!(),
            }
            assert!(!retained(&invalid, &invalid.operations[0], &target));
        }
        let mut non_array = target.clone();
        non_array.graph.structural_types.make_mut()[1].shape =
            terminal_psi::StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    storage: None,
                    id: leaf,
                    identity: terminal_psi::TerminalIdentityIndex(23),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                }],
            };
        assert!(!retained(&function, &function.operations[0], &non_array));
    }
}

fn owned_field_fixture(
    scalar_type: ScalarType,
    multiplicity: StructuralMultiplicity,
) -> (AbstractFunction, TargetFunction) {
    let (mut function, mut target) = field_fixture(scalar_type);
    let parameter = &mut function.structural_parameters[0];
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
                    3 => source.path.push(StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(29),
                    )),
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
        let source = scalar_field_read_source_mut(&mut target.graph, 0);
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
        for mutation in 0..8 {
            let mut changed = target.clone();
            let TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                case,
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
                5 => source.path.push(StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(29),
                )),
                // A record read relabeled as a case payload read would move
                // selection to an overlay offset the carrier never named.
                6 => *case = Some(semantic_vocabulary::StructuralCaseId::new(1).unwrap()),
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
        if let AbstractOperation::StructuralScalarFieldRead { result, .. } = operation
            && result.scalar_type != ScalarType::Boolean
        {
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
