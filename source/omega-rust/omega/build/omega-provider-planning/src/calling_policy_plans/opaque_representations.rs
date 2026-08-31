/// One compiler-derived opaque representation actually used by value while
/// materializing an exact boundary signature.
///
/// The symbol handles are private compiler join coordinates. Downstream
/// canonical evidence must rejoin them to package-qualified declarations; it
/// must not encode arena identities. The compact report fingerprint remains
/// compatibility data beside the authoritative closed-application commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationUse {
    pub(super) opaque: psi_symbols::SymbolHandle,
    pub(super) carrier: psi_symbols::SymbolHandle,
    pub(super) application_report_fingerprint: u64,
    pub(super) application_commitment: [u8; 32],
}

impl BoundaryOpaqueRepresentationUse {
    pub const fn opaque(&self) -> psi_symbols::SymbolHandle {
        self.opaque
    }

    pub const fn carrier(&self) -> psi_symbols::SymbolHandle {
        self.carrier
    }

    pub const fn application_report_fingerprint(&self) -> u64 {
        self.application_report_fingerprint
    }

    pub const fn application_commitment(&self) -> [u8; 32] {
        self.application_commitment
    }
}
