//! Pure scalar plans describe payload computation; computation graphs additionally
//! retain call, selection, and semantic-cast occurrences. A same-carrier cast can
//! still change declared meaning, so its source binding belongs to the graph,
//! even when the payload tree could otherwise fold it away. Independent lowering
//! replays that occurrence before emitting the qualification-change edge.
//! Call operands are collected independently of the enclosing result category:
//! scalar arguments still need exact occurrences when the callee returns custody.
//! Recording operands does not admit the call or establish its result frontier.
//!
//! `expression_plans.rs` drives the plan walk; `scalar_lowering.rs`,
//! `boolean_lowering.rs`, `call_lowering.rs` and
//! `machine_parameter_booleans.rs` lower each expression family, and
//! `expression_facts.rs` answers the questions they share. The remaining
//! modules own array constructions, call arguments, case membership,
//! computations, constant array projection, contract entries, primitive
//! reference reads, result contracts, semantic casts and structural fields.

mod array_constructions;
mod boolean_lowering;
mod call_arguments;
mod call_lowering;
mod case_membership;
mod computations;
mod constant_array_projection;
mod contract_entry;
mod expression_facts;
mod expression_plans;
mod machine_parameter_booleans;
mod primitive_reference_read;
mod result_contract;
mod scalar_lowering;
mod semantic_casts;
mod structural_fields;
#[cfg(test)]
mod tests;

pub(crate) use array_constructions::{CallArrayConstruction, call_array_constructions};
pub(crate) use boolean_lowering::evaluate_closed_boolean_expression;
pub(crate) use call_arguments::is_scalar_return_call;
pub(crate) use call_arguments::{
    nested_structural_call_return_type, retain_nested_structural_call_arguments,
};
#[cfg(test)]
pub(crate) use call_lowering::scalar_qualified_call_expression;
#[cfg(test)]
pub(crate) use computations::build_checked_scalar_computation_plans;
pub(crate) use computations::build_checked_value_computation_plans;
pub(crate) use contract_entry::{
    lower_machine_entry_boolean_expression, lower_machine_entry_crash_contract_expression,
    lower_operator_crash_contract_expression, lower_signature_crash_contract_expression,
};
pub(crate) use expression_facts::{
    occupies_scalar_position, operator_is_builtin, scalar_expression_type,
};
pub(crate) use expression_plans::build_checked_scalar_expression_plans;
pub(crate) use machine_parameter_booleans::lower_machine_parameter_boolean_expression;
pub(crate) use result_contract::{
    lower_integer_parameter_range_requirements, lower_scalar_contract_predicate,
    lower_scalar_parameter_range_requirements, lower_state_scalar_contract_predicate,
};
pub(crate) use scalar_lowering::{
    lower_state_scalar_expression, lower_unit_scalar_argument, retag_exact_integer_literal,
};
pub(crate) use structural_fields::{exclusive_reference, resolve_structural_parameter_path};
