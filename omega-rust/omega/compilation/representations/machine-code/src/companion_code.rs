//! Encoded auxiliary exports carried to container construction. Planning and
//! instruction encoding belong to the physical compiler, never file publication.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAuxiliaryExport {
    pub bytes: Vec<u8>,
    pub effects: calling_conventions::StateEffectsEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsInternetProviderCode {
    pub make_address: EncodedAuxiliaryExport,
    pub local_address: EncodedAuxiliaryExport,
}
