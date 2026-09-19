use crate::lowering::LoweringError;

#[derive(Debug)]
pub enum ArtifactLoweringError {
    SemanticDecode(terminal_codec::CodecError),
    ObligationLedgerDecode(terminal_codec::CodecError),
    TrustGraph(terminal_codec::TrustGraphError),
    ObligationReplay(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    ProofFingerprint(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    /// Consumers that do not own the separate plan-laid input carrier may not
    /// silently discard the roster.
    PlacedViewInputsRequireCustodyLowering,
    /// A supplied provider establishment answered no declared direct-entry
    /// roster row.
    PlacedViewEstablishmentUnexpected {
        machine: semantic_vocabulary::MachineId,
        position: u32,
    },
    /// Two supplied provider establishments answered one roster row.
    PlacedViewEstablishmentDuplicate {
        machine: semantic_vocabulary::MachineId,
        position: u32,
    },
    /// Referent domain qualifications must be strictly ordered and
    /// deduplicated, the same canonical form structural arguments carry.
    PlacedViewEstablishmentQualificationsNonCanonical,
    /// An exclusive-borrow referent may not overlap another established
    /// referent's place.
    PlacedViewEstablishmentAliasing(u64),
    Lowering(LoweringError),
}

impl std::fmt::Display for ArtifactLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactLoweringError {}
