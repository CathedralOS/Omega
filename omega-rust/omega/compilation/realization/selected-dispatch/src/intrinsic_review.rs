//! Exact source-independent data retained from provider binding.
//!
//! The retired settlement producer is removed. These accessors do not create
//! an accepted binding or grant permission to execute one.

use provider_planning::service_declarations::ServiceDeclaration;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAcceptedSemanticBinding {
    accepted: package_compilation::AcceptedSemanticBinding,
    declaration: Arc<ServiceDeclaration>,
}

impl ResolvedAcceptedSemanticBinding {
    pub const fn accepted(&self) -> &package_compilation::AcceptedSemanticBinding {
        &self.accepted
    }

    pub fn declaration(&self) -> &ServiceDeclaration {
        &self.declaration
    }

    pub const fn role(&self) -> package_compilation::AcceptedSemanticBindingRole {
        self.accepted.role()
    }
}
