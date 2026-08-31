#![forbid(unsafe_code)]

//! Durable identities for compiler-validated opaque representation selections.
//!
//! Build planning owns validation and construction. Lower layers consume this
//! target-independent record without depending on the build-planning service.

use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::typed_trees::ClosedConformanceApplication;

/// One exact, build-selected `Carrier satisfies OpaqueRepresentation<Opaque>`
/// application. The concrete carrier is the sole source of physical shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueRepresentationSelection {
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    application: ClosedConformanceApplication,
    selecting_machine: SymbolHandle,
    source_span: SourceSpan,
}

impl OpaqueRepresentationSelection {
    /// Retain an application after the build-planning owner has validated it.
    pub fn from_validated_application(
        opaque: SymbolHandle,
        carrier: SymbolHandle,
        application: ClosedConformanceApplication,
        selecting_machine: SymbolHandle,
        source_span: SourceSpan,
    ) -> Self {
        Self {
            opaque,
            carrier,
            application,
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

    pub fn selecting_machine(&self) -> SymbolHandle {
        self.selecting_machine
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }
}

pub fn selection_for_opaque(
    selections: &[OpaqueRepresentationSelection],
    opaque: SymbolHandle,
) -> Option<&OpaqueRepresentationSelection> {
    selections
        .iter()
        .find(|selection| selection.opaque == opaque)
}
