//! Substitutable-field inventory for the canonical pre-Terminal optimization
//! execution receipt.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `optimization_execution_custody.rs`; keeping the inventory in its
//! own file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution, so a
//! single record field may appear under several variants (the selections
//! roster covers member substitution, extension, and shrinkage).

mutation_matrix::custody_field_inventory! {
    /// One authored substitution leg over the `PsiOptimizationExecutionRecord`
    /// content fields; each leg's verdict is checked by the published
    /// artifact-manifest replay, with the produced-output `validate_output`
    /// binding asserted for output-side legs.
    pub enum OptimizationExecutionCustodyFieldForTest {
        SelectionMemberSubstituted,
        SelectionRosterExtended,
        SelectionMemberDropped,
        InputSemanticSubstituted,
        InputSemanticZeroed,
        InputProofSubstituted,
        OutputSemanticSubstituted,
        OutputProofSubstituted,
    }
}
