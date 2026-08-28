//! Provider-selection projection consumed by program-storage installation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageSelectedProviderPlan {
    pub identity: omega_external_roots::ProviderPlanId,
    pub schema: omega_effects::provider_plan::ServiceSchema,
}

impl ProgramStorageSelectedProviderPlan {
    pub fn new(
        identity: omega_external_roots::ProviderPlanId,
        schema: omega_effects::provider_plan::ServiceSchema,
    ) -> Self {
        Self { identity, schema }
    }
}
