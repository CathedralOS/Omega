use crate::lowering::LoweringError;

#[derive(Debug)]
pub enum ArtifactLoweringError {
    SemanticDecode(terminal_codec::CodecError),
    ObligationLedgerDecode(terminal_codec::CodecError),
    TrustGraph(terminal_codec::TrustGraphError),
    ObligationReplay(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    ProofEncode(terminal_codec::ProofCodecError),
    ProofFingerprint(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    FixedFuel(terminal_fixed_fuel::FixedFuelError),
    RankedNativeCustody(&'static str),
    /// Ordinary/optimizer entrances do not own the separate plan-laid input
    /// carrier and therefore may not silently discard it.
    PlacedViewInputsRequireCustodyLowering,
    Lowering(LoweringError),
}

impl std::fmt::Display for ArtifactLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactLoweringError {}
