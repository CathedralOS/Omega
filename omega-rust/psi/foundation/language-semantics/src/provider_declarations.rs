//! Invocation-owned declaration evidence for selected-plan enrichment.
//!
//! Psi publishes facts and deferred source failures. Omega chooses bindings,
//! settles reach and attaches its selected-plan identities.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReachDeclaration {
    pub requirement_identity: String,
    pub realization_name: String,
    pub upper_bound: Vec<String>,
    pub concrete_reach: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderApplicationDeclarations {
    pub has_closed_application: bool,
    pub applications: Result<Vec<String>, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRowDeclarations {
    pub reach: Result<Option<ProviderReachDeclaration>, String>,
    pub leaf_applications: Result<Vec<String>, Vec<String>>,
    pub external_applications: ProviderApplicationDeclarations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDeclarations {
    pub rows: Vec<ProviderRowDeclarations>,
}

/// Exact normalized callable identities from this checked invocation, not
/// display names, ABI compatibility or permission to execute a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedProviderInstance {
    pub requirement: String,
    pub realization: String,
}

/// One checked receiving application and all its exact realization candidates.
/// Keep failures inside the candidates: selection must judge cardinality before
/// consuming an individual declaration's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInstanceApplication {
    pub candidates: Vec<Result<ClosedProviderInstance, String>>,
}
