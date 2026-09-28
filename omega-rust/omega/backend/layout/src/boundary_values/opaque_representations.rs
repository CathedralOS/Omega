/// One projected opaque representation actually used by value while
/// materializing an exact boundary signature.
///
/// The symbol handles are in-process compiler join coordinates. Downstream
/// canonical evidence must rejoin them to package-qualified declarations; it
/// must not encode arena identities. The compact report fingerprint remains
/// compatibility data beside the authoritative closed-application commitment.
/// This descriptive record is not authority: consumers must rejoin and replay
/// its selection commitment and exact graph occurrence before using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationUse {
    pub opaque: symbols::SymbolHandle,
    pub conformance: symbols::SymbolHandle,
    pub carrier: symbols::SymbolHandle,
    pub shape_root: u16,
    pub application_report_fingerprint: u64,
    pub conformance_application_commitment: [u8; 32],
    pub representation_schema_version: u16,
    pub origin: representation_selections::OpaqueRepresentationApplicationOrigin,
    pub lifecycle: representation_selections::OpaqueRepresentationLifecycleDisposition,
    pub copy_disposition: representation_selections::OpaqueRepresentationCopyDisposition,
    pub selected_application_commitment: [u8; 32],
}

impl BoundaryOpaqueRepresentationUse {
    pub const fn opaque(&self) -> symbols::SymbolHandle {
        self.opaque
    }

    pub const fn carrier(&self) -> symbols::SymbolHandle {
        self.carrier
    }

    pub const fn conformance(&self) -> symbols::SymbolHandle {
        self.conformance
    }

    /// Exact node where the selected carrier entered the materialized
    /// boundary shape graph. Shape construction does not intern nodes, so this
    /// coordinate identifies one structural occurrence rather than one merely
    /// equal layout.
    pub const fn shape_root(&self) -> u16 {
        self.shape_root
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

    pub const fn origin(&self) -> representation_selections::OpaqueRepresentationApplicationOrigin {
        self.origin
    }

    pub const fn lifecycle(
        &self,
    ) -> representation_selections::OpaqueRepresentationLifecycleDisposition {
        self.lifecycle
    }

    pub const fn copy_disposition(
        &self,
    ) -> representation_selections::OpaqueRepresentationCopyDisposition {
        self.copy_disposition
    }

    pub const fn selected_application_commitment(&self) -> [u8; 32] {
        self.selected_application_commitment
    }

    pub fn rederived_selected_application_commitment(&self) -> [u8; 32] {
        representation_selections::selected_application_commitment(
            self.conformance_application_commitment,
            self.lifecycle,
            self.copy_disposition,
            self.origin,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::BoundaryOpaqueRepresentationUse;
    use representation_selections::{
        OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
        OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
        selected_application_commitment,
    };

    #[test]
    fn selected_application_replay_rejects_copy_disposition_drift() {
        let conformance = [0x21; 32];
        let mut use_ = BoundaryOpaqueRepresentationUse {
            opaque: symbols::SymbolHandle::invalid(),
            conformance: symbols::SymbolHandle::invalid(),
            carrier: symbols::SymbolHandle::invalid(),
            shape_root: 0,
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
