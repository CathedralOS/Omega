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
fn element_case_observation_uses_backing_storage_and_rejects_descriptor_substitution() {
    use selected_instructions::{SelectedInstructionKind as I, SelectedMemoryAccessRole as M};
    let mut source = source();
    let tag = StructuralTypeId::new(3).unwrap();
    let mut declarations = source.structural_types.to_vec();
    let StructuralTypeShape::Record { fields } = &mut declarations[0].shape else {
        panic!("record element");
    };
    fields[1].field_type = StructuralFieldType::Structural(tag);
    declarations.push(StructuralTypeDeclaration {
        id: tag,
        identity: "Tag".into(),
        shape: StructuralTypeShape::Sum {
            cases: ["Empty", "Present"]
                .into_iter()
                .enumerate()
                .map(
                    |(position, identity)| terminal_psi::StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(position as u64 + 1)
                            .unwrap(),
                        identity: identity.into(),
                        fields: Vec::new(),
                    },
                )
                .collect(),
        },
    });
    source.structural_types = declarations.into();
    let function = &mut source.functions[0];
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(4).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let observed = OperationId::new(2).unwrap();
    function.operations[1] = O::StructuralCaseMembership {
        psi_operation: observed,
        result: AbstractResult {
            value: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Boolean,
        },
        source: PlaceId::new(1).unwrap(),
        path: vec![
            Segment::RuntimeIndex {
                index: ValueId::new(1).unwrap(),
                obligation: ObligationId::new(1).unwrap(),
            },
            Segment::Field("value".into()),
        ],
        case: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
    };
    let O::Return { scalar_type, .. } = &mut function.operations[2] else {
        panic!("scalar return");
    };
    *scalar_type = ScalarType::Boolean;
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let seed = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert!(
            crate::legalize_target_operations(&targeted, &source, &seed).is_err(),
            "{native:?}: the descriptor cannot replace the required index proof"
        );
        let unit = super::accept_referenced_obligations(seed);
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
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
        let instructions = selected.plan().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let descriptor_load = instructions
            .iter()
            .find(|instruction| {
                instruction.kind == I::Load64 { byte_offset: 0 }
                    && instruction.provenance.operations.contains(&observed)
            })
            .expect("membership first loads the descriptor's backing pointer");
        let descriptor = descriptor_load.operands[0].virtual_register;
        let backing = descriptor_load.operands[1].virtual_register;
        let element_address = instructions
            .iter()
            .find(|instruction| {
                instruction.kind == I::ByteViewAddress
                    && instruction
                        .provenance
                        .obligations
                        .contains(&ObligationId::new(1).unwrap())
            })
            .expect("the index scales onto the backing pointer");
        assert_eq!(
            element_address.operands[0].virtual_register, backing,
            "{native:?}"
        );
        assert_ne!(descriptor, backing, "{native:?}");
        let tag_load = instructions
            .iter()
            .find(|instruction| {
                instruction.kind == I::Load32 { byte_offset: 8 }
                    && instruction.provenance.operations.contains(&observed)
            })
            .expect("the nonzero field offset follows whole-element stride");
        assert_eq!(
            tag_load.operands[0].virtual_register, element_address.operands[2].virtual_register,
            "{native:?}"
        );
        for mutation in 0..3 {
            let mut changed = selected.plan().clone();
            if mutation == 2 {
                changed.functions[0]
                    .memory_accesses
                    .iter_mut()
                    .find(|access| matches!(access.role, M::ReadIndexedPrimitive { .. }))
                    .unwrap()
                    .byte_count = 8;
            } else {
                let instructions = changed.functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions);
                if mutation == 0 {
                    instructions
                        .into_iter()
                        .find(|instruction| {
                            instruction.kind == I::Load64 { byte_offset: 0 }
                                && instruction.provenance.operations.contains(&observed)
                        })
                        .unwrap()
                        .kind = I::CopyI64;
                } else {
                    instructions
                        .into_iter()
                        .find(|instruction| {
                            instruction.kind == I::ByteViewAddress
                                && instruction
                                    .provenance
                                    .obligations
                                    .contains(&ObligationId::new(1).unwrap())
                        })
                        .unwrap()
                        .operands[0]
                        .virtual_register = descriptor;
                }
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
                "{native:?}: changed view tag observation {mutation}"
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

/// Form the descriptor after an edge transfers storage or lends its address.
/// Both arrivals retain the collection originally established by the predecessor.
fn array_arrival_source(access: StructuralAccess) -> abstract_operations::AbstractOperationPlan {
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
            access,
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
                    access,
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
fn unused_established_view_rejects_invalid_result_metadata() {
    use super::owned_block_store::id;
    use target_operations::TargetUnitOperation;

    let native = target::NativeTarget::macos_arm64();
    let mut source = array_arrival_source(StructuralAccess::Owned);
    let function = &mut source.functions[0];
    function.result = AbstractFunctionResult::Unit;
    function.operations.truncate(3);
    function.operations.push(O::ReturnUnit {
        psi_edge: id(2),
        cleanup_actions: Vec::new(),
    });
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
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
    crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
        .unwrap();

    let mut changed_source = source.clone();
    let O::EstablishElementView { result, .. } = &mut changed_source.functions[0].operations[2]
    else {
        panic!("unused establishment");
    };
    result.multiplicity = StructuralMultiplicity::Linear;
    let mut changed_target = targeted.clone();
    let TargetUnitOperation::EstablishElementView { result, .. } = changed_target.functions[0]
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation, TargetUnitOperation::EstablishElementView { .. }))
        .unwrap()
    else {
        panic!("target establishment");
    };
    result.multiplicity = StructuralMultiplicity::Linear;
    let mut changed_unit = unit.clone();
    let O::EstablishElementView { result, .. } = changed_unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .map(|node| &mut node.operation)
        .find(|operation| matches!(operation, O::EstablishElementView { .. }))
        .unwrap()
    else {
        panic!("optimized establishment");
    };
    result.multiplicity = StructuralMultiplicity::Linear;
    changed_unit.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&changed_unit);
    // The unused linear descriptor also lacks its required cleanup. This is
    // an end-to-end custody control, not an isolated test of the native result
    // matcher: semantic admission must reject it before native publication.
    assert!(optimization_unit_semantics::validate_psi_optimization_unit(&changed_unit).is_err());
    assert!(
        crate::legalize_target_operations(&changed_target, &changed_source, &changed_unit).is_err()
    );
    let mut proposed = legalized.plan().clone();
    proposed.optimization_unit = changed_unit.identity;
    assert!(
        crate::validate_legalized_operations(
            &changed_target,
            &changed_source,
            &changed_unit,
            proposed,
        )
        .is_err()
    );
}

#[test]
fn array_arrival_element_view_retains_backing_and_rejects_substitution() {
    use super::owned_block_store::id;
    use selected_instructions::{
        LocalStorageSlotId, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    };
    use target_operations::{
        TargetElementView, TargetStructuralArgumentSource, TargetUnitOperation,
    };
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let source = array_arrival_source(access);
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
            crate::validate_legalized_operations(
                &targeted,
                &source,
                &unit,
                legalized.plan().clone(),
            )
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
                    .any(|observation| {
                        observation.place == id::<PlaceId>(11)
                            && match access {
                                StructuralAccess::Owned => {
                                    observation.origin
                                        == SelectedMemoryAccessOrigin::Operation(id(2))
                                        && observation.role
                                            == SelectedMemoryAccessRole::AddressLocal {
                                                slot:
                                                    LocalStorageSlotId::StructuralBlockParameter {
                                                        block: id(2),
                                                        place: id(11),
                                                    },
                                            }
                                }
                                _ => {
                                    observation.origin == SelectedMemoryAccessOrigin::Block(id(2))
                                        && observation.role == SelectedMemoryAccessRole::ReadPlace
                                        && observation.byte_count == 8
                                }
                            }
                    })
            );
        }
    }
}

#[test]
fn nested_array_view_replays_its_exact_backing_offset() {
    use super::owned_block_store::id;
    use target_operations::{TargetElementView, TargetUnitOperation};

    let mut source = array_arrival_source(StructuralAccess::Owned);
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: id(5),
            identity: "NestedArray".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(2),
                length: 2,
            },
        });
    let StructuralTypeShape::Record { fields } = &mut source.structural_types.make_mut()[2].shape
    else {
        panic!("the existing owned arrival carries a record");
    };
    fields[0].field_type = StructuralFieldType::Structural(id(5));
    let O::EstablishElementView {
        source: backing, ..
    } = &mut source.functions[0].operations[2]
    else {
        panic!("view establishment");
    };
    backing.path = vec![Segment::Field("values".into()), Segment::FixedIndex(1)];
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let mut targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(
            target::NativeTarget::macos_arm64(),
        ),
    )
    .unwrap();
    let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
    crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
        .unwrap();

    let offset = targeted.functions[0]
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match operation {
            TargetUnitOperation::EstablishElementView {
                view:
                    TargetElementView::Established {
                        source_byte_offset, ..
                    },
                ..
            } => Some(source_byte_offset),
            _ => None,
        })
        .expect("established descriptor");
    assert_eq!(
        *offset, 8,
        "the second two-i32 array starts after the first"
    );
    *offset = 0;
    assert!(crate::legalize_target_operations(&targeted, &source, &unit).is_err());
    assert!(
        crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone(),)
            .is_err(),
        "same-type neighboring storage cannot substitute for the selected array"
    );
}

#[test]
fn owned_arrival_element_view_rejects_unavailable_backing() {
    use super::owned_block_store::id;
    for mutation in 0..2 {
        let mut source = array_arrival_source(StructuralAccess::Owned);
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

fn returned_view_source() -> abstract_operations::AbstractOperationPlan {
    use abstract_operations::AbstractStructuralCallResult;
    let mut plan = source();
    let view = StructuralTypeId::new(2).unwrap();
    let original = PlaceId::new(1).unwrap();
    let first_result = PlaceId::new(4).unwrap();
    let second_result = PlaceId::new(5).unwrap();
    let mut callee = plan.functions[0].clone();
    callee.machine = semantic_vocabulary::MachineId::new(2).unwrap();
    callee.parameters.clear();
    callee.result = AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
        place: PlaceId::new(99).unwrap(),
        structural_type: view,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    callee.operations = vec![O::ReturnStructural {
        psi_edge: EdgeId::new(2).unwrap(),
        source: original,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }];
    let caller = &mut plan.functions[0];
    let mut other = caller.structural_parameters[0].clone();
    other.place = PlaceId::new(9).unwrap();
    other.position = 1;
    caller.structural_parameters.push(other);
    let call = |identity, place, parent| O::CallStructural {
        psi_operation: OperationId::new(identity).unwrap(),
        result: AbstractStructuralCallResult::Structural {
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type: view,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            returned_claim_transfers: Vec::new(),
            selected_evidence: Vec::new(),
        },
        callee: callee.machine,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: parent,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let mut operations = vec![
        call(10, first_result, original),
        call(11, second_result, first_result),
    ];
    for mut operation in std::mem::take(&mut caller.operations) {
        match &mut operation {
            O::ElementViewLength { source, .. } | O::ElementViewRead { source, .. } => {
                *source = second_result
            }
            _ => {}
        }
        operations.push(operation);
    }
    caller.operations = operations;
    plan.functions.push(callee);
    plan
}

#[test]
fn returned_view_calls_replay_backing_origins_result_homes_and_memory_reads() {
    use legalized_operations::{
        LegalizedDescriptorOrigin as Origin, LegalizedScalarInstructionKind as K,
    };
    use selected_instructions::{LocalStorageSlotId, SelectedMemoryAccessRole as M};
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = returned_view_source();
        let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .expect("two descriptor calls and a returned element read lower");
        let unit = super::accept_referenced_obligations(
            optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap(),
        );
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
        crate::validate_legalized_operations(&targeted, &source, &unit, legalized.plan().clone())
            .unwrap();
        let caller = &legalized.plan().scalar_functions[0];
        let calls = caller
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|row| {
                if let K::Call(call) = &row.kind {
                    Some(call)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        // Each call substitutes its immediate actual. The second origin is
        // the first descriptor result; its own call row retains the original
        // input backing. Neither result becomes an independent allocation.
        for (call, actual) in calls.into_iter().zip([1, 4]) {
            assert!(
                matches!(call.descriptor_origins.as_slice(), [Origin::Argument { source, exact: true, indirect: false }]
                if source.place == PlaceId::new(actual).unwrap() && source.path.is_empty()),
                "actual origins: {:?}",
                call.descriptor_origins
            );
        }
        for mutation in 0..6 {
            let mut changed = legalized.plan().clone();
            let call = changed.scalar_functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find_map(|row| match &mut row.kind {
                    K::Call(call) if row.operation == OperationId::new(11).unwrap() => Some(call),
                    _ => None,
                })
                .unwrap();
            match mutation {
                0 => {
                    if let Origin::Argument { source, .. } = &mut call.descriptor_origins[0] {
                        source.place = PlaceId::new(9).unwrap();
                    }
                }
                1 => {
                    if let Origin::Argument { source, .. } = &mut call.descriptor_origins[0] {
                        source.path.push(Segment::Referent);
                    }
                }
                2 => {
                    if let Origin::Argument { source, .. } = &mut call.descriptor_origins[0] {
                        source.access = StructuralAccess::MutableBorrow;
                    }
                }
                3 => {
                    if let Origin::Argument { indirect, .. } = &mut call.descriptor_origins[0] {
                        *indirect = true;
                    }
                }
                4 => {
                    call.descriptor_origins[0] = Origin::Literal {
                        machine: source.functions[1].machine,
                        place: PlaceId::new(1).unwrap(),
                    }
                }
                _ => call.descriptor_origins.clear(),
            }
            assert!(
                crate::validate_legalized_operations(&targeted, &source, &unit, changed).is_err(),
                "{native:?} origin mutation {mutation}"
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
        let caller = &selected.plan().functions[0];
        assert!(caller.memory_accesses.iter().any(|row| matches!(row.role, M::ReadReturnedView { producer } if producer == OperationId::new(11).unwrap())
            && row.place == PlaceId::new(5).unwrap() && row.byte_offset == 0 && row.byte_count == 0));
        for mutation in 0..3 {
            let mut changed = selected.plan().clone();
            let caller = &mut changed.functions[0];
            match mutation {
                0 => {
                    let access = caller
                        .memory_accesses
                        .iter_mut()
                        .find(|row| matches!(row.role, M::ReadReturnedView { .. }))
                        .unwrap();
                    access.role = M::ReadReturnedView {
                        producer: OperationId::new(10).unwrap(),
                    };
                }
                1 => caller
                    .memory_accesses
                    .retain(|row| !matches!(row.role, M::ReadReturnedView { .. })),
                _ => {
                    let home = caller.local_storage_slots.iter_mut().find(|slot| matches!(slot.id, LocalStorageSlotId::Structural { place, .. } if place == PlaceId::new(5).unwrap())).unwrap();
                    home.id = LocalStorageSlotId::Structural {
                        operation: OperationId::new(10).unwrap(),
                        place: PlaceId::new(5).unwrap(),
                    };
                }
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
                "{native:?} result residence mutation {mutation}"
            );
        }
    }
}

#[test]
fn returned_subslice_materializes_descriptor_home_and_replays_its_producer() {
    for bytes in [true, false] {
        let mut source = source();
        let view = StructuralTypeId::new(2).unwrap();
        if bytes {
            let mut types = source.structural_types.to_vec();
            types[1].shape = StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                },
            );
            source.structural_types = types.into();
        }
        let function = &mut source.functions[0];
        let place = PlaceId::new(1).unwrap();
        let length = ValueId::new(2).unwrap();
        let result = terminal_psi::StructuralOperationResult {
            place: PlaceId::new(4).unwrap(),
            structural_type: view,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        function.result =
            AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
                place: PlaceId::new(99).unwrap(),
                structural_type: view,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            });
        let measured = AbstractResult {
            value: length,
            scalar_type: integer(IntegerSign::Unsigned, 64),
        };
        function.operations = vec![
            if bytes {
                O::ByteSequenceLength {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: measured,
                    source: place,
                }
            } else {
                O::ElementViewLength {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: measured,
                    source: place,
                }
            },
            if bytes {
                O::ByteSequenceSubslice {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: result.clone(),
                    source: place,
                    start: ValueId::new(1).unwrap(),
                    end: length,
                    length,
                    obligation: ObligationId::new(1).unwrap(),
                }
            } else {
                O::ElementViewSubslice {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: result.clone(),
                    source: place,
                    start: ValueId::new(1).unwrap(),
                    end: length,
                    length,
                    obligation: ObligationId::new(1).unwrap(),
                }
            },
            O::ReturnStructural {
                psi_edge: EdgeId::new(1).unwrap(),
                source: result.place,
                returned_claims: Vec::new(),
                trivial_affine_locals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        ];
        let native = target::NativeTarget::macos_arm64();
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
        let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
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
        let mut changed = targeted.clone();
        let graph = &mut changed.functions[0].graph;
        let target_operations::TargetControlTerminator::ReturnStructural {
            source: target_operations::TargetStructuralReturnSource::Home(home),
            ..
        } = &mut graph.blocks[0].terminator
        else {
            panic!("descriptor home return");
        };
        let target_operations::TargetStructuralHomeOrigin::OperationResult { operation, .. } =
            &mut home.origin
        else {
            panic!("operation result home");
        };
        *operation = OperationId::new(1).unwrap();
        assert!(
            crate::legalize_target_operations(&changed, &source, &unit).is_err(),
            "bytes={bytes}: substituted producer"
        );
    }
}
