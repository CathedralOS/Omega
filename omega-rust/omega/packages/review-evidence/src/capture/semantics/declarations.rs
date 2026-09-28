//! Canonical identities for checked declarations: nominal identity and
//! ownership, provider and trait requirement identities, policy requirement
//! identities and the declaring schema of a provider requirement.

mod identity;
mod ownership;
mod policy_requirements;
mod provider_schema;
mod requirements;

pub(crate) use identity::{nominal_identity, nominal_identity_from_symbols};
#[cfg(test)]
pub(crate) use ownership::nominal_owner_from_symbols;
pub(crate) use ownership::{
    is_canonical_virtual_toolchain_path, is_product_scope_instance, nominal_owner,
    reviewed_package_owns, toolchain_source_identity,
};
pub(crate) use policy_requirements::policy_provider_requirement_identity;
pub(crate) use provider_schema::provider_requirement_schema;
pub(crate) use requirements::{
    provider_requirement_identity, top_level_requirement_identity, trait_requirement_identity,
    trait_requirement_identity_from_symbols,
};
