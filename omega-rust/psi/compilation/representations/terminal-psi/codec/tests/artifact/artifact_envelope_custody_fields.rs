//! Substitutable-field inventories for the canonical Terminal artifact
//! transport envelope (`PSIART`).
//!
//! Both families are driven through the shared substitution matrix by
//! `artifact_envelope_custody.rs`: one over the debug-free envelope and one
//! over the envelope carrying the optional debug section, so both presence
//! states of the tagged header field are exercised. Keeping the inventories in
//! their own file names the boundary between what is declared substitutable
//! and the driver that exercises every leg. Truncations, the two-field length
//! swap, and value sweeps beyond each lane's representative stay authored
//! beside the matrices.

mutation_matrix::custody_field_inventory! {
    /// One substituted envelope field of the debug-free artifact; each leg's
    /// verdict is checked by canonical envelope decoding and, for
    /// substitutions the decoder still accepts, replay against the retained
    /// artifact manifest.
    pub enum ArtifactEnvelopeFieldForTest {
        Magic,
        FormatMarker,
        SemanticLengthCleared,
        SemanticLengthShortened,
        SemanticLengthLengthened,
        SemanticLengthOverLarge,
        ProofLengthCleared,
        ProofLengthShortened,
        ProofLengthLengthened,
        ProofLengthOverLarge,
        OptimizationLengthCleared,
        OptimizationLengthShortened,
        OptimizationLengthLengthened,
        OptimizationLengthOverLarge,
        DebugTagUnknown,
        DebugTagClaimedWithoutLength,
        SemanticMagic,
        ProofMagic,
        OptimizationMagic,
        ProofPayloadInSemanticSlot,
        SemanticPayloadDropped,
        ProofPayloadDropped,
        OptimizationPayloadDropped,
        OptimizationPayloadDuplicated,
        TrailingByte,
        SemanticPayloadForeign,
        ProofPayloadForeign,
        OptimizationPayloadForeign,
        AllPayloadsForeign,
        ProofRosterRetargeted,
        OptimizationRosterSelected,
        DebugSectionAdded,
    }
}

mutation_matrix::custody_field_inventory! {
    /// One substituted envelope field of the debug-bearing artifact, checked
    /// the same way against the debug-bearing retained manifest.
    pub enum DebugBearingEnvelopeFieldForTest {
        OptimizationLengthLengthened,
        DebugTagDropped,
        DebugTagCorrupted,
        DebugLengthCleared,
        DebugLengthShortened,
        DebugLengthLengthened,
        DebugLengthOverLarge,
        DebugMagic,
        DebugPayloadDropped,
        DebugPayloadSubstituted,
        DebugSectionDropped,
    }
}
