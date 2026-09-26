//! Substitutable-field inventory for the semantic module's operation crash
//! contract roster.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `operation_crash_contract_custody.rs`; keeping the inventory in
//! its own file names the boundary between what is declared substitutable and
//! the driver that exercises every leg. A leg is one authored wire
//! substitution of the canonical module encoding, so one field appears under
//! several variants (each rejected tag value, each rebinding target). Most
//! legs reject at canonical decoding with an exact error; the dropped rows, the
//! cleared roster, the alias-preserving formal rebinding, and the retarget to
//! the unrowed spare operation decode to a divergent module that the retained
//! manifest replay rejects. Producer-side module states that cannot serialize
//! canonically are not wire legs and stay authored at the encoder.

optimization_core::custody_field_inventory! {
    /// One wire substitution of the operation crash contract roster; each
    /// leg's verdict is checked by canonical decoding and, for substitutions
    /// that still decode, the retained artifact-manifest replay.
    pub enum OperationCrashContractCustodyFieldForTest {
        RosterCleared,
        FirstRowDropped,
        SecondRowDropped,
        RosterCountUnder,
        RosterCountOver,
        RosterCountMax,
        RowDuplicated,
        RosterReordered,
        FirstMachineZero,
        FirstMachineLiftedPastPeer,
        SecondMachineOutsideModule,
        FirstOperationZero,
        FirstOperationCollidingWithPeer,
        FirstOperationLiftedPastPeer,
        SecondOperationOutsideMachine,
        SecondOperationOnOperandFreeConstant,
        SecondOperationCollidingWithPeer,
        SecondOperationReboundToUnrowedAlias,
        PublishedBucketCountOver,
        PublishedRoutesEmptied,
        PublishedBucketDuplicated,
        PublishedBucketSplicedOutOfCauseOrder,
        PublishedCauseTagZero,
        PublishedCauseTagThree,
        PublishedCauseTagMax,
        PublishedTrapRecastAsAbort,
        PublishedAlternativeCountOver,
        PublishedAlternativesEmptied,
        PublishedAlternativesTruthSpliced,
        PublishedGuardTagTwo,
        PublishedGuardTagMax,
        PublishedGuardRecastAsTruth,
        PublishedPredicateRecastAsEquality,
        PublishedPredicateRecastAsNonStrictBound,
        PublishedPredicateRecastAsTruth,
        PublishedPropositionTagZero,
        PublishedPropositionTagNinetyNine,
        PublishedPropositionTagMax,
        PublishedOperandRecastAsBooleanTerm,
        PublishedOperandRecastAsIntegerLiteral,
        PublishedOperandTermTagZero,
        PublishedOperandTermTagNinetyNine,
        PublishedOperandTermTagMax,
        PublishedFormalZero,
        PublishedFormalOutsideTelescope,
        PublishedFormalReboundAcrossDistinctOperands,
        PublishedFormalReboundAcrossAliasedOperands,
        PublishedOperandScalarRecastAsBoolean,
        PublishedOperandRecastAsUnsigned,
        PublishedOperandNarrowed,
        PublishedOperandZeroWidth,
        PublishedBoundRecastAsValueTerm,
        PublishedBoundRecastAsUnsigned,
        PublishedBoundWidened,
        PublishedBoundUnsignedValueTag,
        PublishedBoundBelowZero,
        PublishedBoundAboveZero,
        ContinuationBucketCountOver,
        ContinuationsEmptied,
        ContinuationBucketDuplicated,
        ContinuationCauseTagZero,
        ContinuationCauseTagThree,
        ContinuationCauseTagMax,
        ContinuationTrapRecastAsAbort,
        ContinuationAlternativesEmptied,
        ContinuationAlternativesTruthSpliced,
        ContinuationGuardTagTwo,
        ContinuationGuardTagMax,
        ContinuationGuardRecastAsTruth,
        ContinuationPredicateRecastAsEquality,
        ContinuationOperandZero,
        ContinuationOperandReboundToOtherParameter,
        ContinuationOperandReboundToOperationResult,
        ContinuationOperandReboundToFormal,
        ContinuationBoundAboveZero,
        SecondPublishedBucketCountOver,
        SecondPublishedCauseRecastAsTrap,
        SecondContinuationCauseRecastAsTrap,
        SecondContinuationOperandReboundToOtherActual,
        SecondContinuationOperandReboundToFormal,
        SecondContinuationBoundBelowZero,
    }
}
