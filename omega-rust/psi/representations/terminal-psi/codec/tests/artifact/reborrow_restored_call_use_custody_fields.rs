//! Substitutable-field inventory for the canonical Terminal module's
//! reborrow restored-call-use roster.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `reborrow_restored_call_use_custody.rs`; keeping the inventory
//! in its own file names the boundary between what is declared substitutable
//! and the driver that exercises every leg. A leg is one authored
//! substitution of a `TerminalModule` field that still canonicalizes under
//! `encode_module`: roster drops and the cleared roster, the call boundary's
//! canonical target identity, the free source identities on both the
//! exclusive and the shared-freeze row (the cohort joins only the child
//! strand), the lifecycle boundary coordinates that keep the exact
//! activation < child-activation <= child-weakening < weakening nesting, the
//! child owner identity, the free counted child owner path (cleared,
//! same-payload Case/Field tag recast, a moved fixed index), and the
//! exclusive child's write-only-to-mutable access flip. The join-constrained
//! strands (machine/operation ownership, the class/cohort agreement, the
//! call boundary's statement and ordinal, the lifetime/root reiteration, the
//! child-place extension, the formation/activation split, the member's
//! reiteration of the primary strand), non-canonical or emptied identities,
//! unknown tags, count lies, roster-order violations, and the producer-side
//! encode rejections stay authored in `reborrow_restored_call_use_custody.rs`
//! as decode- and encode-level rejections.
//!
//! These proof-only rows carry no proof obligations the verifier
//! reconstructs, so every declared leg must keep the substituted module
//! *verifiable* under the retained bundle while the two custody replays
//! reject it — the retained-manifest join
//! (`ArtifactManifestError::ManifestMismatch`) and the sealed proof subject
//! join (`ProofSubjectMismatch`), all asserted inside the matrix's checker.

use mutation_matrix::{MutationOutcome, custody_field_inventory};
use terminal_codec::ArtifactManifestError;
use terminal_psi::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment, TerminalModule,
};

use super::borrow;

custody_field_inventory! {
    /// One substitutable field of the canonical module's reborrow
    /// restored-call-use custody; each leg keeps the module verifiable while
    /// the retained-manifest and sealed-proof-subject joins reject it.
    pub enum ReborrowRestoredCallUseCustodyFieldForTest {
        // --- roster ---
        UseRosterCleared,
        FirstUseDropped,
        SecondUseDropped,
        // --- call boundary ---
        CallBoundaryTargetRenamed,
        // --- source identities ---
        SourceMachineIdentityRenamed,
        SourceStateIdentityRenamed,
        DirectOwnerIdentityRenamed,
        SharedSourceMachineIdentityRenamed,
        SharedSourceStateIdentityRenamed,
        SharedDirectOwnerIdentityRenamed,
        // --- direct root lifecycle boundaries ---
        DirectRootActivationMoved,
        DirectRootWeakeningMoved,
        // --- child strand ---
        ChildOwnerIdentityRenamed,
        ChildOwnerCaseRecastAsField,
        ChildOwnerFixedIndexMoved,
        ChildOwnerPathCleared,
        ChildAccessMutable,
    }
}

/// Apply one declared substitution to the module.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor module's restored-call-use roster is empty, so its fields have
/// nothing to lend this lifecycle-joined custody.
pub(crate) fn substitute_reborrow_restored_call_use_for_test(
    record: &mut TerminalModule,
    field: ReborrowRestoredCallUseCustodyFieldForTest,
    donor: &TerminalModule,
) {
    let _ = donor;
    let uses = &mut record.reborrow_restored_call_uses;
    match field {
        ReborrowRestoredCallUseCustodyFieldForTest::UseRosterCleared => uses.clear(),
        ReborrowRestoredCallUseCustodyFieldForTest::FirstUseDropped => {
            uses.remove(0);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SecondUseDropped => {
            uses.remove(1);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::CallBoundaryTargetRenamed => {
            let TerminalBorrowBoundarySource::Call {
                target_identity, ..
            } = &mut uses[0].call_boundary
            else {
                unreachable!("the first row's call boundary is a Call")
            };
            *target_identity = borrow(0x91);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SourceMachineIdentityRenamed => {
            uses[0].source_machine_identity = borrow(0x92);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SourceStateIdentityRenamed => {
            uses[0].source_state_identity = borrow(0x93);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::DirectOwnerIdentityRenamed => {
            uses[0].direct_root_owner_identity = borrow(0x94);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SharedSourceMachineIdentityRenamed => {
            uses[1].source_machine_identity = borrow(0x95);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SharedSourceStateIdentityRenamed => {
            uses[1].source_state_identity = borrow(0x96);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::SharedDirectOwnerIdentityRenamed => {
            uses[1].direct_root_owner_identity = borrow(0x97);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::DirectRootActivationMoved => {
            uses[0].direct_root_activation =
                TerminalBorrowBoundarySource::Statement { statement_index: 5 };
        }
        ReborrowRestoredCallUseCustodyFieldForTest::DirectRootWeakeningMoved => {
            uses[0].direct_root_weakening = TerminalBorrowBoundarySource::Statement {
                statement_index: 50,
            };
        }
        ReborrowRestoredCallUseCustodyFieldForTest::ChildOwnerIdentityRenamed => {
            uses[0].child_owner_identity = borrow(0x9A);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::ChildOwnerCaseRecastAsField => {
            let child_owner_path = &mut uses[0].child_owner_path;
            let TerminalBorrowOwnerSegment::Case(identity) = &child_owner_path[0] else {
                unreachable!("the child owner path opens with a case segment")
            };
            let identity = identity.clone();
            child_owner_path[0] = TerminalBorrowOwnerSegment::Field(identity);
        }
        ReborrowRestoredCallUseCustodyFieldForTest::ChildOwnerFixedIndexMoved => {
            let child_owner_path = &mut uses[0].child_owner_path;
            let TerminalBorrowOwnerSegment::FixedIndex(index) = &mut child_owner_path[1] else {
                unreachable!("the child owner path's second segment is a fixed index")
            };
            *index = 0x57;
        }
        ReborrowRestoredCallUseCustodyFieldForTest::ChildOwnerPathCleared => {
            uses[0].child_owner_path.clear();
        }
        ReborrowRestoredCallUseCustodyFieldForTest::ChildAccessMutable => {
            uses[0].child_access = StructuralAccess::MutableBorrow;
        }
    }
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the semantic identity
/// the manifest binds, so the join always rejects with a manifest mismatch.
/// The verifier admission and sealed-proof-subject joins are asserted inside
/// the matrix's checker.
pub(crate) fn reborrow_restored_call_use_custody_outcome(
    _field: ReborrowRestoredCallUseCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}
