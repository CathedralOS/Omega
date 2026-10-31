//! Invocation-owned declaration evidence for selected-plan enrichment.
//!
//! Psi publishes facts and deferred source failures. Omega chooses bindings,
//! settles reach and attaches its selected-plan identities.

/// Authored target membership. Source scope owns formation; Omega chooses which
/// declarations match a realization target. This is not execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetMachineOrigin {
    pub machine: symbols::SymbolHandle,
    pub target: String,
}

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

/// Exact callable coordinates. Psi uses checked identity handles; the plan
/// boundary renders them into its existing normalized identity vocabulary.
/// Neither form is ABI compatibility or permission to execute a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedProviderInstance<Identity> {
    pub requirement: Identity,
    pub realization: Identity,
}

/// One checked receiving application and all its exact realization candidates.
/// Keep failures inside the candidates: selection must judge cardinality before
/// consuming an individual declaration's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInstanceApplication<Identity> {
    pub candidates: Vec<Result<ClosedProviderInstance<Identity>, String>>,
}
