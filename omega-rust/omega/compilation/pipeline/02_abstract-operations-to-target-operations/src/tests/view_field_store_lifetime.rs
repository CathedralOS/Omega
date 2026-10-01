//! A stored literal view's native backing is a slot in its establishing
//! activation's frame, so lowering admits the descriptor replacement only
//! where that frame outlives every reader: the program entry's own
//! provisioned parameter, written without a `Referent` crossing. Translation
//! validation replays the same gate.
use super::super::{LoweringError, TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, BlockId, EdgeId, MachineId, NativeTarget, OperationId, PlaceId,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, TargetUnitOperation, identity,
};
use crate::AbstractToTargetTranslationValidationError;

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
            },
        ],
    );
}

#[test]
fn the_entry_activation_replaces_its_own_parameter_field() {
    let mut source = fixture();
    store(&mut source, 0, 1, Vec::new());
    for target in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
        let lowered =
            lower_to_target_operations(&source, TargetLoweringRequest::new(target)).unwrap();
        assert!(
            lowered.functions[0].graph.blocks[0]
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

#[test]
fn a_store_whose_root_may_outlive_the_literal_frame_fails_closed() {
    let target = NativeTarget::macos_arm64();
    let without_store =
        lower_to_target_operations(&fixture(), TargetLoweringRequest::new(target)).unwrap();
    let operation = OperationId::new(STORE).unwrap();
    for (function, destination, path) in [
        // A callee's parameter belongs to a root its caller keeps.
        (1, 1, Vec::new()),
        // A reference crossing reaches storage the entry does not own.
        (0, 1, vec![terminal_psi::StructuralPathSegment::Referent]),
        // Only a provisioned parameter is known to outlive the frame.
        (0, 3, Vec::new()),
    ] {
        let mut source = fixture();
        store(&mut source, function, destination, path);
        assert_eq!(
            lower_to_target_operations(&source, TargetLoweringRequest::new(target)),
            Err(LoweringError::UnsupportedViewFieldStoreLifetime(operation))
        );
        // A target graph that omits the store cannot turn independently
        // replayed translation into admission.
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, target, &without_store),
            Err(
                AbstractToTargetTranslationValidationError::UnsupportedViewFieldStoreLifetime(
                    operation
                )
            )
        );
    }
}
