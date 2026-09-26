//! Optimizer module role: stage group. Structural-catalog fixtures by validation ownership.

mod content_projection;
mod function_catalog;
mod provider_specialization;
mod structural_roots;
mod type_declarations;
mod witnesses;

pub(crate) use content_projection::{
    content_entry_claim, install_content_owner, structural_domain,
};
pub(crate) use function_catalog::structural_result_call_unit;
pub(crate) use provider_specialization::provider_attachment_specialization_unit;
#[allow(unused_imports)] // Retained as part of the shared fixture API.
pub(crate) use structural_roots::service_declarations;
pub(crate) use structural_roots::{
    install_service_catalog, installation_root_service_unit,
    multiple_installation_root_service_unit, provider_service_unit, service_effect_unit,
};
pub(crate) use type_declarations::{
    boolean_structural_field_unit, direct_realization_boolean_structural_field_unit,
    direct_realization_integer_structural_field_unit, structural_case, structural_catalog_unit,
    structural_field, structural_leaf_field, structural_scalar_field_store_unit, structural_type,
};
pub(crate) use witnesses::{
    compressed_trivial_affine_return_unit, compressed_trivial_affine_return_unit_with_prefix,
    explicit_trivial_affine_return_unit,
};
