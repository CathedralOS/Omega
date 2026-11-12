// We define `BoundaryOpaqueRepresentationUse` and `BoundaryOpaqueRepresentationMovement`
// to track projected opaque representations used by value while materializing an
// exact boundary signature. Symbol handles remain in-process compiler join
// coordinates, and consumers must rejoin and replay the selection commitment
// and exact un-interned shape-graph occurrence before using the movement.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationUse {
    pub opaque: symbols::SymbolHandle,
    pub conformance: symbols::SymbolHandle,
    pub carrier: symbols::SymbolHandle,
    pub shape_root: u16, // exact structural occurrence node in the un-interned boundary shape graph
    pub application_report_fingerprint: u64,
    pub conformance_application_commitment: [u8; 32],
    pub representation_schema_version: u16,
    pub origin: language_semantics::opaque_representations::OpaqueRepresentationApplicationOrigin,
    pub lifecycle:
        language_semantics::opaque_representations::OpaqueRepresentationLifecycleDisposition,
    pub copy_disposition:
        language_semantics::opaque_representations::OpaqueRepresentationCopyDisposition,
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
    ) -> language_semantics::opaque_representations::OpaqueRepresentationApplicationOrigin {
        self.origin
    }

    pub const fn lifecycle(
        &self,
    ) -> language_semantics::opaque_representations::OpaqueRepresentationLifecycleDisposition {
        self.lifecycle
    }

    pub const fn copy_disposition(
        &self,
    ) -> language_semantics::opaque_representations::OpaqueRepresentationCopyDisposition {
        self.copy_disposition
    }

    pub const fn selected_application_commitment(&self) -> [u8; 32] {
        self.selected_application_commitment
    }

    pub fn rederived_selected_application_commitment(&self) -> [u8; 32] {
        language_semantics::opaque_representations::selected_application_commitment(
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
    use language_semantics::opaque_representations::{
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationMovement {
    pub(super) role: BoundaryOpaqueRepresentationMovementRole,
    pub(super) path: Vec<BoundaryOpaqueRepresentationPathElement>,
    pub(super) placement: calling_conventions::ValuePlacement,
}

impl BoundaryOpaqueRepresentationMovement {
    pub const fn role(&self) -> BoundaryOpaqueRepresentationMovementRole {
        self.role
    }

    pub const fn placement(&self) -> &calling_conventions::ValuePlacement {
        &self.placement
    }

    pub fn path(&self) -> &[BoundaryOpaqueRepresentationPathElement] {
        &self.path
    }
}
