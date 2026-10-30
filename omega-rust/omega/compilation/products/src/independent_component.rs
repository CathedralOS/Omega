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
use terminal_psi_to_abstract_operations::RetainedComponentNativeInputs;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;
mod retention;

/// One selected component's completed producing join. This invocation-local
/// record is not serialized and conveys no receiver installation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIndependentComponent {
    input: RetainedComponentNativeInputs,
    plan: effects::provider_plan::ProviderPlan,
    receiving_identities: Arc<terminal_psi::TerminalIdentityTable>,
    accepted_package_permissions:
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
}

impl SelectedIndependentComponent {
    pub fn join(
        component: Arc<component_description::VerifiedComponent>,
        product: &IndependentComponentProduct,
        plan: effects::provider_plan::ProviderPlan,
        receiving_identities: Arc<terminal_psi::TerminalIdentityTable>,
        applications: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<Self, String> {
        let input = RetainedComponentNativeInputs::join(
            component,
            Arc::clone(&product.retained),
            &plan,
            applications,
        )?;
        Ok(Self {
            input,
            plan,
            receiving_identities,
            accepted_package_permissions: product.accepted_package_permissions.clone(),
        })
    }

    pub fn input(
        &self,
    ) -> Result<terminal_psi_to_abstract_operations::ComponentNativeInputs<'_>, String> {
        self.input.borrow()
    }

    pub fn plan(&self) -> &effects::provider_plan::ProviderPlan {
        &self.plan
    }

    pub fn receiving_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.receiving_identities
    }

    pub fn accepted_package_permissions(
        &self,
    ) -> &terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy
    {
        &self.accepted_package_permissions
    }
}

#[derive(Debug, Clone)]
pub struct IndependentComponentProduct {
    description: IndependentComponentDescription,
    retained: Arc<RetainedTerminalArtifact>,
    accepted_package_permissions:
        terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy,
    native_derivation_inputs: Option<Result<Arc<[u8]>, String>>,
}

impl PartialEq for IndependentComponentProduct {
    fn eq(&self, other: &Self) -> bool {
        self.description == other.description
            && self.retained == other.retained
            && self.accepted_package_permissions == other.accepted_package_permissions
    }
}
impl Eq for IndependentComponentProduct {}

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
            native_derivation_inputs: None,
        })
    }

    pub const fn description(&self) -> &IndependentComponentDescription {
        &self.description
    }

    /// Optional storage belongs to this publication, separately from semantic
    /// product equality. A capture failure preserves its reason and never
    /// changes the fresh in-process compilation's result.
    pub fn with_native_derivation_inputs(mut self, inputs: Result<Vec<u8>, String>) -> Self {
        self.native_derivation_inputs = Some(inputs.map(Arc::from));
        self
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
