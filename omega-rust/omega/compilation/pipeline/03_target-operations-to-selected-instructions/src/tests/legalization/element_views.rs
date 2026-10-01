//! Element indexing keeps whole-element stride separate from scalar leaf geometry.
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation as O, AbstractParameter, AbstractResult,
};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, ObligationId, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment as Segment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(sign, bits).unwrap())
}

fn source() -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let element = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let place = PlaceId::new(1).unwrap();
    let index = ValueId::new(1).unwrap();
    let length = ValueId::new(2).unwrap();
    let read = ValueId::new(3).unwrap();
    let returned = ValueId::new(4).unwrap();
    let scalar = integer(IntegerSign::Signed, 16);
    let field = |number, identity: &str, scalar| StructuralFieldDeclaration {
        id: StructuralFieldId::new(number).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(scalar),
    };
    // Deliberately no standalone scalar type declarations: fields own their
    // scalar types, not synthetic structural identities.
    source.structural_types = vec![
        StructuralTypeDeclaration {
            id: element,
            identity: "Entry".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "prefix", integer(IntegerSign::Unsigned, 64)),
                    field(2, "value", scalar),
                    field(3, "suffix", integer(IntegerSign::Unsigned, 32)),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "Entries".into(),
            shape: StructuralTypeShape::ElementView { element },
        },
    ]
    .into();
    let function = &mut source.functions[0];
    function.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    }];
    function.parameters = vec![AbstractParameter {
        value: index,
        scalar_type: integer(IntegerSign::Unsigned, 64),
    }];
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: returned,
        scalar_type: scalar,
    });
    function.operations = vec![
        O::ElementViewLength {
            psi_operation: OperationId::new(1).unwrap(),
            result: AbstractResult {
                value: length,
                scalar_type: integer(IntegerSign::Unsigned, 64),
            },
            source: place,
        },
        O::ElementViewRead {
            psi_operation: OperationId::new(2).unwrap(),
            result: AbstractResult {
                value: read,
                scalar_type: scalar,
            },
            source: place,
            index,
            length,
            obligation: ObligationId::new(1).unwrap(),
            path: vec![Segment::Field("value".into())],
        },
        O::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: returned,
            value: read,
            scalar_type: scalar,
            cleanup_actions: vec![],
        },
    ];
    source
}

#[test]
fn element_field_reads_replay_leaf_geometry_and_reject_substitution() {
    use legalized_operations::LegalizedScalarInstructionKind as K;
    use selected_instructions::{SelectedInstructionKind as I, SelectedMemoryAccessRole as M};
    use target_operations::{TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation};
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let source = source();
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let unit = super::accept_referenced_obligations(
            optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap(),
        );
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let without_bounds = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert!(crate::legalize_target_operations(&targeted, &source, &without_bounds).is_err());
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
        crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
            .unwrap();

        for mutation in 0..2 {
            let mut changed = targeted.clone();
            let TargetUnitOperation::ScalarDefinition { expression, .. } = changed.functions[0]
                .graph
                .blocks[0]
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(operation, TargetUnitOperation::ScalarDefinition { result_home, .. }
                        if result_home.source_value == ValueId::new(3).unwrap())
                })
                .unwrap()
            else {
                panic!("scalar return");
            };
            let TargetScalarExpression::Integer { expression, .. } = expression else {
                panic!("integer return");
            };
            let TargetIntegerExpression::ElementViewRead {
                path,
                element_byte_offset,
                ..
            } = expression
            else {
                panic!("element read");
            };
            if mutation == 0 {
                *element_byte_offset = 0;
            } else {
                *path = vec![Segment::Field("prefix".into())];
            }
            assert!(
                crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                "{native:?} target mutation {mutation}"
            );
        }
        let mut changed = legalized.plan().clone();
        let K::ElementViewRead { path, .. } = &mut changed.scalar_functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, K::ElementViewRead { .. }))
            .unwrap()
            .kind
        else {
            panic!("element read");
        };
        path.clear();
        assert!(crate::validate_legalized_operations(&targeted, &source, &unit, changed).is_err());

        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let accesses = selected.plan().functions[0]
            .memory_accesses
            .iter()
            .filter(|access| matches!(access.role, M::ReadElementView { .. }))
            .map(|access| (access.byte_offset, access.byte_count))
            .collect::<Vec<_>>();
        assert_eq!(
            accesses,
            [(8, 2)],
            "{native:?}: leaf bytes, not the 16-byte stride"
        );
        // The exact-width load zero-extends; the signed leaf is restored to
        // its sign-extended register carrier before any consumer compares it.
        let rows = selected.plan().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let load = rows
            .iter()
            .position(|row| matches!(row.kind, I::Load16 { byte_offset: 8 }))
            .unwrap();
        assert_eq!(rows[load + 1].kind, I::SignExtendI16, "{native:?}");
        assert_eq!(
            rows[load + 1].operands[0].virtual_register,
            rows[load].operands[1].virtual_register,
            "{native:?}: the extension consumes the loaded leaf"
        );
        for mutation in 0..4 {
            let mut changed = selected.plan().clone();
            if mutation == 3 {
                changed.functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.kind == I::SignExtendI16)
                    .unwrap()
                    .kind = I::ZeroExtendU16;
            } else if mutation == 2 {
                changed.functions[0]
                    .memory_accesses
                    .iter_mut()
                    .find(|access| matches!(access.role, M::ReadElementView { .. }))
                    .unwrap()
                    .byte_count = 16;
            } else {
                let load = changed.functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| matches!(row.kind, I::Load16 { byte_offset: 8 }))
                    .unwrap();
                load.kind = if mutation == 0 {
                    I::Load16 { byte_offset: 0 }
                } else {
                    I::Load64 { byte_offset: 8 }
                };
            }
            assert!(
                crate::validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "{native:?} selected mutation {mutation}"
            );
        }
    }
}

#[test]
fn element_field_paths_reject_missing_and_non_scalar_leaves() {
    for path in [
        vec![],
        vec![Segment::Field("missing".into())],
        vec![Segment::Field("prefix".into())],
        vec![Segment::FixedIndex(0)],
    ] {
        let mut source = source();
        let O::ElementViewRead { path: actual, .. } = &mut source.functions[0].operations[1] else {
            panic!("element read");
        };
        *actual = path;
        assert!(
            abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(
                    target::NativeTarget::linux_x64()
                ),
            )
            .is_err()
        );
    }
}

#[test]
fn element_leaf_geometry_checks_nested_offsets_and_erased_fields() {
    use crate::structural_inputs::structural_reference_input::element_leaf_geometry;
    let source = source();
    let mut declarations = source.structural_types.as_slice().to_vec();
    let scalar = integer(IntegerSign::Signed, 16);
    let array = StructuralTypeId::new(3).unwrap();
    declarations.push(StructuralTypeDeclaration {
        id: array,
        identity: "EntryArray".into(),
        shape: StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(1).unwrap(),
            length: 2,
        },
    });
    let path = [Segment::FixedIndex(1), Segment::Field("value".into())];
    assert_eq!(
        element_leaf_geometry(array, &path, scalar, &declarations),
        Some((24, 2))
    );
    assert_eq!(
        element_leaf_geometry(
            array,
            &[Segment::FixedIndex(2), Segment::Field("value".into())],
            scalar,
            &declarations
        ),
        None
    );
    let StructuralTypeShape::Record { fields } = &mut declarations[0].shape else {
        panic!("record");
    };
    fields[1].relevance = BindingRelevance::Erased;
    assert_eq!(
        element_leaf_geometry(array, &path, scalar, &declarations),
        None
    );
    declarations[0].shape = StructuralTypeShape::PrimitiveScalar(scalar);
    assert_eq!(
        element_leaf_geometry(
            StructuralTypeId::new(1).unwrap(),
            &[],
            scalar,
            &declarations
        ),
        Some((0, 2))
    );
}

/// A state that receives an element view binds it as a block parameter. The
/// entry measures its own view parameter, then transfers it to the successor,
/// which measures the transferred descriptor. The other successor reads a
/// field of a shared record, so the function replays its complete aggregate
/// signature, including every block parameter. `block_access` is the
/// successor parameter's access, bound from an argument of the same access.
fn arrival_source(block_access: StructuralAccess) -> abstract_operations::AbstractOperationPlan {
    use abstract_operations::{AbstractBlockEntry, AbstractStructuralBinding, AbstractSuccessor};
    use semantic_vocabulary::BlockId;
    use terminal_psi::StructuralArgument;
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let element = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let place = PlaceId::new(10).unwrap();
    let arrived = PlaceId::new(11).unwrap();
    let holder = StructuralTypeId::new(3).unwrap();
    let held = PlaceId::new(12).unwrap();
    let count = integer(IntegerSign::Unsigned, 64);
    source.structural_types = vec![
        StructuralTypeDeclaration {
            id: element,
            identity: "Entry".into(),
            shape: StructuralTypeShape::PrimitiveScalar(integer(IntegerSign::Signed, 32)),
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "Entries".into(),
            shape: StructuralTypeShape::ElementView { element },
        },
        StructuralTypeDeclaration {
            id: holder,
            identity: "Holder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(integer(IntegerSign::Signed, 32)),
                }],
            },
        },
    ]
    .into();
    let parameter = |place, access| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let function = &mut source.functions[0];
    function.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type: count,
    }];
    function.structural_parameters = vec![
        parameter(place, StructuralAccess::SharedBorrow),
        StructuralParameterDeclaration {
            position: 1,
            structural_type: holder,
            ..parameter(held, StructuralAccess::SharedBorrow)
        },
    ];
    function.block_entries = vec![
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            structural_parameters: vec![parameter(arrived, block_access)],
            block: BlockId::new(2).unwrap(),
            parameters: Vec::new(),
            operation_offset: 3,
        },
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(3).unwrap(),
            parameters: Vec::new(),
            operation_offset: 5,
        },
    ];
    let successor = |identity, target, transfer: bool| AbstractSuccessor {
        structural_bindings: if transfer {
            vec![AbstractStructuralBinding {
                parameter: arrived,
                argument: StructuralArgument {
                    place,
                    access: block_access,
                    path: Vec::new(),
                },
            }]
        } else {
            Vec::new()
        },
        psi_edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let length = |operation, value, source| O::ElementViewLength {
        psi_operation: OperationId::new(operation).unwrap(),
        result: AbstractResult {
            value: ValueId::new(value).unwrap(),
            scalar_type: count,
        },
        source,
    };
    function.operations = vec![
        length(10, 101, place),
        O::IntegerLessThan {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(102).unwrap(),
            left: ValueId::new(100).unwrap(),
            right: ValueId::new(101).unwrap(),
        },
        O::Conditional {
            condition: ValueId::new(102).unwrap(),
            when_true: successor(10, 2, true),
            when_false: successor(11, 3, false),
        },
        length(12, 103, arrived),
        O::ReturnUnit {
            psi_edge: EdgeId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
        O::StructuralScalarFieldRead {
            psi_operation: OperationId::new(13).unwrap(),
            result: AbstractResult {
                value: ValueId::new(104).unwrap(),
                scalar_type: integer(IntegerSign::Signed, 32),
            },
            source: held,
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        },
        O::ReturnUnit {
            psi_edge: EdgeId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source
}

/// A borrowed element view crosses a state arrival exactly as a byte view
/// does: the successor's block parameter is admitted and its measurement
/// replays. An owned element-view parameter has no block home and is refused.
#[test]
fn a_borrowed_element_view_arrives_through_a_block_parameter() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let source = arrival_source(StructuralAccess::SharedBorrow);
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
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit)
            .unwrap_or_else(|error| panic!("{native:?}: {error:?}"));
        crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
            .unwrap();

        let owned = arrival_source(StructuralAccess::Owned);
        let refused = abstract_operations_to_target_operations::lower_to_target_operations(
            &owned,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .map_err(|_| ())
        .and_then(|targeted| {
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &owned,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .map_err(|_| ())?;
            crate::legalize_target_operations(&targeted, &owned, &unit)
                .map(|_| ())
                .map_err(|_| ())
        });
        assert!(
            refused.is_err(),
            "{native:?}: an owned view has no block home"
        );
    }
}

/// Form the descriptor after ownership crosses an edge: the arrival owns its
/// backing, even though a predecessor originally established the same bytes.
fn owned_array_arrival_source() -> abstract_operations::AbstractOperationPlan {
    use super::owned_block_store::id;
    use abstract_operations::{AbstractBlockEntry, AbstractStructuralBinding};
    use terminal_psi::{StructuralArgument, StructuralOperationResult};
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let scalar = integer(IntegerSign::Signed, 32);
    source.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "Element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "Array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(1),
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "Record".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "values".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id(2)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "View".into(),
            shape: StructuralTypeShape::ElementView { element: id(1) },
        },
    ]
    .into();
    let result = |place, structural_type| StructuralOperationResult {
        place: id(place),
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let function = &mut source.functions[0];
    let measured = AbstractResult {
        value: id(1),
        scalar_type: integer(IntegerSign::Unsigned, 64),
    };
    function.result = AbstractFunctionResult::Scalar(measured);
    function.block_entries.push(AbstractBlockEntry {
        block: id(2),
        parameters: Vec::new(),
        operation_offset: 2,
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(11),
            position: 0,
            is_self: false,
            structural_type: id(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
    });
    function.operations = vec![
        O::EstablishZeroedStructural {
            psi_operation: id(1),
            result: result(10, 3),
        },
        O::Jump {
            psi_edge: id(1),
            target: id(2),
            bindings: Vec::new(),
            structural_bindings: vec![AbstractStructuralBinding {
                parameter: id(11),
                argument: StructuralArgument {
                    place: id(10),
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                },
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
        O::EstablishElementView {
            psi_operation: id(2),
            result: result(12, 4),
            destination: id(12),
            element: id(1),
            source: StructuralArgument {
                place: id(11),
                access: StructuralAccess::SharedBorrow,
                path: vec![Segment::Field("values".into())],
            },
        },
        O::ElementViewLength {
            psi_operation: id(3),
            result: measured,
            source: id(12),
        },
        O::Return {
            psi_edge: id(2),
            result: measured.value,
            value: measured.value,
            scalar_type: measured.scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];
    source
}

#[test]
fn owned_arrival_element_view_retains_backing_and_rejects_substitution() {
    use super::owned_block_store::id;
    use selected_instructions::{
        LocalStorageSlotId, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    };
    use target_operations::{
        TargetElementView, TargetStructuralArgumentSource, TargetUnitOperation,
    };
    let source = owned_array_arrival_source();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
        crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
            .unwrap();
        for mutation in 0..4 {
            let mut changed = targeted.clone();
            let view = changed.functions[0]
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::EstablishElementView { view, .. } => Some(view),
                    _ => None,
                })
                .unwrap();
            let TargetElementView::Established {
                source: backing,
                extent,
                ..
            } = view
            else {
                panic!("established view");
            };
            match mutation {
                0 => {
                    *backing = TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: id(1),
                    }
                }
                1 => {
                    *backing = TargetStructuralArgumentSource::BlockParameter {
                        block: id(1),
                        place: id(11),
                    }
                }
                2 => {
                    *backing = TargetStructuralArgumentSource::BlockParameter {
                        block: id(2),
                        place: id(10),
                    }
                }
                _ => *extent += 1,
            }
            assert!(
                crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}"
            );
            assert!(
                crate::validate_legalized_operations(
                    &changed,
                    &source,
                    &unit,
                    legalized.plan().clone()
                )
                .is_err(),
                "replayed target mutation {mutation}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        assert!(
            selected.plan().functions[0]
                .memory_accesses
                .iter()
                .any(|access| {
                    access.origin == SelectedMemoryAccessOrigin::Operation(id(2))
                        && access.place == id::<PlaceId>(11)
                        && access.role
                            == SelectedMemoryAccessRole::AddressLocal {
                                slot: LocalStorageSlotId::StructuralBlockParameter {
                                    block: id(2),
                                    place: id(11),
                                },
                            }
                })
        );
    }
}

#[test]
fn owned_arrival_element_view_rejects_unavailable_backing() {
    use super::owned_block_store::id;
    for mutation in 0..2 {
        let mut source = owned_array_arrival_source();
        let function = &mut source.functions[0];
        if mutation == 0 {
            // The descriptor cannot read a future block's owned arrival.
            function.operations.swap(1, 2);
            function.block_entries[1].operation_offset = 3;
        } else {
            // Moving an affine root ends that old home even though its
            // establishing operation still dominates the descriptor.
            let O::EstablishZeroedStructural { result, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            result.multiplicity = StructuralMultiplicity::Affine;
            function.block_entries[1].structural_parameters[0].multiplicity =
                StructuralMultiplicity::Affine;
            let O::EstablishElementView { source, .. } = &mut function.operations[2] else {
                unreachable!()
            };
            source.place = id(10);
        }
        let admitted = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .map_err(|_| ())
        .and_then(|unit| {
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).map_err(|_| ())
        });
        assert!(admitted.is_err(), "unavailable backing mutation {mutation}");
    }
}
