//! Arithmetic-domain checks (frozen decision 17). Two rules, both OPERAND-driven
//! (the domain lives on each value's type, not the assignment target):
//!
//! - **S2 mixed-domain rejection**: a binary arithmetic op whose operands carry
//!   DIFFERENT explicit domains is illegal (cross with an `as` cast). Literals are
//!   neutral and adopt the other operand's domain.
//! - **S3 exact-by-default enforcement**: an `Exact` (default, undomained) integer
//!   `+`/`-`/`*` must be PROVEN not to overflow its type, else it is a compile
//!   error directing the user to widen (`as`) or pick a domain. Wrapping/
//!   Saturating/Trapping ops have defined overflow behaviour and are exempt.
//!
//! Operand ranges come from declared type bounds (an `i32` is its full range),
//! narrowed for literals to their exact value; the interval engine then bounds
//! the result and checks it fits the result type. (Range-constraint and
//! loop-bound narrowing -- the ergonomics that keep this from being annotation-
//! hell -- are S4.)
//!
//! This file validates the arithmetic domains of one machine.
//! `integer_ranges.rs` fits integer literals and casts into primitive
//! ranges, `float_arithmetic.rs` resolves float arithmetic and float
//! sources of integer casts, `place_paths.rs` compares place paths,
//! `assignments.rs` records and narrows assignments, `operand_reports.rs`
//! reports out-of-range and mismatched operands, `range_constraints.rs`
//! derives range constraint intervals and containment and
//! `return_ranges.rs` infers and enforces return ranges.

mod abstract_shift_count;
mod assignments;
mod bigint_interval;
mod bitwise;
mod referent_predicates;
pub(crate) use referent_predicates::referent_predicates_preserved;
mod call_result_bounds;
pub(crate) use call_result_bounds::normal_return_referent_interval;
mod call_result_measures;
pub use call_result_measures::normal_return_length_collection;
mod cast_ranges;
mod dispatch_environments;
pub(crate) use dispatch_environments::dispatch_arm_environments;
mod cited_element_bounds;
mod copied_values;
pub(crate) use copied_values::CopiedIntegerValue;
mod dependent_products;
mod dependent_relations;
mod division_definedness;
mod domain_exclusions;
pub(crate) mod domain_intervals;
mod expression_analysis;
mod float_arithmetic;
mod float_contracts;
pub use float_contracts::{float_return_postcondition, normal_return_float_predicates};
mod guard_narrowing;
mod guard_requirements;
mod integer_carriers;
mod integer_ranges;
pub(crate) use integer_carriers::{admitted_integer, integer_magnitude, integer_type};
mod interval;
mod invariant_bounds;
pub use guard_requirements::guarded_call_requires_proven;
mod live_integer_constraints;
mod monotonic_update;
mod operand_reports;
mod ordered_values;
mod place_paths;
mod range_constraints;
pub(crate) use range_constraints::declares_bracketed_range;
mod return_ranges;
mod selected_scalar_intervals;
pub use bigint_interval::{
    BigIntInterval, bitwise_and_constants, bitwise_carrier_bounds, floor_shift_right,
};
pub use interval::{Interval, WideInterval};
pub use return_ranges::normal_return_integer_bounds;
pub(crate) use selected_scalar_intervals::binary as selected_integer_binary_interval;
pub use selected_scalar_intervals::{
    SelectedScalarIntervalSource, carrier_integer_range, checked_integer_binary_kind,
    frozen_declared_integer_range, integer_range_contains, selected_boolean_decision,
    selected_scalar_interval,
};
mod stored_integer_writes;
pub(crate) use guard_narrowing::{
    OccurrenceValueEnvironments, occurrence_value_environment, occurrence_value_environments,
};
mod total_specification;
mod unsigned_representability;
mod value_environment;

pub(crate) use assignments::{
    capture_float_assignment, check_narrowing_assignment, check_value_narrowing,
    enforced_declared_range, record_assignment, record_float_assignment,
    record_unsigned_literal_assignment, stored_value_carrier,
};
pub(crate) use cast_ranges::validate_range_cast_at_use;
pub use cast_ranges::{plain_exact_widening_cast, unannotated_exact_cast};
pub(crate) use cited_element_bounds::{
    element_place_path, seed_cited_element_bounds, stable_element_place_path,
};
pub(crate) use expression_analysis::ArithmeticDecisions;
pub(crate) use float_arithmetic::float_source_proves_integer_conversion;
pub(crate) use float_arithmetic::{FloatSourceBounds, float_source_bounds};
pub use guard_narrowing::{
    CollectionBoundsObservations, arrival_integer_expression_bounds,
    occurrence_collection_footprint_fits, occurrence_integer_expression_bounds,
    occurrence_integer_expression_fits_range,
};
pub(crate) use guard_narrowing::{
    fall_through_narrowed_environment, guard_narrowed_environment, incoming_guard_environments,
    record_boolean_assignment, record_boolean_local, requires_value_environment,
    seed_out_param_ensures, short_circuit_operand_environment,
};
pub use integer_ranges::integer_widen_is_total;
pub(crate) use integer_ranges::{
    integer_bit_width, integer_source_bounds, literal_i64, primitive_range,
    validate_anonymous_integer_range, validate_value_range,
    validate_value_range_retaining_decisions,
};
pub(crate) use invariant_bounds::{
    call_result_interval, immutable_integer_expression_interval, standing_integer_interval,
    upper_bound_beyond_place_carrier,
};
pub use invariant_bounds::{
    declared_integer_expression_lands, enforced_integer_type_bounds,
    immutable_integer_expression_bounds, standing_integer_bounds, state_parameter_integer_interval,
    stored_integer_bounds,
};
pub use monotonic_update::builtin_monotonic_integer_update_bounds;
pub(crate) use operand_reports::{
    is_arithmetic, report_mismatched_integer_operands, report_out_of_range_comparison_literal,
};
pub use ordered_values::validate_ordered_requirement_call_totality;
pub(crate) use place_paths::place_path;
pub(crate) use range_constraints::{
    check_range_containment, check_range_under_non_exact_domain, check_where_interval_containment,
    enforced_declared_range_interval, range_constraint_interval,
};
pub(crate) use return_ranges::{
    call_return_type, enforce_declared_return_range, enforce_symbolic_range,
    validate_return_value_range,
};
pub(crate) use stored_integer_writes::{
    stored_integer_field_type, stored_integer_value_fits, validate_stored_integer_write,
};
pub use stored_integer_writes::{
    stored_integer_source_may_require_fit, stored_integer_source_projection,
    stored_integer_source_projections,
};
pub(crate) use total_specification::{
    validate_abstract_total_specification_arithmetic,
    validate_machine_total_specification_arithmetic, validate_total_specification_arithmetic,
};
pub(crate) use value_environment::ValueEnvironment;

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use crate::value_custody::places::declared_place_type_raw;

/// Walk a value expression, apply the domain + overflow rules to every nested
/// arithmetic binary, and return the expression's proven interval (so the caller
/// can record it for the assigned place). `owner` describes the site.
/// `target_primitive` is the declared integer type of the value's destination
/// (the local/field/return type). It is the FALLBACK integer type for an
/// otherwise-untyped operand tree -- so a bare-literal computation like
/// `let c: u8 = 200 + 100` is range-checked against `u8` (and rejected) instead
/// of slipping through with no primitive.
/// `target_domain` is the destination's declared arithmetic domain. When NEITHER
/// operand of a binary carries a domain, the operation computes INTO the
/// destination and adopts its domain -- the backend already selects the op by
/// the target's domain, so `let v: i32 in Wrapping = t + 100` (t a plain local)
/// is wrapping arithmetic, not an Exact obligation. `Exact` (the default) keeps
/// the S3 proof obligation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_arithmetic_domains(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Interval {
    validate_value_range(
        program,
        machine,
        state,
        expression,
        environment,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    )
    .0
}

/// Run `check` on every `Binary` node whose operator is in `operators`,
/// post-order — the shared scaffold behind the per-domain shift-count and
/// division-definedness validations.
pub(super) fn visit_binary_operators(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    operators: &[typed_trees::expression::BinaryOperator],
    mut check: impl FnMut(ExpressionHandle, &typed_trees::expression::TableBinaryExpression),
) {
    let mut visit = |expression, node: &typed_trees::expression::ExpressionNode| {
        let typed_trees::expression::ExpressionNode::Binary(binary) = node else {
            return;
        };
        if operators.contains(&binary.operator) {
            check(expression, binary);
        }
    };
    crate::expression_traversal::visit_post_order(program, expression, &mut Vec::new(), &mut visit);
}
