//! An owned leaf copy of a readable root legalizes to a chunked storage copy.
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
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn i32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
}

fn i64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
}

fn field(id: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type,
    }
}

pub(crate) fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let inner = StructuralTypeId::new(10).unwrap();
    let outer = StructuralTypeId::new(20).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: inner,
            identity: "test::Inner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "first", StructuralFieldType::Scalar(i64_type())),
                    field(2, "second", StructuralFieldType::Scalar(i32_type())),
                ],
            },
        });
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: outer,
            identity: "test::Outer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "tag", StructuralFieldType::Scalar(i32_type())),
                    field(2, "inner", StructuralFieldType::Structural(inner)),
                ],
            },
        });
    source.functions[0]
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: outer,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::StructuralLeafCopy {
            psi_operation: OperationId::new(1).unwrap(),
            source: PlaceId::new(1).unwrap(),
            path: vec![StructuralPathSegment::Field("inner".into())],
            result: StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: PlaceId::new(2).unwrap(),
                structural_type: inner,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        },
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("a readable root admits a static leaf copy");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("leaf copy graph keeps canonical custody");
    (source, target, unit)
}

#[test]
fn leaf_copy_lowers_with_exact_projection_offset() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(native);
        let mut copies = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        result_home,
                        source,
                        path,
                        byte_offset,
                    } if *psi_operation == OperationId::new(1).unwrap()
                        && *source == PlaceId::new(1).unwrap()
                        && path == &[StructuralPathSegment::Field("inner".into())]
                        && *byte_offset == 8
                        && result_home.operation_result().is_some_and(|(operation, result)|
                            operation == OperationId::new(1).unwrap()
                                && result.place == PlaceId::new(2).unwrap()
                                && result.structural_type == StructuralTypeId::new(10).unwrap())
                )
            });
        assert_eq!(copies.clone().count(), 1, "one retained leaf copy row");
        assert!(copies.next().is_some());
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("leaf copy legalizes on every native ABI");
        let rows: Vec<_> = legal.plan().scalar_functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|row| {
                matches!(
                    &row.kind,
                    LegalizedScalarInstructionKind::StructuralLeafCopy {
                        source,
                        byte_offset,
                        shape,
                        ..
                    } if *source == PlaceId::new(1).unwrap()
                        && *byte_offset == 8
                        && *shape == calling_conventions::ValueShape::integer(16, 8)
                )
            })
            .collect();
        assert_eq!(rows.len(), 1, "one legalized leaf copy instruction");
    }
}

#[test]
fn leaf_copy_selects_chunked_loads_and_stores_into_fresh_storage() {
    let native = NativeTarget::linux_x64();
    let (source, target, unit) = fixture(native);
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
    let constraints = selection_constraints(&legal, &environment);
    let selected = select_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("leaf copy reaches target-op selection");
    validate_selected_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
        selected.plan().clone(),
    )
    .unwrap();
    let function = &selected.plan().functions[0];
    assert_eq!(function.local_storage_slots.len(), 1);
    assert_eq!(
        function.local_storage_slots[0].id,
        selected_instructions::LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: PlaceId::new(2).unwrap(),
        }
    );
    assert_eq!(function.local_storage_slots[0].byte_size, 16);
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .collect();
    let loads: Vec<u32> = instructions
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Load64 { byte_offset } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(loads, [8, 16]);
    let stores: Vec<u32> = instructions
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Store { byte_offset, .. } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(stores, [0, 8]);
    let reads: Vec<u32> = function
        .memory_accesses
        .iter()
        .filter_map(|access| {
            (access.place == PlaceId::new(1).unwrap()
                && access.role == SelectedMemoryAccessRole::ReadPlace)
                .then_some(access.byte_offset)
        })
        .collect();
    assert_eq!(reads, [8, 16]);
    let writes: Vec<u32> = function
        .memory_accesses
        .iter()
        .filter_map(|access| {
            (access.place == PlaceId::new(2).unwrap()
                && access.role == SelectedMemoryAccessRole::WritePlace)
                .then_some(access.byte_offset)
        })
        .collect();
    assert_eq!(writes, [0, 8]);
    assert!(
        function
            .virtual_registers
            .iter()
            .any(|register| matches!(register.origin,
                selected_instructions::VirtualRegisterOrigin::AbiTransport { place, .. }
                    if place == PlaceId::new(2).unwrap())),
        "the copied leaf publishes its result pointer"
    );
}

#[test]
fn leaf_copy_rejects_projection_and_home_substitution() {
    let native = NativeTarget::linux_x64();
    let (source, target, unit) = fixture(native);
    for mutation in 0..4 {
        let mut changed = target.clone();
        let body = &mut changed.functions[0].graph;
        let TargetUnitOperation::StructuralLeafCopy {
            source: actual_source,
            path,
            byte_offset,
            result_home,
            ..
        } = &mut body.blocks[0].operations[0]
        else {
            unreachable!()
        };
        match mutation {
            0 => *actual_source = PlaceId::new(9).unwrap(),
            1 => *path = vec![StructuralPathSegment::Field("tag".into())],
            2 => *byte_offset = 0,
            3 => {
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    result, ..
                } = &mut result_home.origin
                else {
                    unreachable!()
                };
                result.structural_type = StructuralTypeId::new(20).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}
