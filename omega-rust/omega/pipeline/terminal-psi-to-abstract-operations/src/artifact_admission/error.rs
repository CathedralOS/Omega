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
    UnsupportedUnsignedCountdownNativeCustody,
    /// Consumers that do not own the separate plan-laid input carrier may not
    /// silently discard the roster.
    PlacedViewInputsRequireCustodyLowering,
    Lowering(LoweringError),
}

impl std::fmt::Display for ArtifactLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactLoweringError {}
