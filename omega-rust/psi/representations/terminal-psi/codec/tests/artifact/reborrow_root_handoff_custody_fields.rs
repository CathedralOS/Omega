//! Substitutable-field inventory for the canonical Terminal module's
//! reborrow root-handoff roster.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `reborrow_root_handoff_custody.rs`; keeping the inventory in its
//! own file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution of a
//! `TerminalModule` field that still canonicalizes under `encode_module`:
//! roster drops and the cleared roster, the machine rebinding in both
//! directions between the fixture's two real machines, canonical borrow
//! identity renames, the free counted owner paths (cleared, same-payload
//! Field/Case tag recasts, a moved fixed index), the exclusive-access
//! flips that stay permitted, the free boundary coordinates, and the `Call`
//! boundary's statement, ordinal, and canonical target. Non-canonical or
//! emptied identities, join-constrained strands (the lifetime/root reiteration,
//! the child-place extension, the formation/activation split), unknown tags,
//! count lies, and the producer-side encode rejections stay authored in
//! `reborrow_root_handoff_custody.rs` as decode- and encode-level rejections.
//!
//! Like the float-meaning rosters, these proof-only rows carry no proof
//! obligations the verifier reconstructs, so every declared leg must keep the
//! substituted module *verifiable* under the retained bundle while the two
//! custody replays reject it — the retained-manifest join
//! (`ArtifactManifestError::ManifestMismatch`) and the sealed proof subject
//! join (`ProofSubjectMismatch`), all asserted inside the matrix's checker.

use super::super::machine_id;
use super::borrow;
use mutation_matrix::{MutationOutcome, custody_field_inventory};
use terminal_codec::ArtifactManifestError;
use terminal_psi::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment, TerminalModule,
};

custody_field_inventory! {
    /// One substitutable field of the canonical module's reborrow root-handoff
    /// custody; each leg keeps the module verifiable while the
    /// retained-manifest and sealed-proof-subject joins reject it.
    pub enum ReborrowRootHandoffCustodyFieldForTest {
        // --- roster ---
        HandoffRosterCleared,
        FirstHandoffDropped,
        SecondHandoffDropped,
        // --- machine ---
        FirstHandoffReboundToSecondMachine,
        SecondHandoffReboundToFirstMachine,
        // --- source identities ---
        SourceMachineIdentityRenamed,
        SourceStateIdentityRenamed,
        DirectRootOwnerIdentityRenamed,
        // --- direct root owner path ---
        DirectOwnerPathCleared,
        OwnerFieldRecastAsCase,
        OwnerCaseRecastAsField,
        OwnerFixedIndexMoved,
        // --- access and boundaries ---
        DirectRootAccessWriteOnly,
        DirectRootActivationMoved,
        DirectRootWeakeningMoved,
        // --- lineage step ---
        ChildOwnerIdentityRenamed,
        ChildOwnerCaseRecastAsField,
        ChildOwnerFixedIndexMoved,
        ChildOwnerPathCleared,
        ChildAccessMutable,
        ChildWeakeningMoved,
        // --- second-row call boundary ---
        CallBoundaryStatementMoved,
        CallBoundaryOrdinalMoved,
        CallBoundaryTargetRenamed,
    }
}

/// Apply one declared substitution to the module.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor module's reborrow roster is empty, so its fields have nothing to
/// lend this coordinate-joined lineage.
pub(crate) fn substitute_reborrow_root_handoff_for_test(
    record: &mut TerminalModule,
    field: ReborrowRootHandoffCustodyFieldForTest,
    donor: &TerminalModule,
) {
    let _ = donor;
    let handoffs = &mut record.reborrow_root_handoffs;
    match field {
        ReborrowRootHandoffCustodyFieldForTest::HandoffRosterCleared => handoffs.clear(),
        ReborrowRootHandoffCustodyFieldForTest::FirstHandoffDropped => {
            handoffs.remove(0);
        }
        ReborrowRootHandoffCustodyFieldForTest::SecondHandoffDropped => {
            handoffs.remove(1);
        }
        ReborrowRootHandoffCustodyFieldForTest::FirstHandoffReboundToSecondMachine => {
            handoffs[0].machine = machine_id(2);
        }
        ReborrowRootHandoffCustodyFieldForTest::SecondHandoffReboundToFirstMachine => {
            handoffs[1].machine = machine_id(1);
        }
        ReborrowRootHandoffCustodyFieldForTest::SourceMachineIdentityRenamed => {
            handoffs[0].source_machine_identity = borrow(0x91);
        }
        ReborrowRootHandoffCustodyFieldForTest::SourceStateIdentityRenamed => {
            handoffs[0].source_state_identity = borrow(0x92);
        }
        ReborrowRootHandoffCustodyFieldForTest::DirectRootOwnerIdentityRenamed => {
            handoffs[0].direct_root_owner_identity = borrow(0x93);
        }
        ReborrowRootHandoffCustodyFieldForTest::DirectOwnerPathCleared => {
            handoffs[0].direct_root_owner_path.clear();
        }
        ReborrowRootHandoffCustodyFieldForTest::OwnerFieldRecastAsCase => {
            let owner_path = &mut handoffs[0].direct_root_owner_path;
            let TerminalBorrowOwnerSegment::Field(identity) = &owner_path[0] else {
                unreachable!("the direct owner path opens with a field segment")
            };
            let identity = identity.clone();
            owner_path[0] = TerminalBorrowOwnerSegment::Case(identity);
        }
        ReborrowRootHandoffCustodyFieldForTest::OwnerCaseRecastAsField => {
            let owner_path = &mut handoffs[0].direct_root_owner_path;
            let TerminalBorrowOwnerSegment::Case(identity) = &owner_path[1] else {
                unreachable!("the direct owner path's second segment is a case")
            };
            let identity = identity.clone();
            owner_path[1] = TerminalBorrowOwnerSegment::Field(identity);
        }
        ReborrowRootHandoffCustodyFieldForTest::OwnerFixedIndexMoved => {
            let owner_path = &mut handoffs[0].direct_root_owner_path;
            let TerminalBorrowOwnerSegment::FixedIndex(index) = &mut owner_path[2] else {
                unreachable!("the direct owner path's third segment is a fixed index")
            };
            *index = 0x55;
        }
        ReborrowRootHandoffCustodyFieldForTest::DirectRootAccessWriteOnly => {
            handoffs[0].direct_root_access = StructuralAccess::WriteOnlyBorrow;
        }
        ReborrowRootHandoffCustodyFieldForTest::DirectRootActivationMoved => {
            handoffs[0].direct_root_activation = TerminalBorrowBoundarySource::Statement {
                statement_index: 0x61,
            };
        }
        ReborrowRootHandoffCustodyFieldForTest::DirectRootWeakeningMoved => {
            handoffs[0].direct_root_weakening = TerminalBorrowBoundarySource::Statement {
                statement_index: 0x62,
            };
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildOwnerIdentityRenamed => {
            handoffs[0].lineage[0].child_owner_identity = borrow(0x96);
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildOwnerCaseRecastAsField => {
            let child_owner_path = &mut handoffs[0].lineage[0].child_owner_path;
            let TerminalBorrowOwnerSegment::Case(identity) = &child_owner_path[0] else {
                unreachable!("the child owner path opens with a case segment")
            };
            let identity = identity.clone();
            child_owner_path[0] = TerminalBorrowOwnerSegment::Field(identity);
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildOwnerFixedIndexMoved => {
            let child_owner_path = &mut handoffs[0].lineage[0].child_owner_path;
            let TerminalBorrowOwnerSegment::FixedIndex(index) = &mut child_owner_path[1] else {
                unreachable!("the child owner path's second segment is a fixed index")
            };
            *index = 0x57;
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildOwnerPathCleared => {
            handoffs[0].lineage[0].child_owner_path.clear();
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildAccessMutable => {
            handoffs[0].lineage[0].child_access = StructuralAccess::MutableBorrow;
        }
        ReborrowRootHandoffCustodyFieldForTest::ChildWeakeningMoved => {
            handoffs[0].lineage[0].child_weakening = TerminalBorrowBoundarySource::Statement {
                statement_index: 0x65,
            };
        }
        ReborrowRootHandoffCustodyFieldForTest::CallBoundaryStatementMoved => {
            let TerminalBorrowBoundarySource::Call {
                statement_index, ..
            } = &mut handoffs[1].lineage[0].child_weakening
            else {
                unreachable!("the second row's child weakening is a call boundary")
            };
            *statement_index = 0x66;
        }
        ReborrowRootHandoffCustodyFieldForTest::CallBoundaryOrdinalMoved => {
            let TerminalBorrowBoundarySource::Call { call_ordinal, .. } =
                &mut handoffs[1].lineage[0].child_weakening
            else {
                unreachable!("the second row's child weakening is a call boundary")
            };
            *call_ordinal = 0x67;
        }
        ReborrowRootHandoffCustodyFieldForTest::CallBoundaryTargetRenamed => {
            let TerminalBorrowBoundarySource::Call {
                target_identity, ..
            } = &mut handoffs[1].lineage[0].child_weakening
            else {
                unreachable!("the second row's child weakening is a call boundary")
            };
            *target_identity = borrow(0x98);
        }
    }
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the semantic identity
/// the manifest binds, so the join always rejects with a manifest mismatch.
/// The verifier admission and sealed-proof-subject joins are asserted inside
/// the matrix's checker.
pub(crate) fn reborrow_root_handoff_custody_outcome(
    _field: ReborrowRootHandoffCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}
