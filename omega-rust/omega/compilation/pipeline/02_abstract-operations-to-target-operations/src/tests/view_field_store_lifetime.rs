//! A stored literal view names immutable image storage, which outlives every
//! root, so lowering admits the descriptor replacement in any activation:
//! the program entry's own parameter and a callee's parameter alike.
use super::super::{TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, BlockId, EdgeId, MachineId, NativeTarget, OperationId, PlaceId,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, TargetUnitOperation, identity,
};

const STORE: u64 = 21;

fn shared_view() -> terminal_psi::ByteSequenceCarrier {
    terminal_psi::ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    }
}

/// Two machines, each taking an exclusive `Room { label: &[u8] }`; machine 1
/// is the program entry. Neither stores yet.
fn fixture() -> AbstractOperationPlan {
    let room = StructuralTypeId::new(1).unwrap();
    let function = |raw| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: room,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(raw).unwrap(),
            cleanup_actions: Vec::new(),
        }],
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: MachineId::new(1).unwrap(),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: room,
                identity: "Room".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "label".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(shared_view()),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: StructuralTypeId::new(2).unwrap(),
                identity: "bytes".into(),
                shape: StructuralTypeShape::ByteSequence(shared_view()),
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![function(1), function(2)],
    }
}

/// Establish `"Gate"` and store it into `destination` below `path`, ahead of
/// the selected function's return.
fn store(
    plan: &mut AbstractOperationPlan,
    function: usize,
    destination: u64,
    path: Vec<terminal_psi::StructuralPathSegment>,
) {
    let literal_type = plan.structural_types[1].clone();
    plan.functions[function].operations.splice(
        0..0,
        [
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: OperationId::new(20).unwrap(),
                place: terminal_psi::StructuralPlaceDeclaration {
                    id: PlaceId::new(2).unwrap(),
                    kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: 0,
                        structural_type: literal_type.id,
                    },
                },
                structural_type: literal_type,
                bytes: b"Gate".to_vec(),
                qualifications: Vec::new(),
            },
            AbstractOperation::StructuralByteViewFieldStore {
                psi_operation: OperationId::new(STORE).unwrap(),
                destination: PlaceId::new(destination).unwrap(),
                path,
                field: StructuralFieldId::new(1).unwrap(),
                source: PlaceId::new(2).unwrap(),
                predicate_obligation: None,
            },
        ],
    );
}

#[test]
fn every_activation_replaces_its_parameter_field_with_image_backed_literal() {
    // Function 0 is the program entry; function 1 is a callee whose
    // parameter belongs to a root its caller keeps.
    for function in [0, 1] {
        let mut source = fixture();
        store(&mut source, function, 1, Vec::new());
        for target in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
            let lowered =
                lower_to_target_operations(&source, TargetLoweringRequest::new(target)).unwrap();
            assert!(
                lowered.functions[function].graph.blocks[0]
                    .operations
                    .iter()
                    .any(|operation| matches!(
                        operation,
                        TargetUnitOperation::StructuralByteViewFieldStore { .. }
                    ))
            );
            crate::validate_abstract_to_target_translation(&source, target, &lowered).unwrap();
        }
    }
}

#[test]
fn indexed_view_fields_retain_selector_custody_and_mutable_access() {
    let source = super::structural_borrows::source_plan(
        r#"
        domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
        data Room { name: &[u8] in Utf8; }
        data Main { rooms: [Room; 3]; }
        machine Main::run(root: &mut Main) {
            let index: u64 = 1;
            let room_slice: &mut [Room] = root.rooms.as_mut_slice();
            room_slice[index].name = "expected";
            transition room_slice[index].name == "expected" {
                true -> finish(root)
                false -> fail(root)
            }
            state finish(root: &mut Main) { }
            state fail(root: &mut Main) { }
        }
    "#,
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
        let lowered = lower_to_target_operations(&source, TargetLoweringRequest::new(target))
            .expect("indexed fields use the borrowed collection backing");
        let fields = lowered
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::StructuralByteViewFieldStore {
                    destination,
                    indices,
                    ..
                } => Some((destination, indices)),
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    source, indices, ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldRead {
                    source, indices, ..
                } => Some((source, indices)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(fields.len() >= 3);
        for (source, indices) in fields {
            assert_eq!(source.access, StructuralAccess::MutableBorrow);
            let selectors = source
                .path
                .iter()
                .filter_map(|segment| segment.runtime_index())
                .collect::<Vec<_>>();
            assert_eq!(selectors.len(), indices.len());
            assert!(!indices.is_empty());
            for ((selector, _), retained) in selectors.iter().zip(indices) {
                assert_eq!(*selector, retained.operand.source_value());
                assert!(retained.stride > 0);
            }
        }
        crate::validate_abstract_to_target_translation(&source, target, &lowered).unwrap();
    }
    let mut shared = source.clone();
    for operation in shared
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.operations)
    {
        if let AbstractOperation::EstablishElementView { source, result, .. } = operation {
            source.access = StructuralAccess::SharedBorrow;
            result.multiplicity = StructuralMultiplicity::Unrestricted;
        }
    }
    assert!(
        lower_to_target_operations(
            &shared,
            TargetLoweringRequest::new(NativeTarget::macos_arm64())
        )
        .is_err(),
        "owning an immutable descriptor does not authorize field replacement"
    );
}
