#![forbid(unsafe_code)]

//! Durable identities for compiler-validated opaque representation selections.
//!
//! Build planning owns validation and construction. Lower layers consume this
//! target-independent record without depending on the build-planning service.

use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::typed_trees::ClosedConformanceApplication;

/// Compiler-derived lifecycle role for one selected opaque representation.
///
/// V1 has exactly one role: the carrier contributes storage only and owns no
/// independently invoked lifecycle semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueRepresentationLifecycleDisposition {
    Inert,
}

/// One exact, build-selected `Carrier satisfies OpaqueRepresentation<Opaque>`
/// application. The concrete carrier is the sole source of physical shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueRepresentationSelection {
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    application: ClosedConformanceApplication,
    lifecycle: OpaqueRepresentationLifecycleDisposition,
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
        selecting_machine: SymbolHandle,
        source_span: SourceSpan,
    ) -> Self {
        Self {
            opaque,
            carrier,
            application,
            lifecycle,
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
