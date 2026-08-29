//! Canonical semantic-value encoders, grouped by the value families they encode.

mod callables;
mod contracts;
mod crashes;
mod declarations;
mod effects;
mod expressions;
mod identity;
mod providers;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use callables::{
    encode_callable, encode_callable_conformance, encode_external_executable_supply,
    encode_external_executable_supply_key,
};
#[allow(unused_imports)]
pub(crate) use contracts::{
    encode_callable_contract, encode_contract_fact, encode_proposition_application,
};
#[allow(unused_imports)]
pub(crate) use crashes::{
    encode_boolean_expression, encode_crash, encode_crash_call, encode_crash_predicate,
    encode_crash_route, encode_crash_site, encode_permission_claim, encode_primitive_type,
    encode_scalar_expression, encode_structural_field, encode_structural_path, integer_binary_tag,
    integer_comparison_tag,
};
#[allow(unused_imports)]
pub(crate) use declarations::{
    encode_const_shape, encode_evidence_interface, encode_operator_coordinate,
    encode_operator_shape, encode_proposition_binder, encode_proposition_shape,
    operator_spelling_tag,
};
#[allow(unused_imports)]
pub(crate) use effects::{
    encode_capability_flow, encode_installation_reach, encode_mutation,
    encode_synchronous_invocation, encode_termination,
};
#[allow(unused_imports)]
pub(crate) use expressions::{
    encode_contract_expression, encode_contract_operator_meaning, encode_contract_static_argument,
};
#[allow(unused_imports)]
pub(crate) use identity::{encode_nominal, encode_supply};
#[allow(unused_imports)]
pub(crate) use providers::{
    encode_carry_policy, encode_provider, encode_provider_family, encode_provider_row,
    encode_service_schema,
};
