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
    pub(super) conformance_application_commitment: [u8; 32],
    pub(super) representation_schema_version: u16,
    pub(super) origin: omega_representation_planning::OpaqueRepresentationApplicationOrigin,
    pub(super) lifecycle: omega_representation_planning::OpaqueRepresentationLifecycleDisposition,
    pub(super) copy_disposition: omega_representation_planning::OpaqueRepresentationCopyDisposition,
    pub(super) selected_application_commitment: [u8; 32],
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

    pub const fn conformance_application_commitment(&self) -> [u8; 32] {
        self.conformance_application_commitment
    }

    pub const fn representation_schema_version(&self) -> u16 {
        self.representation_schema_version
    }

    pub const fn origin(
        &self,
    ) -> omega_representation_planning::OpaqueRepresentationApplicationOrigin {
        self.origin
    }

    pub const fn lifecycle(
        &self,
    ) -> omega_representation_planning::OpaqueRepresentationLifecycleDisposition {
        self.lifecycle
    }

    pub const fn copy_disposition(
        &self,
    ) -> omega_representation_planning::OpaqueRepresentationCopyDisposition {
        self.copy_disposition
    }

    pub const fn selected_application_commitment(&self) -> [u8; 32] {
        self.selected_application_commitment
    }

    pub fn rederived_selected_application_commitment(&self) -> [u8; 32] {
        omega_representation_planning::selected_application_commitment(
            self.conformance_application_commitment,
            self.lifecycle,
            self.copy_disposition,
            self.origin,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_representation_planning::{
        OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
        OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
        selected_application_commitment,
    };

    #[test]
    fn selected_application_replay_rejects_copy_disposition_drift() {
        let conformance = [0x21; 32];
        let mut use_ = BoundaryOpaqueRepresentationUse {
            opaque: psi_symbols::SymbolHandle::invalid(),
            carrier: psi_symbols::SymbolHandle::invalid(),
            application_report_fingerprint: 7,
            conformance_application_commitment: conformance,
            representation_schema_version: OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
            origin: OpaqueRepresentationApplicationOrigin::NamedConformance,
            lifecycle: OpaqueRepresentationLifecycleDisposition::Inert,
            copy_disposition: OpaqueRepresentationCopyDisposition::PlacementOnly,
            selected_application_commitment: selected_application_commitment(
                conformance,
                OpaqueRepresentationLifecycleDisposition::Inert,
                OpaqueRepresentationCopyDisposition::PlacementOnly,
                OpaqueRepresentationApplicationOrigin::NamedConformance,
            ),
        };
        assert_eq!(
            use_.selected_application_commitment(),
            use_.rederived_selected_application_commitment()
        );
        use_.copy_disposition = OpaqueRepresentationCopyDisposition::CheckedSemanticCopy;
        assert_ne!(
            use_.selected_application_commitment(),
            use_.rederived_selected_application_commitment()
        );
    }
}
