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
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
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

fn case(
    id: u64,
    identity: &str,
    fields: Vec<StructuralFieldDeclaration>,
) -> terminal_psi::StructuralCaseDeclaration {
    terminal_psi::StructuralCaseDeclaration {
        id: semantic_vocabulary::StructuralCaseId::new(id).unwrap(),
        identity: identity.into(),
        fields,
    }
}

fn declare(id: u64, identity: &str, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: identity.into(),
        shape,
    }
}

fn inner_type() -> StructuralTypeDeclaration {
    declare(
        10,
        "test::Inner",
        StructuralTypeShape::Record {
            fields: vec![
                field(1, "first", StructuralFieldType::Scalar(i64_type())),
                field(2, "second", StructuralFieldType::Scalar(i32_type())),
            ],
        },
    )
}

fn outer_type() -> StructuralTypeDeclaration {
    declare(
        20,
        "test::Outer",
        StructuralTypeShape::Record {
            fields: vec![
                field(1, "tag", StructuralFieldType::Scalar(i32_type())),
                field(
                    2,
                    "inner",
                    StructuralFieldType::Structural(StructuralTypeId::new(10).unwrap()),
                ),
            ],
        },
    )
}

fn scalar_i32_type() -> StructuralTypeDeclaration {
    declare(30, "i32", StructuralTypeShape::PrimitiveScalar(i32_type()))
}

fn sum_type() -> StructuralTypeDeclaration {
    declare(
        40,
        "test::Sum",
        StructuralTypeShape::Sum {
            cases: vec![
                case(
                    1,
                    "Alpha",
                    vec![field(1, "wide", StructuralFieldType::Scalar(i64_type()))],
                ),
                case(
                    2,
                    "Beta",
                    vec![
                        field(
                            1,
                            "inner",
                            StructuralFieldType::Structural(StructuralTypeId::new(10).unwrap()),
                        ),
                        field(2, "code", StructuralFieldType::Scalar(i32_type())),
                    ],
                ),
            ],
        },
    )
}

fn mixed_type() -> StructuralTypeDeclaration {
    declare(
        50,
        "test::Mixed",
        StructuralTypeShape::Mixed {
            fields: vec![field(1, "common", StructuralFieldType::Scalar(i32_type()))],
            cases: vec![case(
                2,
                "Ready",
                vec![field(1, "wide", StructuralFieldType::Scalar(i64_type()))],
            )],
        },
    )
}

fn array_type() -> StructuralTypeDeclaration {
    declare(
        60,
        "test::Array",
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(10).unwrap(),
            length: 2,
        },
    )
}

fn inner_array_type() -> StructuralTypeDeclaration {
    declare(
        61,
        "test::InnerArray",
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(10).unwrap(),
            length: 3,
        },
    )
}

fn nested_array_type() -> StructuralTypeDeclaration {
    declare(
        62,
        "test::NestedArray",
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(61).unwrap(),
            length: 2,
        },
    )
}

fn sequence_type() -> StructuralTypeDeclaration {
    declare(
        70,
        "test::Sequence",
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
    )
}

fn view_type() -> StructuralTypeDeclaration {
    declare(
        80,
        "test::View",
        StructuralTypeShape::ElementView {
            element: StructuralTypeId::new(30).unwrap(),
        },
    )
}

fn scalar_i64_type() -> StructuralTypeDeclaration {
    declare(31, "i64", StructuralTypeShape::PrimitiveScalar(i64_type()))
}

fn reference_holder_type() -> StructuralTypeDeclaration {
    declare(
        91,
        "test::ReferenceHolder",
        StructuralTypeShape::Record {
            fields: vec![field(
                1,
                "loan",
                StructuralFieldType::Structural(StructuralTypeId::new(92).unwrap()),
            )],
        },
    )
}

fn reference_type() -> StructuralTypeDeclaration {
    declare(
        92,
        "test::Loan",
        StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(31).unwrap(),
            access: StructuralAccess::MutableBorrow,
        },
    )
}

fn catalog() -> Vec<StructuralTypeDeclaration> {
    vec![
        inner_type(),
        outer_type(),
        scalar_i32_type(),
        sum_type(),
        mixed_type(),
        array_type(),
        inner_array_type(),
        nested_array_type(),
        sequence_type(),
        view_type(),
        scalar_i64_type(),
        reference_holder_type(),
        reference_type(),
    ]
}

/// A leaf copy whose parameter type, path, and result type vary per endpoint
/// kind. `param` must be a record when `path` projects through fields; an
/// empty `path` copies the whole root.
pub(crate) fn endpoint_fixture(
    native: NativeTarget,
    param: StructuralTypeDeclaration,
    path: Vec<StructuralPathSegment>,
    result_type: StructuralTypeId,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let source = leaf_copy_source(param, path, result_type);
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

/// The source half of `endpoint_fixture`, before lowering.
fn leaf_copy_source(
    param: StructuralTypeDeclaration,
    path: Vec<StructuralPathSegment>,
    result_type: StructuralTypeId,
) -> abstract_operations::AbstractOperationPlan {
    leaf_copy_source_access(param, path, result_type, StructuralAccess::SharedBorrow)
}

/// `leaf_copy_source` with the parameter's access kind chosen by the caller.
fn leaf_copy_source_access(
    param: StructuralTypeDeclaration,
    path: Vec<StructuralPathSegment>,
    result_type: StructuralTypeId,
    access: StructuralAccess,
) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let mut declarations = catalog();
    declarations.retain(|declaration| declaration.id != param.id);
    declarations.push(param.clone());
    declarations.sort_by_key(|declaration| declaration.id);
    for declaration in declarations {
        source.structural_types.make_mut().push(declaration);
    }
    source.functions[0]
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: param.id,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::StructuralLeafCopy {
            psi_operation: OperationId::new(1).unwrap(),
            source: PlaceId::new(1).unwrap(),
            path,
            result: StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: PlaceId::new(2).unwrap(),
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        },
    );
    source
}

/// A record host whose `leaf` field lands at byte offset 8 behind a `pad`
/// scalar, so every endpoint kind shares one expected offset.
fn host(leaf_field_type: StructuralFieldType) -> StructuralTypeDeclaration {
    declare(
        90,
        "test::Host",
        StructuralTypeShape::Record {
            fields: vec![
                field(1, "pad", StructuralFieldType::Scalar(i32_type())),
                field(2, "leaf", leaf_field_type),
            ],
        },
    )
}

/// The retained leaf copy's `(byte_offset, shape)` in both target operations
/// and the legalized scalar graph, asserting the stages agree.
fn leaf_copy_rows(
    target: &target_operations::TargetOperationPlan,
    legal: &crate::ValidatedLegalizedOperations,
) -> (u32, calling_conventions::ValueShape) {
    let copies: Vec<_> = target.functions[0]
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation, TargetUnitOperation::StructuralLeafCopy { .. }))
        .collect();
    assert_eq!(copies.len(), 1, "one retained leaf copy row");
    let TargetUnitOperation::StructuralLeafCopy { byte_offset, .. } = copies[0] else {
        unreachable!()
    };
    let byte_offset = *byte_offset;
    let rows: Vec<_> = legal.plan().scalar_functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
            )
        })
        .collect();
    assert_eq!(rows.len(), 1, "one legalized leaf copy instruction");
    let LegalizedScalarInstructionKind::StructuralLeafCopy {
        byte_offset: legalized_offset,
        shape,
        ..
    } = rows[0].kind
    else {
        unreachable!()
    };
    assert_eq!(legalized_offset, byte_offset);
    (byte_offset, shape)
}

/// A leaf copy projected through a `leaf` structural field in `host` is the
/// shared endpoint harness: lowering keeps the projection offset and
/// legalization pairs it with the canonical leaf shape.
fn field_leaf(
    leaf_type: StructuralTypeDeclaration,
    result_type: StructuralTypeId,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    endpoint_fixture(
        NativeTarget::linux_x64(),
        host(structural_field(leaf_type)),
        vec![StructuralPathSegment::Field("leaf".into())],
        result_type,
    )
}

fn structural_field(ty: StructuralTypeDeclaration) -> StructuralFieldType {
    StructuralFieldType::Structural(ty.id)
}

pub(crate) fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    endpoint_fixture(
        native,
        outer_type(),
        vec![StructuralPathSegment::Field("inner".into())],
        StructuralTypeId::new(10).unwrap(),
    )
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
                        indices,
                    } if *psi_operation == OperationId::new(1).unwrap()
                        && *source == PlaceId::new(1).unwrap()
                        && path == &[StructuralPathSegment::Field("inner".into())]
                        && *byte_offset == 8
                        && indices.is_empty()
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

#[test]
fn leaf_copy_scalar_field_endpoint_lowers_and_legalizes() {
    // An inline scalar leaf resolves through its canonical PrimitiveScalar
    // declaration: the host's only field is the leaf at offset 0.
    let (source, target, unit) = endpoint_fixture(
        NativeTarget::linux_x64(),
        declare(
            90,
            "test::ScalarHost",
            StructuralTypeShape::Record {
                fields: vec![field(1, "leaf", StructuralFieldType::Scalar(i32_type()))],
            },
        ),
        vec![StructuralPathSegment::Field("leaf".into())],
        StructuralTypeId::new(30).unwrap(),
    );
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("a scalar leaf endpoint legalizes");
    assert_eq!(
        leaf_copy_rows(&target, &legal),
        (0, calling_conventions::ValueShape::integer(4, 4))
    );
}

#[test]
fn leaf_copy_sum_mixed_and_view_endpoints_lower_and_legalize() {
    // Sum (structural payload), Mixed, ByteSequence view, and ElementView
    // endpoints all resolve through one `Structural` field at offset 8.
    for (endpoint, result_type) in [
        (sum_type(), StructuralTypeId::new(40).unwrap()),
        (mixed_type(), StructuralTypeId::new(50).unwrap()),
        (sequence_type(), StructuralTypeId::new(70).unwrap()),
        (view_type(), StructuralTypeId::new(80).unwrap()),
    ] {
        let (source, target, unit) = field_leaf(endpoint, result_type);
        let legal =
            legalize_target_operations(&target, &source, &unit).expect("endpoint kind legalizes");
        let (offset, shape) = leaf_copy_rows(&target, &legal);
        assert_eq!(offset, 8, "every endpoint field sits behind pad");
        assert!(shape.byte_size > 0, "endpoint carries a real extent");
    }
}

#[test]
fn leaf_copy_fixed_index_traversal_resolves_the_element() {
    // `leaf.elements[1]` walks a FixedArray: offset = leaf(8) + stride(16).
    let (source, target, unit) = endpoint_fixture(
        NativeTarget::linux_x64(),
        host(structural_field(array_type())),
        vec![
            StructuralPathSegment::Field("leaf".into()),
            StructuralPathSegment::FixedIndex(1),
        ],
        StructuralTypeId::new(10).unwrap(),
    );
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("a fixed array element endpoint legalizes");
    assert_eq!(
        leaf_copy_rows(&target, &legal),
        (24, calling_conventions::ValueShape::integer(16, 8))
    );
}

#[test]
fn leaf_copy_whole_root_copy_lowers_and_legalizes() {
    // An empty path copies the entire readable root; the parameter type is
    // the endpoint directly.
    let (source, target, unit) = endpoint_fixture(
        NativeTarget::linux_x64(),
        sum_type(),
        Vec::new(),
        StructuralTypeId::new(40).unwrap(),
    );
    let legal =
        legalize_target_operations(&target, &source, &unit).expect("a whole-root copy legalizes");
    let (offset, shape) = leaf_copy_rows(&target, &legal);
    assert_eq!(offset, 0);
    assert!(shape.byte_size > 0);
}

#[test]
fn leaf_copy_rejects_reference_carrying_endpoint() {
    // A record leaf containing a reference carrier would copy a loan handle,
    // not owned storage: borrowed roots carrying references are rejected by
    // the lowering boundary before the endpoint containment walk runs.
    let source = leaf_copy_source(
        host(StructuralFieldType::Structural(
            StructuralTypeId::new(91).unwrap(),
        )),
        vec![StructuralPathSegment::Field("leaf".to_string())],
        StructuralTypeId::new(91).unwrap(),
    );
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
}

#[test]
fn leaf_copy_selects_fixed_array_copy() {
    // A 32-byte FixedArray endpoint realizes as four chunked load/store
    // pairs into its fresh structural slot.
    let (source, target, unit) = field_leaf(array_type(), StructuralTypeId::new(60).unwrap());
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let environment =
        register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
            .unwrap();
    let constraints = selection_constraints(&legal, &environment);
    let selected = select_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("a fixed array leaf copy reaches selection");
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
    assert_eq!(function.local_storage_slots[0].byte_size, 32);
    let loads: Vec<u32> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Load64 { byte_offset } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(loads, [8, 16, 24, 32]);
    let stores: Vec<u32> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Store { byte_offset, .. } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(stores, [0, 8, 16, 24]);
}

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

/// A leaf copy through one `RuntimeIndex` segment: the host's `leaf` field is
/// a fixed array of records and `selector` 0 names a `u64` index parameter.
fn runtime_index_source(
    selector: u32,
    maximum: u128,
    index_scalar: ScalarType,
) -> abstract_operations::AbstractOperationPlan {
    let mut source = leaf_copy_source(
        host(structural_field(array_type())),
        vec![
            StructuralPathSegment::Field("leaf".into()),
            StructuralPathSegment::RuntimeIndex {
                selector,
                minimum: IntegerValue::Unsigned(0),
                maximum: IntegerValue::Unsigned(maximum),
            },
        ],
        StructuralTypeId::new(10).unwrap(),
    );
    source.functions[0]
        .parameters
        .push(abstract_operations::AbstractParameter {
            value: ValueId::new(9).unwrap(),
            scalar_type: index_scalar,
        });
    source
}

fn runtime_index_fixture() -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let source = runtime_index_source(0, 1, u64_type());
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .expect("a bounded runtime index admits a leaf copy");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("runtime-index leaf copy graph keeps canonical custody");
    (source, target, unit)
}

#[test]
fn leaf_copy_runtime_index_copies_through_dynamic_address() {
    let native = NativeTarget::linux_x64();
    let (source, target, unit) = runtime_index_fixture();
    // The retained target row carries the static field offset plus the
    // scaled runtime index, binding the selector's parameter verbatim.
    let copies: Vec<_> = target.functions[0]
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation, TargetUnitOperation::StructuralLeafCopy { .. }))
        .collect();
    assert_eq!(copies.len(), 1, "one retained leaf copy row");
    let TargetUnitOperation::StructuralLeafCopy {
        byte_offset,
        indices,
        ..
    } = copies[0]
    else {
        unreachable!()
    };
    assert_eq!(*byte_offset, 8, "the arr field offset stays static");
    let [index] = indices.as_slice() else {
        panic!("one runtime index traversal is retained")
    };
    assert_eq!(index.stride, 16, "the record element stride");
    assert_eq!(
        index.operand,
        target_operations::TargetUnitScalarArgumentSource::Parameter {
            parameter_index: 0,
            source_value: ValueId::new(9).unwrap(),
            scalar_type: u64_type(),
        }
    );
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("runtime-index leaf copy legalizes");
    let rows: Vec<_> = legal.plan().scalar_functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
            )
        })
        .collect();
    assert_eq!(rows.len(), 1, "one legalized leaf copy instruction");
    let LegalizedScalarInstructionKind::StructuralLeafCopy {
        byte_offset,
        shape,
        indices,
        ..
    } = &rows[0].kind
    else {
        unreachable!()
    };
    assert_eq!(*byte_offset, 8);
    assert_eq!(*shape, calling_conventions::ValueShape::integer(16, 8));
    assert_eq!(
        indices.as_slice(),
        &[legalized_operations::LegalizedRuntimeIndexOperand {
            operand: abstract_operations::AbstractResult {
                value: ValueId::new(9).unwrap(),
                scalar_type: u64_type(),
            },
            stride: 16,
        }]
    );
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
    let constraints = selection_constraints(&legal, &environment);
    let selected = select_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("runtime-index leaf copy reaches selection");
    // The independent validation replay must agree with the emitted stream.
    validate_selected_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
        selected.plan().clone(),
    )
    .unwrap();
    let function = &selected.plan().functions[0];
    let ops: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .collect();
    assert!(
        ops.iter().any(|row| matches!(
            row.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(16)
            }
        )),
        "the element stride materializes"
    );
    assert!(
        ops.iter()
            .any(|row| matches!(row.kind, SelectedInstructionKind::WrappingMultiplyI64)),
        "the runtime index scales by the stride"
    );
    assert!(
        ops.iter()
            .any(|row| matches!(row.kind, SelectedInstructionKind::ByteViewAddress)),
        "the scaled index joins the source pointer"
    );
    let loads: Vec<u32> = ops
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Load64 { byte_offset } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(loads, [8, 16], "static field offset + chunk stride");
    let stores: Vec<u32> = ops
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Store { byte_offset, .. } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(stores, [0, 8]);
}

#[test]
fn leaf_copy_runtime_index_rejects_over_extent_maximum() {
    // The declared maximum must stay inside the array extent — exactly the
    // terminal verifier's bound replay.
    let source = runtime_index_source(0, 5, u64_type());
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
}

#[test]
fn leaf_copy_runtime_index_requires_a_u64_parameter() {
    // A narrower index scalar cannot feed the i64 address math honestly.
    let source = runtime_index_source(0, 1, i32_type());
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
    // A selector that names no parameter slot cannot resolve an operand.
    let source = runtime_index_source(3, 1, u64_type());
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
}

/// A leaf copy through two `RuntimeIndex` segments: the host's `leaf` field
/// is a fixed array of fixed arrays of records and `selector`s 0 and 1 name
/// the two `u64` index parameters.
fn nested_runtime_index_source(
    selectors: (u32, u32),
    maxima: (u128, u128),
    index_scalar: ScalarType,
) -> abstract_operations::AbstractOperationPlan {
    let mut source = leaf_copy_source(
        host(structural_field(nested_array_type())),
        vec![
            StructuralPathSegment::Field("leaf".into()),
            StructuralPathSegment::RuntimeIndex {
                selector: selectors.0,
                minimum: IntegerValue::Unsigned(0),
                maximum: IntegerValue::Unsigned(maxima.0),
            },
            StructuralPathSegment::RuntimeIndex {
                selector: selectors.1,
                minimum: IntegerValue::Unsigned(0),
                maximum: IntegerValue::Unsigned(maxima.1),
            },
        ],
        StructuralTypeId::new(10).unwrap(),
    );
    source.functions[0].parameters.extend([
        abstract_operations::AbstractParameter {
            value: ValueId::new(9).unwrap(),
            scalar_type: index_scalar,
        },
        abstract_operations::AbstractParameter {
            value: ValueId::new(10).unwrap(),
            scalar_type: index_scalar,
        },
    ]);
    source
}

#[test]
fn leaf_copy_nested_runtime_indices_copy_through_chained_addresses() {
    let native = NativeTarget::linux_x64();
    let source = nested_runtime_index_source((0, 1), (1, 2), u64_type());
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("bounded runtime indices admit a leaf copy");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("runtime-index leaf copy graph keeps canonical custody");
    let copies: Vec<_> = target.functions[0]
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation, TargetUnitOperation::StructuralLeafCopy { .. }))
        .collect();
    assert_eq!(copies.len(), 1, "one retained leaf copy row");
    let TargetUnitOperation::StructuralLeafCopy {
        byte_offset,
        indices,
        ..
    } = copies[0]
    else {
        unreachable!()
    };
    assert_eq!(
        *byte_offset, 8,
        "every static segment folds into the offset"
    );
    assert_eq!(
        indices.as_slice(),
        &[
            target_operations::TargetStructuralRuntimeIndex {
                operand: target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value: ValueId::new(9).unwrap(),
                    scalar_type: u64_type(),
                },
                stride: 48,
            },
            target_operations::TargetStructuralRuntimeIndex {
                operand: target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 1,
                    source_value: ValueId::new(10).unwrap(),
                    scalar_type: u64_type(),
                },
                stride: 16,
            },
        ],
        "outer stride spans a record array, inner stride spans one record",
    );
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("nested runtime-index leaf copy legalizes");
    let rows: Vec<_> = legal.plan().scalar_functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
            )
        })
        .collect();
    assert_eq!(rows.len(), 1, "one legalized leaf copy instruction");
    let LegalizedScalarInstructionKind::StructuralLeafCopy {
        byte_offset,
        shape,
        indices,
        ..
    } = &rows[0].kind
    else {
        unreachable!()
    };
    assert_eq!(*byte_offset, 8);
    assert_eq!(*shape, calling_conventions::ValueShape::integer(16, 8));
    assert_eq!(
        indices.as_slice(),
        &[
            legalized_operations::LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: ValueId::new(9).unwrap(),
                    scalar_type: u64_type(),
                },
                stride: 48,
            },
            legalized_operations::LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: ValueId::new(10).unwrap(),
                    scalar_type: u64_type(),
                },
                stride: 16,
            },
        ]
    );
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
    let constraints = selection_constraints(&legal, &environment);
    let selected = select_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("nested runtime-index leaf copy reaches selection");
    validate_selected_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
        selected.plan().clone(),
    )
    .unwrap();
    let function = &selected.plan().functions[0];
    let ops: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .collect();
    for stride in [48u128, 16] {
        assert!(
            ops.iter().any(|row| matches!(
                row.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(value)
                } if value == stride
            )),
            "each element stride materializes",
        );
    }
    assert_eq!(
        ops.iter()
            .filter(|row| matches!(row.kind, SelectedInstructionKind::WrappingMultiplyI64))
            .count(),
        2,
        "each runtime index scales by its stride"
    );
    assert_eq!(
        ops.iter()
            .filter(|row| matches!(row.kind, SelectedInstructionKind::ByteViewAddress))
            .count(),
        2,
        "each scaled index joins the accumulating pointer"
    );
    let loads: Vec<u32> = ops
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Load64 { byte_offset } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(loads, [8, 16], "static field offset + chunk stride");
    let stores: Vec<u32> = ops
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Store { byte_offset, .. } => Some(byte_offset),
            _ => None,
        })
        .collect();
    assert_eq!(stores, [0, 8]);
}

#[test]
fn leaf_copy_nested_runtime_index_rejects_an_over_extent_maximum() {
    // Each segment replays the verifier's bound against its own array extent:
    // the outer array admits at most 1, the inner at most 2.
    for maxima in [(2, 2), (1, 3)] {
        let source = nested_runtime_index_source((0, 1), maxima, u64_type());
        assert!(
            abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                TargetLoweringRequest::new(NativeTarget::linux_x64()),
            )
            .is_err(),
            "maxima {maxima:?} must reject",
        );
    }
}

#[test]
fn leaf_copy_nested_runtime_index_requires_u64_parameters() {
    let source = nested_runtime_index_source((0, 1), (1, 2), i32_type());
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
    // A selector that names no parameter slot cannot resolve an operand.
    let source = nested_runtime_index_source((0, 5), (1, 2), u64_type());
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(NativeTarget::linux_x64()),
        )
        .is_err()
    );
}

#[test]
fn leaf_copy_fragment_backed_owned_param_selects_and_validates() {
    // An owned ElementView param keeps its descriptor in ABI fragments (the
    // view type earns no pointer home), so the whole-root copy stores each
    // fragment register into the result slot without a single load.
    let native = NativeTarget::linux_x64();
    let mut source = leaf_copy_source_access(
        view_type(),
        vec![],
        StructuralTypeId::new(80).unwrap(),
        StructuralAccess::Owned,
    );
    source.functions[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    if let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut source.functions[0].operations[1]
    {
        cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
            PlaceId::new(1).unwrap(),
        ));
    }
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("an owned fragment-backed root admits a whole-root leaf copy");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let legal = legalize_target_operations(&target, &source, &unit)
        .expect("leaf copy from a fragment-backed owned param legalizes");
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
    let constraints = selection_constraints(&legal, &environment);
    let selected = select_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("leaf copy from fragment storage reaches selection");
    validate_selected_instructions(
        &legal,
        &constraints,
        environment.physical(),
        environment.constraints(),
        selected.plan().clone(),
    )
    .expect("independent replay accepts the fragment-backed copy");
    let function = &selected.plan().functions[0];
    // The owned param earns no structural-parameter home slot; the result
    // slot is the only structural storage the copy materializes.
    assert!(
        !function
            .local_storage_slots
            .iter()
            .any(|slot| matches!(slot.id,
                selected_instructions::LocalStorageSlotId::StructuralParameter { place }
                    if place == PlaceId::new(1).unwrap())),
        "no StructuralParameter home for the fragment-backed param"
    );
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.provenance.operations == [OperationId::new(1).unwrap()])
        .collect();
    assert!(
        instructions.iter().all(|row| !matches!(
            row.kind,
            SelectedInstructionKind::Load8 { .. }
                | SelectedInstructionKind::Load16 { .. }
                | SelectedInstructionKind::Load32 { .. }
                | SelectedInstructionKind::Load64 { .. }
        )),
        "fragment registers store directly, without loads"
    );
    let mut stores: Vec<u32> = instructions
        .iter()
        .filter_map(|row| match row.kind {
            SelectedInstructionKind::Store { byte_offset, .. } => Some(byte_offset),
            _ => None,
        })
        .collect();
    stores.sort_unstable();
    assert_eq!(stores, [0, 8], "two 8-byte fragment stores fill the leaf");
    let mut writes: Vec<u32> = function
        .memory_accesses
        .iter()
        .filter_map(|access| {
            (access.place == PlaceId::new(2).unwrap()
                && access.role == SelectedMemoryAccessRole::WritePlace)
                .then_some(access.byte_offset)
        })
        .collect();
    writes.sort_unstable();
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
