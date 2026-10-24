//! Replacing a structural member vacates and restores a subtree. A subsequent
//! whole transfer must withdraw its old cleanup without authorizing new deaths.
use super::complete;
use language_core::BindingRelevance;
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, OperationId, PlaceId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralRecordStorage, StructuralStoreDisposition,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalIdentityIndex, Terminator,
};

fn operation(number: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(number).unwrap(),
        result,
        kind,
    }
}

fn returned(
    place: u64,
    structural_type: u64,
    multiplicity: StructuralMultiplicity,
) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: PlaceId::new(place).unwrap(),
        structural_type: StructuralTypeId::new(structural_type).unwrap(),
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    })
}

fn factory(callee: u64) -> OperationKind {
    OperationKind::CallStructuralWithScalarArguments {
        callee: MachineId::new(callee).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    }
}

fn declarations() -> Vec<StructuralTypeDeclaration> {
    let member = StructuralTypeDeclaration {
        quotient_carrier: None,
        id: StructuralTypeId::new(2).unwrap(),
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        shape: StructuralTypeShape::Record {
            fields: Vec::new(),
            storage: StructuralRecordStorage::Packed,
        },
        scalar_restriction: None,
    };
    let owner = StructuralTypeDeclaration {
        quotient_carrier: None,
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(0),
        owns_linear_claim: false,
        copyable: false,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: TerminalIdentityIndex(2),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(member.id),
                storage: None,
            }],
            storage: StructuralRecordStorage::Packed,
        },
        scalar_restriction: None,
    };
    vec![owner, member]
}

fn replacement_then_transfer(restored: bool, transferred: bool) -> Vec<Block> {
    let root = PlaceId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let mut block = Block::plain(
        BlockId::new(1).unwrap(),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(root)],
        },
    );
    block.operations = vec![
        operation(
            1,
            returned(1, 1, StructuralMultiplicity::Affine),
            factory(1),
        ),
        operation(
            2,
            returned(2, 2, StructuralMultiplicity::Unrestricted),
            OperationKind::MoveStructuralField {
                source: root,
                path: Vec::new(),
                field: Some(field),
            },
        ),
    ];
    if restored {
        block.operations.extend([
            operation(
                3,
                returned(3, 2, StructuralMultiplicity::Unrestricted),
                factory(2),
            ),
            operation(
                4,
                OperationResult::Unit,
                OperationKind::StoreStructuralField {
                    disposition: StructuralStoreDisposition::Vacant,
                    destination: root,
                    path: Vec::new(),
                    field: Some(field),
                    value: StructuralArgument {
                        place: PlaceId::new(3).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                },
            ),
        ]);
    }
    if transferred {
        block.operations.push(operation(
            5,
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: MachineId::new(3).unwrap(),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: root,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
    }
    vec![block]
}

#[test]
fn restored_member_then_whole_transfer_withdraws_selected_discard() {
    let mut blocks = replacement_then_transfer(true, true);
    complete(
        &[],
        &[],
        &[],
        &[],
        blocks[0].id,
        &mut blocks,
        &declarations(),
    )
    .unwrap();
    assert!(
        matches!(&blocks[0].terminator, Terminator::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty())
    );
}

#[test]
fn restored_member_without_transfer_keeps_selected_discard() {
    let mut blocks = replacement_then_transfer(true, false);
    complete(
        &[],
        &[],
        &[],
        &[],
        blocks[0].id,
        &mut blocks,
        &declarations(),
    )
    .unwrap();
    assert!(
        matches!(&blocks[0].terminator, Terminator::ReturnUnit { cleanup_actions, .. }
        if cleanup_actions == &[TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(1).unwrap())])
    );
}

#[test]
fn whole_transfer_before_restoration_still_rejects() {
    let mut blocks = replacement_then_transfer(false, true);
    assert!(
        complete(
            &[],
            &[],
            &[],
            &[],
            blocks[0].id,
            &mut blocks,
            &declarations()
        )
        .is_err()
    );
}
