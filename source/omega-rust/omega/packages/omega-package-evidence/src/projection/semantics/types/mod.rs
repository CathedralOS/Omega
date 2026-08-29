//! Canonical package-review identities for checked types and data fields.

mod fields;
mod identity;
pub(in crate::projection) mod lifetimes;
mod properties;
mod validation;

pub(crate) use fields::project_data_field;
pub(crate) use identity::{
    review_signature_type_identity_with_binders,
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
    review_type_identity_with_binders, review_type_identity_with_binders_and_substitutions,
};
pub(crate) use properties::project_data_properties;
pub(crate) use validation::missing_exact_toolchain_type_owner;
#[cfg(test)]
pub(crate) use validation::{
    validate_package_type_identity_input, validate_package_type_identity_input_inner,
};
