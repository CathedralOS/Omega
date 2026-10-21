//! Completed component products beside their descriptive publication.
//!
//! Package review needs serialized descriptions, while executable composition
//! also needs the exact canonical artifact and completed native proposal that
//! produced them. Keep those products together before their source continuation
//! leaves the compiler invocation. A native consumer receives these source-free
//! products, never a callback that reopens the dependency's source or Build.
//!
//! This carrier is not an installation, entry capability, or replacement era.
//! Receiving provider, representation, resource and physical admission remain
//! independent joins at the actual composition edge.

use std::sync::Arc;

use package_compilation::IndependentComponentDescription;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependentComponentProduct {
    description: IndependentComponentDescription,
    retained: Arc<RetainedTerminalArtifact>,
    accepted_package_permissions:
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
}

impl IndependentComponentProduct {
    /// Couple the two outputs of one component compilation. Subject equality
    /// checks this relationship only; it supplies no verification verdict or
    /// permission to execute the enclosed product.
    pub fn new(
        description: IndependentComponentDescription,
        retained: RetainedTerminalArtifact,
        accepted_package_permissions: terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    ) -> Result<Self, &'static str> {
        if description.expected_subject() != retained.artifact().view().identity() {
            return Err("component description and retained product name different subjects");
        }
        Ok(Self {
            description,
            retained: Arc::new(retained),
            accepted_package_permissions,
        })
    }

    pub const fn description(&self) -> &IndependentComponentDescription {
        &self.description
    }

    /// Acceptance acquired from this dependency's checked package custody,
    /// independently of the native proposal's own retained permission claims.
    pub const fn accepted_package_permissions(
        &self,
    ) -> &terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy
    {
        &self.accepted_package_permissions
    }

    pub fn retained(&self) -> &RetainedTerminalArtifact {
        &self.retained
    }
}
