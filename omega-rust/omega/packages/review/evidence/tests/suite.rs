//! Every integration test of this crate as one binary: each topic file is a module here,
//! shared support modules are declared once, and `cargo nextest run -p <crate> --test suite`
//! runs them all. A new `tests/<topic>.rs` joins by one `mod` line below.

mod support;

mod authority;
mod boundary_application_policy;
mod boundary_supply;
mod callable_policy;
mod calling_policy_lifetimes;
mod calling_policy_opaque;
mod calling_policy_source;
mod calling_policy_substitution;
mod conformance;
mod conformance_policy_source;
mod conformance_policy_values;
mod contract_expressions;
mod exact_contract_identity;
mod external_supply_policy;
mod obligation_ledger;
mod operational;
mod operators;
mod package_policy;
mod package_review_row_recovery;
mod proposition_contracts;
mod public_api;
mod representation_policy;
mod selected_provider_policy;
mod source_custody;
mod terminal_permission_policy;
mod trait_contracts;
