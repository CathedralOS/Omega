#![forbid(unsafe_code)]

//! Durable identities for compiler-validated opaque representation selections.
//!
//! Build planning owns validation and construction. Lower layers consume this
//! target-independent record without depending on the build-planning service.

use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::typed_trees::ClosedConformanceApplication;
use sha2::{Digest, Sha256};

/// Current target-independent opaque-representation application schema.
///
/// Target movement remains part of the boundary-plan application. This
/// version covers the selected conformance, its origin, inert lifecycle, and
/// the semantic-copy disposition derived from the complete carrier graph.
pub const OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION: u16 = 1;

/// Compiler-derived lifecycle role for one selected opaque representation.
///
/// V1 has exactly one role: the carrier contributes storage only and owns no
/// independently invoked lifecycle semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueRepresentationLifecycleDisposition {
    Inert,
}

/// Compiler-derived relationship between semantic duplication and physical
/// carrier copying.
///
/// `PlacementOnly` means backend byte copies may only relocate one affine or
/// linear semantic occurrence. `CheckedSemanticCopy` means the opaque datum is
/// unrestricted and the complete inert carrier graph is structurally
/// unrestricted, so a checked semantic copy may create another occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueRepresentationCopyDisposition {
    PlacementOnly,
    CheckedSemanticCopy,
}

/// Closed source of one selected v1 opaque-representation application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueRepresentationApplicationOrigin {
    NamedConformance,
}

/// One exact, build-selected `Carrier satisfies OpaqueRepresentation<Opaque>`
/// application. The concrete carrier is the sole source of physical shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueRepresentationSelection {
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    application: ClosedConformanceApplication,
    lifecycle: OpaqueRepresentationLifecycleDisposition,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    origin: OpaqueRepresentationApplicationOrigin,
    selected_application_commitment: [u8; 32],
    selecting_machine: SymbolHandle,
    source_span: SourceSpan,
}

impl OpaqueRepresentationSelection {
    /// Retain an application after the build-planning owner has validated it.
    pub fn from_validated_application(
        opaque: SymbolHandle,
        carrier: SymbolHandle,
        application: ClosedConformanceApplication,
        lifecycle: OpaqueRepresentationLifecycleDisposition,
        copy_disposition: OpaqueRepresentationCopyDisposition,
        origin: OpaqueRepresentationApplicationOrigin,
        selecting_machine: SymbolHandle,
        source_span: SourceSpan,
    ) -> Self {
        let selected_application_commitment = selected_application_commitment(
            application.commitment.as_bytes(),
            lifecycle,
            copy_disposition,
            origin,
        );
        Self {
            opaque,
            carrier,
            application,
            lifecycle,
            copy_disposition,
            origin,
            selected_application_commitment,
            selecting_machine,
            source_span,
        }
    }

    pub fn opaque(&self) -> SymbolHandle {
        self.opaque
    }

    pub fn carrier(&self) -> SymbolHandle {
        self.carrier
    }

    pub fn application(&self) -> &ClosedConformanceApplication {
        &self.application
    }

    pub fn lifecycle(&self) -> OpaqueRepresentationLifecycleDisposition {
        self.lifecycle
    }

    pub fn copy_disposition(&self) -> OpaqueRepresentationCopyDisposition {
        self.copy_disposition
    }

    pub fn origin(&self) -> OpaqueRepresentationApplicationOrigin {
        self.origin
    }

    pub const fn schema_version(&self) -> u16 {
        OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
    }

    /// Domain-separated commitment to the target-independent selected
    /// representation application. Build-machine identity and source
    /// occurrence are provenance and deliberately absent.
    pub const fn selected_application_commitment(&self) -> [u8; 32] {
        self.selected_application_commitment
    }

    /// Recompute the selected-application commitment from the complete
    /// retained fields. Compiler replay uses this before trusting the stored
    /// commitment in a boundary plan.
    pub fn rederived_selected_application_commitment(&self) -> [u8; 32] {
        selected_application_commitment(
            self.application.commitment.as_bytes(),
            self.lifecycle,
            self.copy_disposition,
            self.origin,
        )
    }

    pub fn selecting_machine(&self) -> SymbolHandle {
        self.selecting_machine
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }
}

pub fn selected_application_commitment(
    conformance_application_commitment: [u8; 32],
    lifecycle: OpaqueRepresentationLifecycleDisposition,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    origin: OpaqueRepresentationApplicationOrigin,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.opaque-representation-application.v1");
    digest.update(OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION.to_le_bytes());
    digest.update([match origin {
        OpaqueRepresentationApplicationOrigin::NamedConformance => 1,
    }]);
    digest.update([match lifecycle {
        OpaqueRepresentationLifecycleDisposition::Inert => 1,
    }]);
    digest.update([match copy_disposition {
        OpaqueRepresentationCopyDisposition::PlacementOnly => 1,
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
    }]);
    digest.update(conformance_application_commitment);
    digest.finalize().into()
}

pub fn selection_for_opaque(
    selections: &[OpaqueRepresentationSelection],
    opaque: SymbolHandle,
) -> Option<&OpaqueRepresentationSelection> {
    selections
        .iter()
        .find(|selection| selection.opaque == opaque)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_application_commitment_binds_copy_disposition() {
        let conformance = [0x5a; 32];
        let placement = selected_application_commitment(
            conformance,
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::PlacementOnly,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
        );
        let copying = selected_application_commitment(
            conformance,
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
        );
        assert_ne!(placement, copying);
        assert_ne!(placement, conformance);
        assert_ne!(copying, conformance);
    }
}
