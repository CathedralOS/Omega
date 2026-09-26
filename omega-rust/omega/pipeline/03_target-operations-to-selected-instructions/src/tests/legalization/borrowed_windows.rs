//! A borrowed window's move and store legalize as the two directions of one
//! extent copy.
//!
//! `self.inner = copy` over a mutable-borrowed `Outer { tag: i32, inner:
//! Inner }`: a leaf copy produces the replacement, the window move copies the
//! displaced `inner` out at offset 8, and the store copies the replacement's
//! home back to offset 8.
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::AbstractOperation;
use abstract_operations_to_target_operations::TargetLoweringRequest;
use legalized_operations::LegalizedScalarInstructionKind;
use selected_instructions::{SelectedInstructionKind, SelectedMemoryAccessRole};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId,
};
use target::NativeTarget;
use target_operations::TargetUnitOperation;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn integer(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap())
}

fn field(id: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type,
    }
}

fn record(
    id: u64,
    identity: &str,
    fields: Vec<StructuralFieldDeclaration>,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: identity.into(),
        shape: StructuralTypeShape::Record { fields },
    }
}

fn inner() -> StructuralTypeId {
    StructuralTypeId::new(10).unwrap()
}

fn operation(id: u64) -> OperationId {
    OperationId::new(id).unwrap()
}

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}

fn root(access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(20).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn result(id: u64) -> StructuralOperationResult {
    StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: place(id),
        structural_type: inner(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    }
}

/// Copy `inner` out as the replacement, open the window on `inner`, and
/// reseat it with the copy.
fn source(access: StructuralAccess) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    for declaration in [
        record(
            10,
            "test::Inner",
            vec![
                field(1, "first", StructuralFieldType::Scalar(integer(64))),
                field(2, "second", StructuralFieldType::Scalar(integer(32))),
            ],
        ),
        record(
            20,
            "test::Outer",
            vec![
                field(1, "tag", StructuralFieldType::Scalar(integer(32))),
                field(2, "inner", StructuralFieldType::Structural(inner())),
            ],
        ),
    ] {
        source.structural_types.make_mut().push(declaration);
    }
    let function = &mut source.functions[0];
    function.structural_parameters.push(root(access));
    function.operations.splice(
        0..0,
        [
            AbstractOperation::StructuralLeafCopy {
                psi_operation: operation(1),
                result: result(2),
                source: place(1),
                path: vec![StructuralPathSegment::Field("inner".into())],
            },
            AbstractOperation::MoveStructuralField {
                psi_operation: operation(2),
                result: result(3),
                source: root(access),
                path: Vec::new(),
                field: StructuralFieldId::new(2).unwrap(),
            },
            AbstractOperation::StoreStructuralField {
                psi_operation: operation(3),
                destination: root(access),
                path: Vec::new(),
                field: StructuralFieldId::new(2).unwrap(),
                value: StructuralArgument {
                    place: place(2),
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                },
            },
        ],
    );
    source
}

fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let source = source(StructuralAccess::MutableBorrow);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("a verified window pair lowers to its two extent copies");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("the window pair keeps canonical custody");
    (source, target, unit)
}

fn natives() -> [NativeTarget; 3] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ]
}

#[test]
fn window_pair_lowers_and_legalizes_as_extent_copies() {
    for native in natives() {
        let (source, target, unit) = fixture(native);
        let rows = &target.functions[0].graph.blocks[0].operations;
        assert!(rows.iter().any(|row| matches!(row,
            TargetUnitOperation::StructuralLeafCopy { psi_operation, source, path, byte_offset: 8, indices, result_home }
                if *psi_operation == operation(2)
                    && *source == place(1)
                    && path == &[StructuralPathSegment::Field("inner".into())]
                    && indices.is_empty()
                    && result_home.place() == place(3))));
        assert!(rows.iter().any(|row| matches!(row,
            TargetUnitOperation::StoreStructuralField { psi_operation, byte_offset: 8, value_home, .. }
                if *psi_operation == operation(3) && value_home.place() == place(2))));
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("the window pair legalizes on every native ABI");
        let instructions: Vec<_> = legal.plan().scalar_functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect();
        let extent = calling_conventions::ValueShape::integer(16, 8);
        assert!(instructions.iter().any(|row| row.operation == operation(2)
            && matches!(&row.kind,
                LegalizedScalarInstructionKind::StructuralLeafCopy { source, byte_offset: 8, shape, .. }
                    if *source == place(1) && *shape == extent)));
        assert!(instructions.iter().any(|row| row.operation == operation(3)
            && matches!(&row.kind,
                LegalizedScalarInstructionKind::StoreStructuralField { value, byte_offset: 8, shape, .. }
                    if *value == place(2) && *shape == extent)));
    }
}

/// Both ISAs select the reseat as the inverse chunked copy: loads from the
/// replacement's home at 0 and 8, stores into the root at 8 and 16.
#[test]
fn window_store_selects_the_inverse_chunked_copy() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("the window pair reaches selection");
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .expect("selection replay accepts both copy directions");
        let function = &selected.plan().functions[0];
        let offsets = |operation_id: OperationId, store: bool| -> Vec<u32> {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|row| row.provenance.operations == [operation_id])
                .filter_map(|row| match row.kind {
                    SelectedInstructionKind::Load64 { byte_offset } if !store => Some(byte_offset),
                    SelectedInstructionKind::Store { byte_offset, .. } if store => {
                        Some(byte_offset)
                    }
                    _ => None,
                })
                .collect()
        };
        assert_eq!(offsets(operation(2), false), [8, 16], "{native:?}");
        assert_eq!(offsets(operation(2), true), [0, 8], "{native:?}");
        assert_eq!(offsets(operation(3), false), [0, 8], "{native:?}");
        assert_eq!(offsets(operation(3), true), [8, 16], "{native:?}");
        let footprint = |operation_id: OperationId, role: SelectedMemoryAccessRole| {
            function
                .memory_accesses
                .iter()
                .filter(|access| {
                    access.origin
                        == selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            operation_id,
                        )
                        && access.role == role
                })
                .map(|access| (access.place, access.byte_offset))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            footprint(operation(3), SelectedMemoryAccessRole::ReadPlace),
            [(place(2), 0), (place(2), 8)]
        );
        assert_eq!(
            footprint(operation(3), SelectedMemoryAccessRole::WritePlace),
            [(place(1), 8), (place(1), 16)]
        );
    }
}

/// Legalization re-resolves the field extent and the value's producer, so a
/// retained row naming another offset, field, path, root or home rejects.
#[test]
fn window_rows_reject_extent_field_and_home_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    let displaced_home = target.functions[0].graph.blocks[0]
        .operations
        .iter()
        .find_map(|row| match row {
            TargetUnitOperation::StructuralLeafCopy {
                psi_operation,
                result_home,
                ..
            } if *psi_operation == operation(2) => Some(result_home.clone()),
            _ => None,
        })
        .unwrap();
    for mutation in 0..6 {
        let mut changed = target.clone();
        for row in &mut changed.functions[0].graph.blocks[0].operations {
            match (mutation, row) {
                (0, TargetUnitOperation::StoreStructuralField { byte_offset, .. }) => {
                    *byte_offset = 0
                }
                (1, TargetUnitOperation::StoreStructuralField { field, .. }) => {
                    *field = StructuralFieldId::new(1).unwrap()
                }
                (2, TargetUnitOperation::StoreStructuralField { value_home, .. }) => {
                    *value_home = displaced_home.clone()
                }
                (
                    3,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        path,
                        ..
                    },
                ) if *psi_operation == operation(2) => {
                    *path = vec![StructuralPathSegment::Field("tag".into())]
                }
                (
                    4,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        byte_offset,
                        ..
                    },
                ) if *psi_operation == operation(2) => *byte_offset = 0,
                (
                    5,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        source,
                        ..
                    },
                ) if *psi_operation == operation(2) => *source = place(9),
                _ => {}
            }
        }
        assert_ne!(changed, target, "mutation {mutation} changed a row");
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Only a mutable-borrowed root opens a window: target lowering refuses the
/// same pair over a shared borrow instead of writing through it.
#[test]
fn window_pair_over_a_shared_root_rejects_in_target_lowering() {
    let source = source(StructuralAccess::SharedBorrow);
    let error = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .expect_err("a shared root opens no window");
    assert!(
        matches!(
            error,
            abstract_operations_to_target_operations::LoweringError::UnsupportedControlFlow { site, .. }
                if site.file().ends_with("control_flow/borrowed_windows.rs")
        ),
        "{error:?}"
    );
}
