use crate::lowering::LoweringError;

#[derive(Debug)]
pub enum ArtifactLoweringError {
    SemanticDecode(psi_terminal_codec::CodecError),
    ObligationLedgerDecode(psi_terminal_codec::CodecError),
    TrustGraph(psi_terminal_codec::TrustGraphError),
    ObligationReplay(psi_terminal_codec::CodecError),
    ProofDecode(psi_terminal_codec::ProofCodecError),
    ProofEncode(psi_terminal_codec::ProofCodecError),
    ProofFingerprint(psi_terminal_codec::ProofCodecError),
    Verification(psi_terminal_verifier::VerificationError),
    FixedFuel(psi_terminal_fixed_fuel::FixedFuelError),
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
