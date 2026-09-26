//! Substitutable-field inventory for the canonical Terminal obligation ledger.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `obligation_ledger_custody.rs`; keeping the inventory in its own
//! file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution that
//! still decodes to a canonical ledger, so a single record field may appear
//! under several variants (each owner kind's identities and positions, each
//! authorized-class field). Wire framing, zero identities, closed tags,
//! lying counts, truncations, and non-canonical rosters are not representable
//! ledgers and stay authored below the matrix as decode-level rejections.

optimization_core::custody_field_inventory! {
    /// One representable substitution of a `TerminalObligationLedger`; each
    /// leg's verdict is checked by `validate_terminal_obligation_ledger`, the
    /// independent reconstruction replay against the fixture module and
    /// current trust graph.
    pub enum ObligationLedgerCustodyFieldForTest {
        ProgramFingerprintFlipped,
        ProgramFingerprintZeroed,
        ProgramFingerprintSubstituted,
        TrustGraphIdentityFlipped,
        TrustGraphIdentityZeroed,
        LeadingRowDropped,
        TrailingRowDropped,
        RosterReordered,
        ForeignObligationAppended,
        OperationOwnerMachine,
        OperationOwnerOperation,
        CallRequirementOwnerMachine,
        CallRequirementOwnerOperation,
        CallRequirementPosition,
        EnsuresOwnerMachine,
        EnsuresOwnerContract,
        EnsuresOwnerClausePosition,
        OperationRowCallRequirementOwner,
        OperationRowNominalCleanupOwner,
        OperationRowEnsuresOwner,
        OperationRowBlockInvariantOwner,
        BlockInvariantOwnerMachine,
        BlockInvariantOwnerHeader,
        BlockInvariantOwnerEdge,
        CleanupOwnerMachine,
        CleanupOwnerEdge,
        CleanupOwnerCleanupPosition,
        CleanupOwnerRequirementPosition,
        ObligationIdentity,
        AdmissionAuthorizedClass,
        AdmissionSite,
        AdmissionKind,
        CheckedAssemblyAdmissionKind,
        AdmissionAuthority,
        OperationGoal,
        CallRequirementGoal,
        EnsuresGoal,
        RequirementMemberSubstituted,
        RequirementMemberDropped,
        RequirementRosterExtended,
        RequirementRosterReordered,
        AxiomMemberSubstituted,
        AxiomMemberDropped,
        AxiomRosterExtended,
        CanonicalCertificateFlipped,
    }
}
