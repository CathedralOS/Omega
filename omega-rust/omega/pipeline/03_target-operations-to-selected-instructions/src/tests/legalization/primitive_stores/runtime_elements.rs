//! Runtime-selected path elements at any depth lower as `(index, stride)`
//! runs for primitive stores, primitive reads and scalar field stores, and
//! every stage replays them against the verified certificates.
use super::{
    AbstractOperation, AbstractResult, BindingRelevance, FuelScheduleIdentity, IntegerSign,
    LegalizedScalarInstructionKind, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, TargetUnitOperation, ValueId,
};
use crate::selected_instructions::{SelectedInstructionKind, SelectedMemoryAccessRole};
use crate::tests::legalization::primitive_stores::integer;
use crate::tests::legalization::{accept_referenced_obligations, runtime_operand};
use crate::{legalize_target_operations, validate_legalized_operations};
use semantic_vocabulary::{EdgeId, ObligationId};
use terminal_psi::StructuralPathSegment as Segment;
use terminal_psi_to_abstract_operations::abstract_operations::{
    AbstractFunctionResult, AbstractParameter,
};

const STORE: u64 = 2;
const FIELD_STORE: u64 = 3;
const READ: u64 = 4;

fn operation(id: u64) -> OperationId {
    OperationId::new(id).unwrap()
}

fn value(id: u64) -> ValueId {
    ValueId::new(id).unwrap()
}

fn obligation(id: u64) -> ObligationId {
    ObligationId::new(id).unwrap()
}

fn runtime(index: u64, bound: u64) -> Segment {
    Segment::RuntimeIndex {
        index: value(index),
        obligation: obligation(bound),
    }
}

/// `root.grid[i][j] = v`, `root.ents[i].hp = v` and a read of
/// `root.grid[2][j]` beneath one mutable borrow: `grid` is `[[i32; 4]; 3]`
/// behind a 4-byte `padding`, `ents` is `[Entity; 2]` of `{tag: u8, hp: i32}`
/// records. `i` (value 10) is a `u64` selector and `j` (value 11) a `u32`
/// one, so the address model widens the second.
fn source() -> terminal_psi_to_abstract_operations::abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let i32_type = integer(IntegerSign::Signed, 32);
    let root = StructuralTypeId::new(1).unwrap();
    let element = StructuralTypeId::new(2).unwrap();
    let row = StructuralTypeId::new(3).unwrap();
    let grid = StructuralTypeId::new(4).unwrap();
    let entity = StructuralTypeId::new(5).unwrap();
    let entities = StructuralTypeId::new(6).unwrap();
    let field = |id, identity: &str, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let declare = |id, identity: &str, shape| StructuralTypeDeclaration {
        id,
        identity: identity.into(),
        shape,
    };
    source.structural_types = vec![
        declare(
            root,
            "Root",
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, "padding", StructuralFieldType::Scalar(i32_type)),
                    field(2, "grid", StructuralFieldType::Structural(grid)),
                    field(3, "ents", StructuralFieldType::Structural(entities)),
                ],
            },
        ),
        declare(
            element,
            "Element",
            StructuralTypeShape::PrimitiveScalar(i32_type),
        ),
        declare(
            row,
            "Row",
            StructuralTypeShape::FixedArray { element, length: 4 },
        ),
        declare(
            grid,
            "Grid",
            StructuralTypeShape::FixedArray {
                element: row,
                length: 3,
            },
        ),
        declare(
            entity,
            "Entity",
            StructuralTypeShape::Record {
                fields: vec![
                    field(
                        4,
                        "tag",
                        StructuralFieldType::Scalar(integer(IntegerSign::Unsigned, 8)),
                    ),
                    field(5, "hp", StructuralFieldType::Scalar(i32_type)),
                ],
            },
        ),
        declare(
            entities,
            "Entities",
            StructuralTypeShape::FixedArray {
                element: entity,
                length: 2,
            },
        ),
    ]
    .into();
    let destination = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let stored = AbstractResult {
        value: value(12),
        scalar_type: i32_type,
    };
    let read = AbstractResult {
        value: value(13),
        scalar_type: i32_type,
    };
    let result = AbstractResult {
        value: value(14),
        scalar_type: i32_type,
    };
    let function = &mut source.functions[0];
    function.structural_parameters = vec![destination.clone()];
    function.parameters = vec![
        AbstractParameter {
            value: value(10),
            scalar_type: integer(IntegerSign::Unsigned, 64),
        },
        AbstractParameter {
            value: value(11),
            scalar_type: integer(IntegerSign::Unsigned, 32),
        },
        AbstractParameter {
            value: stored.value,
            scalar_type: i32_type,
        },
    ];
    function.result = AbstractFunctionResult::Scalar(result);
    function.operations = vec![
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation: operation(STORE),
            destination: destination.clone(),
            path: vec![
                Segment::Field("grid".into()),
                runtime(10, 1),
                runtime(11, 2),
            ],
            value: stored,
        },
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: operation(FIELD_STORE),
            destination: destination.clone(),
            path: vec![Segment::Field("ents".into()), runtime(10, 3)],
            field: StructuralFieldId::new(5).unwrap(),
            value: stored,
            range_obligation: None,
        },
        AbstractOperation::PrimitiveScalarRead {
            psi_operation: operation(READ),
            result: read,
            source: destination.place,
            path: vec![
                Segment::Field("grid".into()),
                Segment::FixedIndex(2),
                runtime(11, 4),
            ],
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: result.value,
            value: read.value,
            scalar_type: i32_type,
            cleanup_actions: Vec::new(),
        },
    ];
    source
}

fn store_indices(
    target: &abstract_operations_to_target_operations::target_operations::TargetOperationPlan,
) -> Vec<(ValueId, u32)> {
    target.functions[0]
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation {
            TargetUnitOperation::WriteOnlyPrimitiveStore { indices, .. } => Some(
                indices
                    .iter()
                    .map(|index| (index.operand.source_value(), index.stride))
                    .collect(),
            ),
            _ => None,
        })
        .expect("one runtime-projected store")
}

#[test]
fn runtime_elements_scale_reads_stores_and_field_stores_through_every_replay() {
    let u64_type = integer(IntegerSign::Unsigned, 64);
    let u32_type = integer(IntegerSign::Unsigned, 32);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let source = source();
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap_or_else(|error| panic!("{native:?} lowers runtime elements: {error:?}"));
        // Rows are 16 bytes and elements 4; the selectors keep path order.
        assert_eq!(store_indices(&target), [(value(10), 16), (value(11), 4)]);
        let field_store = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarFieldStore {
                    field_byte_offset,
                    indices,
                    ..
                } => Some((*field_byte_offset, indices.clone())),
                _ => None,
            })
            .expect("one runtime-carrier field store");
        // `ents` follows 4 bytes of padding and 48 bytes of grid; `hp`
        // follows the one-byte tag at offset 4 of an 8-byte entity.
        assert_eq!(field_store.0, 4 + 48 + 4);
        assert_eq!(
            field_store
                .1
                .iter()
                .map(|index| (index.operand.source_value(), index.stride))
                .collect::<Vec<_>>(),
            [(value(10), 8)]
        );

        let seed = terminal_psi_to_abstract_operations::optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        terminal_psi_to_abstract_operations::optimization_unit_semantics::validate_psi_optimization_unit(&seed)
            .expect("runtime-element graph keeps canonical custody");
        // A bound reaches legalization only as the verifier's accepted
        // certificate; without one the element has no evidence.
        assert!(
            legalize_target_operations(&target, &source, &seed).is_err(),
            "{native:?} runtime elements without certificates"
        );
        let unit = accept_referenced_obligations(seed);
        let legalized = legalize_target_operations(&target, &source, &unit)
            .unwrap_or_else(|error| panic!("{native:?} legalizes runtime elements: {error:?}"));
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
            .unwrap_or_else(|error| panic!("{native:?} replays runtime elements: {error:?}"));
        let rows = &legalized.plan().scalar_functions[0].blocks[0].instructions;
        let operand = |id, scalar_type| AbstractResult {
            value: value(id),
            scalar_type,
        };
        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            byte_offset,
            indices,
            ..
        } = &rows[0].kind
        else {
            panic!("store row")
        };
        assert_eq!(*byte_offset, 4, "the static part is the grid field");
        assert_eq!(
            *indices,
            [
                runtime_operand(
                    &unit,
                    operation(STORE),
                    operand(10, u64_type),
                    16,
                    3,
                    obligation(1)
                ),
                runtime_operand(
                    &unit,
                    operation(STORE),
                    operand(11, u32_type),
                    4,
                    4,
                    obligation(2)
                ),
            ]
        );
        let LegalizedScalarInstructionKind::PrimitiveScalarRead { indices, .. } = &rows[2].kind
        else {
            panic!("read row")
        };
        assert_eq!(
            *indices,
            [runtime_operand(
                &unit,
                operation(READ),
                operand(11, u32_type),
                4,
                4,
                obligation(4)
            )]
        );

        // Independent replay rejects a substituted stride, extent, selector
        // or certificate on any runtime element.
        for mutation in 0..4 {
            let mut changed = legalized.plan().clone();
            let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { indices, .. } =
                &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
            else {
                panic!("store row")
            };
            match mutation {
                0 => indices[1].stride = 8,
                1 => indices[0].extent = 4,
                2 => indices.swap(0, 1),
                _ => indices[0].obligation = obligation(2),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "{native:?} replay mutation {mutation}"
            );
        }
        // The target row must name the path's own selectors.
        let mut changed = target.clone();
        let TargetUnitOperation::WriteOnlyPrimitiveStore { indices, .. } =
            changed.functions[0].graph.blocks[0]
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
                    )
                })
                .unwrap()
        else {
            unreachable!()
        };
        indices.reverse();
        assert!(legalize_target_operations(&changed, &source, &unit).is_err());

        let environment =
            crate::register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("{native:?} selects runtime elements: {error:?}"));
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap_or_else(|error| panic!("{native:?} validates runtime elements: {error:?}"));
        let function = &selected.plan().functions[0];
        // Each runtime element joins the address once, carrying its own
        // obligation: two for the store, one for the field store, one for
        // the read.
        let joins = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|row| matches!(row.kind, SelectedInstructionKind::ByteViewAddress))
            .map(|row| row.provenance.obligations.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            joins,
            [
                vec![obligation(1)],
                vec![obligation(2)],
                vec![obligation(3)],
                vec![obligation(4)],
            ]
        );
        let footprint = |psi| {
            function
                .memory_accesses
                .iter()
                .filter(|access| {
                    access.origin
                        == crate::selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            operation(psi),
                        )
                })
                .map(|access| (access.byte_offset, access.byte_count, access.role))
                .collect::<Vec<_>>()
        };
        let store = footprint(STORE);
        assert_eq!(store.len(), 2, "one footprint row per runtime element");
        assert!(store.iter().all(|(offset, count, role)| *offset == 4
            && *count == 4
            && matches!(role, SelectedMemoryAccessRole::WriteIndexedPrimitive { value: stored, .. }
                if *stored == value(12))));
        assert!(matches!(
            footprint(FIELD_STORE).as_slice(),
            [(
                56,
                4,
                SelectedMemoryAccessRole::WriteIndexedPrimitive {
                    stride: 8,
                    extent: 2,
                    ..
                }
            )]
        ));
        assert!(matches!(
            footprint(READ).as_slice(),
            [(
                36,
                4,
                SelectedMemoryAccessRole::ReadIndexedPrimitive {
                    stride: 4,
                    extent: 4,
                    ..
                }
            )]
        ));
        // Selection replay rejects a dropped or re-attributed element row.
        for mutation in 0..2 {
            let mut changed = selected.plan().clone();
            let rows = &mut changed.functions[0].memory_accesses;
            let position = rows
                .iter()
                .position(|access| {
                    matches!(
                        access.role,
                        SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
                    )
                })
                .unwrap();
            match mutation {
                0 => {
                    rows.remove(position);
                }
                _ => {
                    if let SelectedMemoryAccessRole::WriteIndexedPrimitive {
                        obligation: bound,
                        ..
                    } = &mut rows[position].role
                    {
                        *bound = obligation(4);
                    }
                }
            }
            assert!(
                crate::validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed,
                )
                .is_err(),
                "{native:?} selection mutation {mutation}"
            );
        }
    }
}

#[test]
fn runtime_elements_reject_non_integer_selectors_before_target_lowering() {
    let mut source = source();
    source.functions[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(
                NativeTarget::linux_x64()
            ),
        )
        .is_err(),
        "a Boolean cannot select an element"
    );
}
