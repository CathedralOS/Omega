//! Provider-installation integration coverage, classified by the semantic
//! custody each scenario constructs and independently replays.

#[path = "provider_installation/builders.rs"]
mod builders;
#[path = "provider_installation/catalog_admission.rs"]
mod catalog_admission;
#[path = "provider_installation/ids.rs"]
mod ids;
#[path = "provider_installation/projected_claims.rs"]
mod projected_claims;
#[path = "provider_installation/scalar_provider.rs"]
mod scalar_provider;
#[path = "provider_installation/structural_provider.rs"]
mod structural_provider;
