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
mod bitwise;
mod call_result_bounds;
mod cast_ranges;
mod dependent_products;
mod dependent_relations;
mod exact_division_definedness;
mod expression_analysis;
mod float_arithmetic;
mod guard_narrowing;
mod integer_ranges;
mod interval;
mod invariant_bounds;
mod monotonic_update;
mod operand_reports;
mod ordered_values;
mod place_paths;
mod range_constraints;
mod return_ranges;
#[cfg(test)]
mod tests;
mod total_specification;
mod unsigned_representability;
mod value_environment;

pub(crate) use assignments::{
    check_narrowing_assignment, check_value_narrowing, enforced_declared_range, record_assignment,
    record_unsigned_literal_assignment,
};
pub(crate) use cast_ranges::record_float_literal_assignment;
pub(crate) use cast_ranges::validate_range_cast_at_use;
pub(crate) use float_arithmetic::float_source_proves_int_cast;
pub use guard_narrowing::arrival_integer_expression_bounds;
pub(crate) use guard_narrowing::{
    fall_through_narrowed_environment, guard_narrowed_environment, incoming_guard_environments,
    requires_value_environment, seed_out_param_ensures,
};
pub use integer_ranges::integer_widen_is_total;
pub(crate) use integer_ranges::{
    collect_exact_integer_cast_facts, literal_i64, validate_anonymous_integer_range,
    validate_value_range,
};
pub(crate) use interval::Interval;
pub use invariant_bounds::{
    declared_integer_expression_lands, enforced_integer_type_bounds,
    immutable_integer_expression_bounds,
};
pub use monotonic_update::builtin_monotonic_integer_update_bounds;
pub(crate) use operand_reports::{
    is_arithmetic, report_mismatched_width_operands, report_out_of_range_comparison_literal,
};
pub use ordered_values::validate_ordered_requirement_call_totality;
pub(crate) use place_paths::place_path;
pub(crate) use range_constraints::{
    check_range_containment, check_range_under_non_exact_domain, enforced_declared_range_interval,
    range_constraint_interval,
};
pub(crate) use return_ranges::{
    call_return_type, enforce_declared_return_range, enforce_symbolic_range,
    validate_return_value_range,
};
pub(crate) use total_specification::{
    validate_abstract_total_specification_arithmetic,
    validate_machine_total_specification_arithmetic, validate_total_specification_arithmetic,
};
pub(crate) use value_environment::ValueEnvironment;

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

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
    program: &TypedTrees,
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
