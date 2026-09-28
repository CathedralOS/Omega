//! Substitutable-field inventory for the semantic module's placed-view input
//! roster.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `placed_view_input_custody.rs`; keeping the inventory in its own
//! file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored wire substitution of
//! the canonical module encoding, so one field appears under several variants
//! (a representable respelling beside each rejected spelling, each rejected
//! tag value). A leg either rejects at canonical decoding with an exact error
//! or decodes to a divergent module that the retained manifest replay
//! rejects. Producer-side module states that cannot serialize canonically and
//! envelope truncations are not wire field legs and stay authored in the
//! test.

mutation_matrix::custody_field_inventory! {
    /// One wire substitution of the placed-view input roster; each leg's
    /// verdict is checked by canonical decoding and, for substitutions that
    /// still decode, the retained artifact-manifest replay.
    pub enum PlacedViewInputCustodyFieldForTest {
        RosterCountOver,
        RosterCountMax,
        RosterCleared,
        FirstRowDropped,
        SecondRowDropped,
        RowDuplicated,
        RosterReordered,
        MachineZero,
        MachineOutsideModule,
        FirstRowReboundToSecondMachine,
        SecondRowReboundToFirstMachine,
        FirstPositionMoved,
        SecondPositionMoved,
        SourceMachineRenamed,
        SourceMachineEmptied,
        SourceMachineNonHermetic,
        SourceMachineNonUtf8,
        SourceStateRenamed,
        SourceStateEmptied,
        SourceStateLengthLie,
        SourceParameterRenamed,
        SourceParameterNonHermetic,
        AccessWriteOnly,
        SecondAccessSharedBorrow,
        AccessOwned,
        AccessTagZero,
        AccessTagFive,
        AccessTagMax,
        BindingConstSet,
        BindingConstNonBoolean,
        BindingMutableCleared,
        BindingMutableNonBoolean,
        ViewIdentityForged,
        ViewIdentityEmptied,
        PolicySubstituted,
        PolicyNonHermetic,
        SchemaSubstituted,
        PlanRenamed,
        PlanNonHermetic,
        FingerprintZero,
        FingerprintSubstituted,
        CommitmentZero,
        CommitmentSubstituted,
    }
}
