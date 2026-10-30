//! Own authored provider derivation before native and toolchain augmentation.
//!
//! Early settlement and artifact publication consume the same target/defaults/
//! candidate relationship. Publication may ask for additional target inventories,
//! but never reconstructs the one settlement already produced. These are source
//! declarations, not selected supplies or portable declaration-owner evidence.

use crate::{DerivedProviderPlan, ProviderPlanningContext, ProviderSelection};
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredTargetProviders {
    target: Option<target::TargetProfile>,
    defaults: Vec<ProviderSelection>,
    candidates: Vec<DerivedProviderPlan>,
}

impl AuthoredTargetProviders {
    pub(crate) fn target(&self) -> Option<target::TargetProfile> {
        self.target
    }

    pub(crate) fn derive(
        context: ProviderPlanningContext<'_>,
        body_declarations: &DirectCallDeclarations<SymbolHandle>,
        calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
        target: Option<target::TargetProfile>,
        declarations: crate::SettledProviderDefaults,
    ) -> Result<Self, Vec<Diagnostic>> {
        let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::new(
            context,
            body_declarations,
            calling_plans,
            target.map(target::TargetProfile::target_name),
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
}

/// Source-side preparation for the full publication target roster. Unavailable
/// targets retain their refusal; they cannot invalidate another target's artifact.
pub struct TargetProviderInventoryRequest<'program> {
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub context: ProviderPlanningContext<'program>,
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub settlement: &'program crate::ProviderSettlement,
}

impl TargetProviderInventoryRequest<'_> {
    pub fn prepare(&self, target: &str) -> Result<Arc<AuthoredTargetProviders>, Vec<Diagnostic>> {
        let profile = target::TargetProfile::from_canonical_target_name(target).map_err(|_| {
            vec![Diagnostic::error(format!(
                "provider proposal has no receiving target catalog for `{target}`"
            ))]
        })?;
        profile.native_realization().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "target `{target}` has no provider realization in this compiler"
            ))]
        })?;
        let retained = &self.settlement.authored_providers;
        if retained.target == Some(profile) {
            return Ok(Arc::clone(retained));
        }
        let declarations = super::settle_provider_defaults(self.defaults, profile);
        AuthoredTargetProviders::derive(
            self.context,
            self.body_declarations,
            &self.settlement.boundary_calling_plan_realizations,
            Some(profile),
            declarations,
        )
        .map(Arc::new)
    }
}
