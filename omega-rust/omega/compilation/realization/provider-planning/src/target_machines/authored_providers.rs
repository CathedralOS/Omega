//! Own authored provider derivation before native and toolchain augmentation.
//!
//! Source preparation closes the target/default/candidate relationship before
//! portable publication. Calling attaches to the retained exact surfaces later.
//! These are declarations, not selected supplies or execution authority.

use crate::{DerivedProviderPlan, ProviderSelection};
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthoredTargetProviders {
    target: Option<target::TargetProfile>,
    defaults: Vec<ProviderSelection>,
    candidates: Vec<DerivedProviderPlan>,
}

impl AuthoredTargetProviders {
    /// Neutral helper contracts precede the ABI value a Calling policy computes.
    /// They use ordinary whole-provider selection but grant no native ABI identity.
    pub(crate) fn unevaluated(
        services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
        body_declarations: &DirectCallDeclarations<SymbolHandle>,
        defaults: &[ProviderSelection],
        target: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidates =
            crate::derive_satisfies_plans(crate::ProviderPlanDerivation::unevaluated(
                services,
                body_declarations,
                &[],
                Some(target.target_name()),
            ))?;
        Ok(Self {
            target: Some(target),
            defaults: defaults.to_vec(),
            candidates,
        })
    }

    pub(crate) fn target(&self) -> Option<target::TargetProfile> {
        self.target
    }

    pub(crate) fn derive(
        services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
        body_declarations: &DirectCallDeclarations<SymbolHandle>,
        bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
        target: Option<target::TargetProfile>,
        declarations: crate::SettledTargetMachineDeclarations,
        native_bindings: &[crate::DerivedProviderPlan],
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::evaluated(
            services,
            body_declarations,
            &[],
            target.map(target::TargetProfile::target_name),
            bindings,
            &declarations.origins,
            native_bindings,
        ))?;
        Ok(Self {
            target,
            defaults: declarations.provider_defaults,
            candidates,
        })
    }

    pub(crate) fn defaults(&self) -> &[ProviderSelection] {
        &self.defaults
    }

    pub(crate) fn candidates(&self) -> &[DerivedProviderPlan] {
        &self.candidates
    }

    pub(crate) fn into_candidates(self) -> Vec<DerivedProviderPlan> {
        self.candidates
    }
}
