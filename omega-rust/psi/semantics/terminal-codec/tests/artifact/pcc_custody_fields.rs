//! Substitutable-field inventory for the bounded PCC `.proof` sidecar offer.
//!
//! The family's declared fields are driven through the shared substitution
//! matrix by `pcc_custody.rs`; keeping the inventory in its own file names
//! the boundary between what is declared substitutable and the driver that
//! exercises every leg. Values the canonical constructor refuses, roster
//! reorders, receiver-policy variations, and truncations are not one-field
//! substitutions of an offer and stay authored beside the matrix.

mutation_matrix::custody_field_inventory! {
    /// One substituted field of the offered (artifact bytes, sidecar bytes)
    /// pair; each leg's verdict is checked by canonical sidecar decoding and
    /// independent replay under the receiver's pinned policy.
    pub enum PccOfferFieldForTest {
        ProductKind,
        ArtifactCommitment,
        SemanticProfile,
        CheckerProfile,
        GuaranteeIdentity,
        GuaranteeAdded,
        PremiseAdded,
        Evidence,
        AssumptionSubstituted,
        AssumptionAdded,
        AssumptionDropped,
        DependencyIdentity,
        DependencyCommitment,
        DependencyAdded,
        DependencyDropped,
        ArtifactTruncatedUnderRecomputedCommitment,
        ArtifactForeignUnderRecomputedCommitment,
        Magic,
        FormatMarker,
        ProductTagUnknown,
        SemanticProfileLengthOverLong,
        SemanticProfileEmptyOnWire,
        CheckerProfileEmptyOnWire,
        SemanticProfileNonUtf8,
        GuaranteeCountZero,
        GuaranteeCountOverCeiling,
        PremiseCountOverCeiling,
        GuaranteeIdentityEmptyOnWire,
        EvidenceLengthOverCeiling,
        AssumptionCountOverCeiling,
        DependencyCountOverCeiling,
        DependencyIdentityEmptyOnWire,
        EvidenceBytesOnWire,
        DependencyCommitmentOnWire,
        TrailingByte,
    }
}
