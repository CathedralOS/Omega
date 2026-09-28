//! Relabel authored uses only from the exact expected-type context available here.

mod assignments;
mod calls;
mod constructors;
mod expressions;
mod patterns;

pub(super) use assignments::{
    relabel_closed_data_uses_in_constants, relabel_closed_data_uses_in_exact_assignments,
};
pub(super) use calls::relabel_closed_data_uses_in_exact_calls_and_returns;
pub(super) use constructors::{
    closed_constructor_carrier, constructor_carrier_span, constructor_path_name, expected_instance,
    relabel_data_literal_for_expected_type, selected_case_value, selected_constructor,
};
pub(super) use expressions::{
    ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
    concrete_machine_expression_handles, concrete_machine_state_handles,
};
pub(super) use patterns::{
    closed_sum_path, named_type_name, relabel_closed_sum_memberships_from_local_types,
    relabel_unique_closed_sum_paths,
};
