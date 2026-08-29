mod canonical;
mod checked_facts;
mod conformances;
mod lifetime_identities;
mod nominal_identities;
mod parameter_contracts;
mod type_identities;

pub(crate) use canonical::{canonical_digest_label, framed_identity};
// These helpers were crate-visible from the former flat module. Keep that
// compatibility surface even when a helper currently has only local callers.
pub(crate) use checked_facts::exactly_one;
#[allow(unused_imports)]
pub(crate) use conformances::{
    ProjectedSelectedConformanceApplication, project_conformance_bounds,
    project_selected_conformance_application, selected_conformance_application_type_reference,
};
#[allow(unused_imports)]
pub(crate) use lifetime_identities::{
    lifetime_binder_ordinal, review_domain_lifetime_label,
    review_lifetime_topology_with_substitutions, substituted_lifetime_binder_ordinal,
};
#[allow(unused_imports)]
pub(crate) use nominal_identities::{
    is_canonical_virtual_toolchain_path, nominal_identity, nominal_owner,
    nominal_owner_from_symbols, provider_requirement_identity, reviewed_package_owns,
    toolchain_source_identity, trait_requirement_identity, trait_requirement_identity_from_symbols,
};
#[allow(unused_imports)]
pub(crate) use parameter_contracts::{
    project_machine_parameter_contract, project_signature_crash_routes, project_type_parameters,
    project_type_parameters_after,
};
#[allow(unused_imports)]
pub(crate) use type_identities::{
    missing_exact_toolchain_type_owner, project_data_field,
    review_signature_type_identity_with_binders,
    review_signature_type_identity_with_binders_and_substitutions,
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
    review_type_identity_with_binders, review_type_identity_with_binders_and_substitutions,
    validate_package_const_binder, validate_package_index_expression,
    validate_package_named_type_leaf, validate_package_type_identity_input,
    validate_package_type_identity_input_inner,
};

#[cfg(test)]
mod tests;
