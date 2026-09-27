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
        for mutation in 0..3 {
            let mut changed = selected.plan().clone();
            if mutation == 2 {
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
