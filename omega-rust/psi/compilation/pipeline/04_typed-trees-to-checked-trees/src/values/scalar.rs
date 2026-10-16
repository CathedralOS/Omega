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
//! `subslice_endpoints.rs` lowers range endpoints for every site that narrows
//! a view, and
//! `expression_facts.rs` answers the questions they share. The remaining
//! modules own array constructions, call arguments, case membership,
//! constant array projection, contract entries, primitive
//! reference reads, result contracts, selected-operator operands, semantic
//! casts and structural fields. The computation graph builder itself lives
//! in `execution::scalar_computations`, the one plan builder producing
//! scalar and Unit computations; these members stay `pub(crate)` so that
//! builder consumes the same lowering utilities the pure plans use.

pub(crate) mod array_constructions;
pub(crate) mod atomic_operands;
pub(crate) mod boolean_lowering;
pub(crate) mod call_arguments;
pub(crate) mod call_lowering;
pub(crate) mod case_membership;
pub(crate) mod constant_array_projection;
pub(crate) mod contract_entry;
pub(crate) mod expression_facts;
pub(crate) mod expression_plans;
pub(crate) mod machine_parameter_booleans;
pub(crate) mod primitive_reference_read;
pub(crate) mod result_contract;
pub(crate) mod scalar_lowering;
pub(crate) mod selected_operator_operands;
pub(crate) mod semantic_casts;
pub(crate) mod structural_fields;
pub(crate) mod subslice_endpoints;
pub(crate) use subslice_endpoints::subslice_endpoints;

pub(crate) use boolean_lowering::evaluate_closed_boolean_expression;
pub(crate) use call_arguments::is_scalar_return_call;
pub(crate) use call_arguments::{nested_structural_call_return_type, retain_nested_call_arguments};
pub(crate) use contract_entry::{
    lower_entry_predicate, lower_entry_scalar, lower_machine_entry_boolean_expression,
    lower_machine_entry_crash_contract_expression, lower_operator_crash_contract_expression,
    lower_signature_crash_contract_expression,
};
pub(crate) use expression_facts::{
    occupies_scalar_position, operator_is_builtin, parameter_position, scalar_expression_type,
    zeroed_scalar_local_initializer,
};
pub(crate) use expression_plans::{
    assignment_target_primitive_type, build_checked_scalar_expression_plans,
};
pub(crate) use machine_parameter_booleans::{
    lower_machine_parameter_boolean_expression, lower_parameter_boolean_expression,
    lower_parameter_scalar_expression,
};
pub(crate) use result_contract::{
    lower_callable_parameter_range_requirements, lower_callable_scalar_contract_predicate,
    lower_scalar_contract_predicate, lower_scalar_parameter_refinement_predicates,
    lower_state_declared_range_predicate, lower_state_domain_predicate,
    lower_state_scalar_contract_predicate,
};
pub(crate) use scalar_lowering::{
    landed_for_primitive, lower_state_scalar_expression, lower_unit_scalar_argument,
    lower_unit_scalar_argument_for_places, unit_scalar_binding_symbols,
};
pub(crate) use structural_fields::{
    exclusive_reference, resolve_structural_parameter_path, structural_data,
    structural_parameter_place,
};
