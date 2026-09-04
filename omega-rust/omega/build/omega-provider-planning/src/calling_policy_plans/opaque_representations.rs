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
    pub(super) conformance: psi_symbols::SymbolHandle,
    pub(super) carrier: psi_symbols::SymbolHandle,
    pub(super) shape_root: u16,
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

    pub const fn conformance(&self) -> psi_symbols::SymbolHandle {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryOpaqueRepresentationMovementRole {
    Parameter {
        formal_ordinal: u32,
        native_ordinal: u32,
    },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryOpaqueRepresentationPathElement {
    FixedArrayElement,
    RecordField { ordinal: u16 },
}

/// Exact target movement assigned to one opaque occurrence by the
/// replay-validated boundary entry plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationMovement {
    pub(super) role: BoundaryOpaqueRepresentationMovementRole,
    pub(super) path: Vec<BoundaryOpaqueRepresentationPathElement>,
    pub(super) placement: omega_calling_conventions::ValuePlacement,
}

impl BoundaryOpaqueRepresentationMovement {
    pub const fn role(&self) -> BoundaryOpaqueRepresentationMovementRole {
        self.role
    }

    pub const fn placement(&self) -> &omega_calling_conventions::ValuePlacement {
        &self.placement
    }

    pub fn path(&self) -> &[BoundaryOpaqueRepresentationPathElement] {
        &self.path
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
            conformance: psi_symbols::SymbolHandle::invalid(),
            carrier: psi_symbols::SymbolHandle::invalid(),
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
