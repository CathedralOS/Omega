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

use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

use crate::places::declared_place_type_raw;

mod abstract_shift_count;
mod dependent_products;
mod dependent_relations;
mod exact_division_definedness;
mod expression_analysis;
mod guard_narrowing;
mod interval;
mod total_specification;
mod value_environment;

use dependent_products::{refine_dependent_product, refine_dependent_product_factor};
use dependent_relations::refine_dependent_subtract;
use expression_analysis::analyze;
pub(crate) use guard_narrowing::{
    fall_through_narrowed_env, guard_narrowed_env, incoming_guard_env, requires_value_env,
    seed_out_param_ensures,
};
pub(crate) use interval::Interval;
pub(crate) use total_specification::{
    validate_abstract_total_specification_arithmetic,
    validate_machine_total_specification_arithmetic, validate_total_specification_arithmetic,
};
use value_environment::FloatInterval;
pub(crate) use value_environment::ValueEnv;

/// Resolve only the reserved named-float arithmetic surface. Contract
/// expressions can reach validation before their static namespace receiver has
/// a symbol, so use the retained one-segment `F32`/`F64` spelling as a strict
/// fallback. The shared resolver still enforces exact path, arity, and
/// uniqueness; arbitrary float-returning operators do not enter this lane.
fn resolve_named_float_arithmetic<'program>(
    program: &'program TypedTrees,
    call: &TableCallExpression,
) -> Option<(
    &'program psi_typed_trees::operator::OperatorDefinition,
    PrimitiveType,
)> {
    let operator = psi_typed_trees::operator::resolve_named_expression_call(program, call)
        .or_else(|| {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [namespace] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !matches!(namespace.as_str(), "F32" | "F64") {
                return None;
            }
            let static_receiver = [namespace.as_str()];
            psi_typed_trees::operator::resolve_named_call(
                program,
                call.target_symbol,
                Some(&static_receiver),
                call.target.as_str(),
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
                false,
            )
        })?;
    let primitive = program.primitive_type_reference(operator.return_type)?;
    let namespace = program.operator_path_members(operator.name).first()?;
    matches!(
        (namespace.as_str(), primitive),
        ("F32", PrimitiveType::F32) | ("F64", PrimitiveType::F64)
    )
    .then_some((operator, primitive))
}

fn canonical_path_pair(left: String, right: String) -> (String, String) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn place_paths_overlap(left: &str, right: &str) -> bool {
    crate::calls::frame_paths_overlap(left, right)
}

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
    env: &ValueEnv,
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
        env,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    )
    .0
}

/// Like [`validate_arithmetic_domains`] but also returns the expression's source
/// integer primitive (the `None`-for-unknown result). The narrowing check needs
/// it: a value produced by a typed source is ALWAYS within that type's range (a
/// `u32 in Wrapping` sum is a u32 even when its mathematical interval spills past
/// `u32`), so the sound value range is `interval ∩ primitive_range(source)` --
/// intersecting keeps a flow-proven tighter interval while clamping a
/// domain-wrapped over-approximation back to the type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Interval, Option<PrimitiveType>) {
    if !expression.is_valid() {
        return (Interval::UNBOUNDED, None);
    }
    let analysis = analyze(
        program,
        machine,
        state,
        expression,
        env,
        target_primitive,
        target_domain,
        owner,
        diagnostics,
    );
    (analysis.interval, analysis.primitive)
}

/// Retain every accepted occurrence-dependent exact fixed-integer cast under
/// the same flow environment used by validation. This is a query over the
/// validator's judgment, not a second range engine: operand intervals are
/// obtained by `validate_value_range`, and only fully bounded intervals that
/// fit the target are published. Later checked lowering can therefore discard
/// `ValueEnv` without turning validation success into ambient trust.
pub(crate) fn collect_exact_integer_cast_facts(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    facts: &mut Vec<crate::ExactIntegerCastFact>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            if !cast.form.is_recast()
                && cast.semantic_domain.is_empty()
                && cast.domain == ArithmeticDomain::Exact
                && let Some(target_type) = program.primitive_type_reference(cast.target_type)
                && target_type != PrimitiveType::Addr
            {
                let mut diagnostics = Vec::new();
                let (interval, source_type) = validate_value_range(
                    program,
                    machine,
                    state,
                    cast.value,
                    env,
                    None,
                    ArithmeticDomain::Exact,
                    "checked exact integer cast evidence",
                    &mut diagnostics,
                );
                if diagnostics.is_empty()
                    && let Some(source_type) = source_type
                    && source_type != target_type
                    && source_type != PrimitiveType::Addr
                    && primitive_range(source_type).is_some()
                    && primitive_range(target_type).is_some()
                    && integer_interval_fits_primitive(interval, source_type, target_type)
                {
                    let source_range = primitive_range(source_type)
                        .expect("fixed integer source has a primitive range");
                    let effective = if source_range.contains(interval) {
                        interval
                    } else {
                        source_range
                    };
                    if let (Some(minimum), Some(maximum)) = (effective.low, effective.high) {
                        facts.push(crate::ExactIntegerCastFact {
                            expression,
                            source_type,
                            target_type,
                            minimum: psi_numerics::bignum::BigInt::from_i64(minimum),
                            maximum: psi_numerics::bignum::BigInt::from_i64(maximum),
                        });
                    }
                }
            }
            collect_exact_integer_cast_facts(program, machine, state, cast.value, env, facts);
        }
        ExpressionNode::Atomic(atomic) => {
            collect_exact_integer_cast_facts(program, machine, state, atomic.value, env, facts);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_exact_integer_cast_facts(program, machine, state, *value, env, facts);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_exact_integer_cast_facts(program, machine, state, binary.left, env, facts);
            collect_exact_integer_cast_facts(program, machine, state, binary.right, env, facts);
        }
        ExpressionNode::Call(call) => {
            collect_exact_integer_cast_facts(program, machine, state, call.receiver, env, facts);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_exact_integer_cast_facts(program, machine, state, *argument, env, facts);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_exact_integer_cast_facts(
                program,
                machine,
                state,
                indexed.collection,
                env,
                facts,
            );
            collect_exact_integer_cast_facts(program, machine, state, indexed.index, env, facts);
        }
        ExpressionNode::Member(member) => {
            collect_exact_integer_cast_facts(program, machine, state, member.receiver, env, facts);
        }
        ExpressionNode::Borrow(value) => {
            collect_exact_integer_cast_facts(program, machine, state, value.target, env, facts);
        }
        ExpressionNode::Range(range) => {
            collect_exact_integer_cast_facts(program, machine, state, range.start, env, facts);
            collect_exact_integer_cast_facts(program, machine, state, range.end, env, facts);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_exact_integer_cast_facts(program, machine, state, field.value, env, facts);
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_exact_integer_cast_facts(program, machine, state, unary.operand, env, facts);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// The straight-line place path an expression denotes (`self.v`, `count`), for
/// the value environment. `None` for non-place expressions.
pub(crate) fn place_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.is_empty() {
                return None;
            }
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = place_path(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        _ => None,
    }
}

/// Record an assignment's proven interval into the environment (decision 17 S4).
/// A place whose path cannot be formed (a complex lvalue) just is not tracked.
/// The interval is INTERSECTED with the place's declared `[a..=b]` range before
/// recording: an env entry SHADOWS the declared-range fallback in the operand
/// analysis, so recording an UNBOUNDED interval (an unresolvable initializer)
/// onto a range-declared place would WIDEN its effective range -- the same
/// landmine as the guard-seeding one (`let __hoist: i32 [0..=9] = cells[k].v`
/// recorded unbounded and `__hoist + 5` "may overflow").
pub(crate) fn record_assignment(
    env: &mut ValueEnv,
    path: Option<String>,
    interval: Interval,
    declared_range: Option<Interval>,
) {
    if let Some(path) = path {
        env.known_u64_values
            .retain(|known, _| !place_paths_overlap(known, &path));
        env.joint_add_upper_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        env.joint_add_lower_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        env.joint_subtract_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        env.signed_joint_subtract_lower_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        env.signed_joint_subtract_upper_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        env.joint_multiply_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        env.signed_joint_multiply_lower_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        env.signed_joint_multiply_upper_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        env.signed_joint_multiply_negation_bounds
            .retain(|value| !place_paths_overlap(value, &path));
        let interval = match declared_range {
            Some(declared) => interval.intersect(declared),
            None => interval,
        };
        env.set(path, interval);
    }
}

/// The declared `[a..=b]` range of a place's type, ONLY when that range is
/// store-enforced (EXACT arithmetic domain; atomics wrap by hardware): the
/// interval an env entry may soundly be intersected with. A `in Wrapping`
/// place can legitimately hold out-of-range values (its declared range is
/// deliberately permissive at stores), so clamping its precise env fact
/// against the range would fabricate an in-range claim -- return None there.
pub(crate) fn enforced_declared_range(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    if is_atomic_type(program, handle)
        || program.arithmetic_domain_for_type_reference(handle) != ArithmeticDomain::Exact
    {
        return None;
    }
    range_constraint_interval(program, handle)
}

/// Decision 17 at a VALUE-BINDING boundary (`self.f = v`, `let x: T = v`): a
/// value whose proven range does not fit the destination integer type is a
/// SILENT NARROWING (truncation). Storing a wider value into a narrower slot is
/// the same proof obligation as an overflowing `+` -- prove it fits or opt into
/// an explicit `as` cast. Flagged ONLY when the source interval is fully bounded
/// AND provably escapes the target range: an unbounded end (a call result, a
/// param, a `u64` high) stays permissive, exactly as exact arithmetic leaves
/// unbounded unknowns unchecked. A `Cast` re-ranges to the target (its interval
/// is the target's own range), so `v as i32` always fits -- the escape hatch.
/// Signedness falls out for free: `i32 -> u32` is caught on the negative half,
/// `u32 -> i32` on the upper half. `target` is `None` (non-primitive or
/// non-integer destination) => nothing to prove.
pub(crate) fn check_narrowing_assignment(
    target: Option<PrimitiveType>,
    value: Interval,
    source: Option<PrimitiveType>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(primitive) = target else {
        return;
    };
    let Some(range) = primitive_range(primitive) else {
        return;
    };
    // The value produced by a typed source is always WITHIN that type's range
    // (even a `Wrapping`/`Saturating` result), so intersect the mathematical
    // interval with the source type's range: a flow-proven `[7, 7]` survives
    // (tighter than the type), while a domain-wrapped over-approximation is
    // clamped back to the source type -- which, if it fits the target, is no
    // narrowing at all.
    let effective = match source.and_then(primitive_range) {
        Some(source_range) => value.intersect(source_range),
        None => value,
    };
    // Only a fully-bounded source can be PROVEN out of range; leave an unbounded
    // (unknown) end permissive rather than turn "unknown" into a spurious error.
    if effective.low().is_some() && effective.high().is_some() && !range.contains(effective) {
        diagnostics.push(Diagnostic::error(format!(
            "narrowing store in {owner} may not fit `{}`: the value is not provably \
             in range (decision 17 -- a narrowing store is a proof obligation, like exact \
             arithmetic). Truncate explicitly with an `as` cast, or constrain the source's \
             range.",
            primitive_name(primitive),
        )));
    }
}

/// Report the decision-17 narrowing-store obligation for a `value` flowing into a
/// `target` scalar slot: analyze the value's interval (honoring the flow facts in
/// `env`) and flag it if it may not fit the target's width. The value's OWN
/// arithmetic obligations are reported by the normal statement walk, so they go to
/// a THROWAWAY buffer here -- only the narrowing check contributes to `diagnostics`.
/// SINGLE SOURCE OF TRUTH for the "does this value fit its typed scalar slot?"
/// obligation, shared by every store position: call/transition arguments,
/// struct-literal field construction, and array-literal elements. Pass the
/// statement `value_env` for flow-sensitive positions, or `&ValueEnv::new()` where
/// no per-statement env is threaded (construction).
pub(crate) fn check_value_narrowing(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut throwaway = Vec::new();
    let (interval, source) = validate_value_range(
        program,
        machine,
        state,
        value,
        env,
        Some(target),
        ArithmeticDomain::Exact,
        owner,
        &mut throwaway,
    );
    if throwaway.is_empty() {
        check_narrowing_assignment(Some(target), interval, source, owner, diagnostics);
    }
}

/// The interval of an anonymous literal (D14). A literal that fits i64 is a
/// point; an oversize u64-magnitude literal is honestly over-approximated as
/// "above i64::MAX" (or "below i64::MIN" for its folded negation), so
/// narrowing checks against i64-bounded targets still REJECT by interval
/// math alone -- never by silently skipping.
fn literal_interval(literal: &psi_numerics::literals::IntegerLiteral) -> Interval {
    match literal.value_i64() {
        Some(value) => Interval::constant(value),
        None if !literal.text().starts_with('-') => Interval {
            low: Some(i64::MAX),
            high: None,
        },
        None => Interval {
            low: None,
            high: Some(i64::MIN),
        },
    }
}

/// The representable range of an integer primitive. `None` for non-integers
/// (`bool`/`f32`/`f64`/`String`) and for `u64`/`usize` whose maximum exceeds
/// `i64` (their high end is left unbounded -- an over-approximation that still
/// rejects genuine overflow).
fn primitive_range(primitive: PrimitiveType) -> Option<Interval> {
    let (low, high): (Option<i64>, Option<i64>) = match primitive {
        PrimitiveType::I8 => (Some(i8::MIN as i64), Some(i8::MAX as i64)),
        PrimitiveType::U8 => (Some(0), Some(u8::MAX as i64)),
        PrimitiveType::I16 => (Some(i16::MIN as i64), Some(i16::MAX as i64)),
        PrimitiveType::U16 => (Some(0), Some(u16::MAX as i64)),
        PrimitiveType::I32 => (Some(i32::MIN as i64), Some(i32::MAX as i64)),
        PrimitiveType::U32 => (Some(0), Some(u32::MAX as i64)),
        PrimitiveType::I64 => (Some(i64::MIN), Some(i64::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (Some(0), None),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    Some(Interval { low, high })
}

/// Whether every value admitted by `source` is representable by the target
/// integer carrier. Unbounded endpoints compare structurally: an unbounded
/// source side is accepted only when the target is unbounded on that side too.
fn integer_interval_fits_primitive(
    source: Interval,
    source_primitive: PrimitiveType,
    target: PrimitiveType,
) -> bool {
    let Some(source_range) = primitive_range(source_primitive) else {
        return false;
    };
    let Some(target) = primitive_range(target) else {
        return false;
    };
    // Every runtime value is already constrained by its source carrier. Keep a
    // tighter flow interval only when it is wholly inside that carrier. A
    // wrapped expression's mathematical interval can lie outside the carrier;
    // intersecting a disjoint interval would manufacture an empty range and
    // falsely prove arbitrary targets, so fall back to the full carrier.
    let source = if source_range.contains(source) {
        source
    } else {
        source_range
    };
    let low_fits = match (source.low, target.low) {
        (_, None) => true,
        (Some(source), Some(target)) => source >= target,
        (None, Some(_)) => false,
    };
    let high_fits = match (source.high, target.high) {
        (_, None) => true,
        (Some(source), Some(target)) => source <= target,
        (None, Some(_)) => false,
    };
    low_fits && high_fits
}

/// The bit width of an integer primitive (the F8 shift-count bound), or `None`
/// for non-integer types. `Addr` is excluded deliberately: shifting an address
/// is already rejected by the address-arithmetic model above.
fn integer_bit_width(primitive: PrimitiveType) -> Option<i64> {
    match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(8),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(16),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(32),
        PrimitiveType::I64 | PrimitiveType::U64 => Some(64),
        PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

/// Prove an Exact multiplication total when both operands are value-preserving
/// unsigned widenings and the sum of their source widths fits the common target
/// carrier. The ordinary interval lattice is i64-backed, so it cannot express
/// the complete u64 upper endpoint even though `u32::MAX * u32::MAX` is
/// representable by u64. This structural proof closes only that representation
/// gap; arbitrary casts and mixed targets fail closed.
fn exact_unsigned_widened_multiply_fits(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let operand = |expression| -> Option<(i64, PrimitiveType)> {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
            return None;
        };
        if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            || cast.domain != ArithmeticDomain::Exact
        {
            return None;
        }
        let source_type = declared_place_type_raw(program, machine, state, cast.value)?;
        let source = program.primitive_type_reference(source_type)?;
        let target = program.primitive_type_reference(cast.target_type)?;
        if !matches!(
            source,
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32
        ) || !matches!(
            target,
            PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
        ) {
            return None;
        }
        let source_width = integer_bit_width(source)?;
        let target_width = integer_bit_width(target)?;
        (source_width < target_width).then_some((source_width, target))
    };
    let Some((left_width, left_target)) = operand(left) else {
        return false;
    };
    let Some((right_width, right_target)) = operand(right) else {
        return false;
    };
    left_target == right_target
        && integer_bit_width(left_target).is_some_and(|width| left_width + right_width <= width)
}

fn u64_exact_shift_left_fits(value: Interval, count: Interval) -> bool {
    let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
        (value.low, value.high, count.low, count.high)
    else {
        return false;
    };
    if value_low < 0 || count_low < 0 || count_high >= 64 {
        return false;
    }
    u128::try_from(value_high)
        .ok()
        .and_then(|value| value.checked_shl(count_high as u32))
        .is_some_and(|maximum| maximum <= u128::from(u64::MAX))
}

/// The float value of a literal operand (through a `Mutable` wrapper), read at
/// its landed format, or `None` when the operand is not a plain float literal.
/// The F4 Exact cast obligation's fold-visible proof source.
fn float_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<f64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Borrow(inner) = node {
        node = program.expression_table.expression(inner.target);
    }
    match node {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => None,
    }
}

pub(crate) fn float_source_proves_int_cast(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> bool {
    let (range, non_nan) = if let Some(literal) = float_literal_value(program, value) {
        (
            FloatInterval {
                low: Some(literal),
                high: Some(literal),
            },
            !literal.is_nan(),
        )
    } else {
        let Some(path) = place_path(program, value) else {
            return false;
        };
        let (flow_range, flow_non_nan) = env.float_fact(&path);
        let declared = declared_place_type_raw(program, machine, state, value)
            .and_then(|handle| float_range_constraint_interval(program, handle));
        let range = declared
            .map(|declared| declared.intersect(flow_range))
            .unwrap_or(flow_range);
        let declared_finite = declared.is_some_and(|declared| {
            declared.low.is_some_and(f64::is_finite) && declared.high.is_some_and(f64::is_finite)
        });
        (range, flow_non_nan || declared_finite)
    };
    if !non_nan {
        return false;
    }
    let (Some(low), Some(high)) = (range.low, range.high) else {
        return false;
    };
    if !low.is_finite() || !high.is_finite() || low > high {
        return false;
    }
    float_interval_fits_integer(low.trunc(), high.trunc(), target)
}

fn float_interval_fits_integer(low: f64, high: f64, target: PrimitiveType) -> bool {
    // The i64/u64 upper endpoints need strict comparisons: `i64::MAX as f64`
    // rounds to 2^63, and 2^64 itself is likewise outside u64 even though it
    // is exactly representable as a float. Smaller integer endpoints are all
    // exactly representable in f64 and may use the ordinary closed interval.
    match target {
        PrimitiveType::I64 => low >= -9223372036854775808.0 && high < 9223372036854775808.0,
        PrimitiveType::U64 => low >= 0.0 && high < 18446744073709551616.0,
        _ => primitive_range(target).is_some_and(|range| {
            matches!((range.low, range.high), (Some(target_low), Some(target_high))
                if low >= target_low as f64 && high <= target_high as f64)
        }),
    }
}

/// The integer value of a literal operand (through a `Mutable` wrapper), or `None`
/// when the operand is not a plain integer literal.
fn integer_literal_value(program: &TypedTrees, value: ExpressionHandle) -> Option<i64> {
    let mut node = program.expression_table.expression(value);
    while let ExpressionNode::Borrow(inner) = node {
        node = program.expression_table.expression(inner.target);
    }
    match node {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}

/// Reject a comparison (`==`/`!=`/`<`/`<=`/`>`/`>=`) between an integer-typed value
/// and an integer LITERAL outside that type's range: `self.b == 300` for a `u8` `b`
/// silently TRUNCATED the literal to the operand width (`300 & 0xFF == 44`) and
/// compared `b == 44` -- a confirmed miscompile (native took the `== 44` branch).
/// A literal compared against a value must be a representable value of that value's
/// type. Fires only when one operand resolves to an integer primitive and the other
/// is an integer literal outside its range; two-place, float/bool/text, and in-range
/// pairings are skipped. Sibling of the decision-17 narrowing obligation for stores.
pub(crate) fn report_out_of_range_comparison_literal(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return false;
    }
    for (typed_operand, literal_operand) in [(left, right), (right, left)] {
        let Some(primitive) =
            crate::places::declared_place_type(program, machine, state, typed_operand)
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
        else {
            continue;
        };
        let Some(range) = primitive_range(primitive) else {
            continue;
        };
        let Some(literal) = integer_literal_value(program, literal_operand) else {
            continue;
        };
        let in_range = range.low().is_none_or(|low| literal >= low)
            && range.high().is_none_or(|high| literal <= high);
        if !in_range {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` compares a `{}` value against `{literal}`, which is out of \
                 range for `{}` -- the comparison would silently truncate the literal to the \
                 operand width; compare an in-range value or widen the value with an `as` cast",
                machine.name.as_str(),
                state.map(|state| state.name.as_str()).unwrap_or(""),
                primitive.name(),
                primitive.name(),
            )));
            return true;
        }
    }
    false
}

/// Reject a COMPARISON (`==`/`!=`/`<`/`<=`/`>`/`>=`) or BITWISE (`&`/`|`/`^`)
/// operation between two integer places of DIFFERENT primitive types
/// (`self.i8 == self.i32`, `self.u32 | self.u8`): the backend performs it at the
/// NARROWER operand's width, silently truncating the wider one -- `i8(44) == i32(300)`
/// reads TRUE (`300 & 0xFF == 44`) and `u32(256) | u8(1)` reads `1` not `257` (both
/// confirmed native). Two integer operands must be the SAME type; convert one with an
/// `as` cast. Fires only when BOTH operands resolve to integer primitives
/// (`primitive_range` is `Some`) that differ. NOT arithmetic (`+ - * / %`, whose
/// mismatch is caught by the decision-17 overflow obligation) nor SHIFT (whose right
/// operand is a bit COUNT, not a width-matched value). A literal operand (no declared
/// place type -- handled by the out-of-range check), a float/bool/text operand, and
/// same-type operands are all skipped. Sibling of
/// `report_out_of_range_comparison_literal`.
pub(crate) fn report_mismatched_width_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
    ) {
        return false;
    }
    let operand_integer = |operand| {
        crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .filter(|primitive| primitive_range(*primitive).is_some())
    };
    let (Some(left_primitive), Some(right_primitive)) =
        (operand_integer(left), operand_integer(right))
    else {
        return false;
    };
    if left_primitive == right_primitive {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` applies `{operator:?}` to a `{}` value and a `{}` value -- the \
         operands have different integer types and the operation would silently truncate the wider \
         one to the narrower width; convert one with an `as` cast so both are the same type",
        machine.name.as_str(),
        state.map(|state| state.name.as_str()).unwrap_or(""),
        left_primitive.name(),
        right_primitive.name(),
    )));
    true
}

/// The operators whose result is genuine integer arithmetic and can therefore
/// exceed the `{0, 1}` range even when the operands are bools (bool feeds in as
/// its 0/1 value). Excludes bitwise `& | ^` (which preserve `{0, 1}` for `{0, 1}`
/// operands) and comparison/logical ops (which yield a bool). Used both for the
/// overflow analysis here and, via `expression_types`, to classify an arithmetic
/// result as numeric for the cross-class store check.
/// The source spelling of an arithmetic operator, for diagnostics.
fn arithmetic_operator_spelling(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::ShiftLeft => "<<",
        BinaryOperator::ShiftRight => ">>",
        _ => "?",
    }
}

pub(crate) fn is_arithmetic(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

/// S4: the value range a type declares via a `Range` constraint (`x: i32 [0..N]`),
/// so a bounded value's exact arithmetic can be proven in-range instead of using
/// the full type width. Inclusive bounds (a sound over-approximation either way).
/// `None` when the type has no literal range constraint. Looks through reference
/// shells.
/// `range-constraints-require-exact-domain` (Zach, 2026-07-13: "this is just a
/// compile error"): a declared
/// RANGE constraint combined with a non-Exact arithmetic domain is
/// ill-formed. The range is only enforced under Exact stores, so
/// `u8 [0..=4] in Wrapping` accepted `self.i = 100` silently -- the
/// declaration lied to every reader. Rejecting at the declaration keeps the
/// two features composable-by-omission: keep the range (Exact proof
/// obligations enforce it) or keep the domain (defined overflow, full type
/// range), never both.
pub(crate) fn check_range_under_non_exact_domain(
    program: &TypedTrees,
    handle: psi_typed_trees::types::TypeReferenceHandle,
    owner: crate::type_references::TypeReferenceOwner<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use psi_numerics::arithmetic::ArithmeticDomain;
    // Walk Reference wrappers AND nested Constrained spellings: `u8 [0..=4]
    // in Wrapping` may parse as Constrained(Constrained(u8, [Range]),
    // [Domain]), where the shallow per-node accessors see only one
    // constraint set each.
    fn has_range_constraint(
        program: &TypedTrees,
        handle: psi_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => has_range_constraint(program, *referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Range { .. }))
                    || has_range_constraint(program, *base_type)
            }
            _ => false,
        }
    }
    if !has_range_constraint(program, handle) {
        return;
    }
    let domain = program.type_reference_table.arithmetic_domain(handle);
    if domain == ArithmeticDomain::Exact {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "{owner} declares a range constraint together with the `{domain:?}` domain: ranges are only enforced under Exact arithmetic, so the combination is ill-formed (a store outside the range would be silently accepted). Keep the range and drop the domain, or keep the domain and drop the range"
    )));
}

pub(crate) fn range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Range { minimum, maximum } => Some(Interval {
                    low: Some(literal_i64(program, *minimum)?),
                    high: Some(
                        literal_i64(program, *maximum)
                            .or_else(|| dependent_maximum_substituted(program, *maximum))?,
                    ),
                }),
                _ => None,
            }),
        _ => None,
    }
}

fn float_range_constraint_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<FloatInterval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            float_range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Range { minimum, maximum } => Some(FloatInterval {
                    low: Some(float_literal_value(program, *minimum)?),
                    high: Some(float_literal_value(program, *maximum)?),
                }),
                _ => None,
            }),
        _ => None,
    }
}

fn float_bound_from(
    operator: BinaryOperator,
    literal: f64,
    name_on_left: bool,
) -> Option<FloatInterval> {
    let operator = if name_on_left {
        operator
    } else {
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            _ => return None,
        }
    };
    Some(match operator {
        // Treat strict bounds as inclusive at the same endpoint. This is a
        // conservative widening and avoids target-format nextafter logic.
        BinaryOperator::Less | BinaryOperator::LessOrEqual => FloatInterval {
            low: None,
            high: Some(literal),
        },
        BinaryOperator::Greater | BinaryOperator::GreaterOrEqual => FloatInterval {
            low: Some(literal),
            high: None,
        },
        _ => return None,
    })
}

/// R1a dependent maximum in interval position: `[0..=self.count]` reads as
/// the named field's own enforced literal HIGH plus the offset -- sound
/// because the field's range is store-enforced at every write, so the
/// dependent bound can never exceed it. The field is resolved by NAME
/// across all data definitions and must be UNIQUE (or agree everywhere);
/// an ambiguous name with disagreeing ranges bails rather than guesses
/// (this helper has no machine context; the declaration gate has already
/// verified the binding machine's own field is ranged).
fn dependent_maximum_substituted(
    program: &TypedTrees,
    maximum: psi_typed_trees::expression::ExpressionHandle,
) -> Option<i64> {
    let symbolic =
        psi_typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, maximum)?;
    let mut resolved: Option<i64> = None;
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            if field.name.as_str() != symbolic.field.as_str() || !field.type_reference.is_valid() {
                continue;
            }
            let high = range_constraint_interval(program, field.type_reference)
                .and_then(|interval| interval.high)?;
            match resolved {
                None => resolved = Some(high),
                Some(existing) if existing == high => {}
                Some(_) => return None,
            }
        }
    }
    resolved?.checked_add(symbolic.offset)
}

/// S4 return-range inference: the DECLARED return type of a value-position call,
/// when it resolves soundly and UNIQUELY from `program` alone. Conservative --
/// only `self`/receiver-free calls to a sibling machine (one attached to the same
/// data as the caller, with a state named after the call target) are resolved;
/// an external receiver, an ambiguous match, or no match returns `None` (no
/// narrowing). Soundness rests on uniqueness: bail rather than guess.
/// If `operand` is a value-machine call resolving to a self/sibling machine with an
/// INTEGER return type, return the call's target name (for the decision-17 overflow
/// hint). `None` for non-calls, builtins/external-receiver calls (unresolved), and
/// non-integer returns -- so the hint only fires where "annotate the callee's return"
/// is the actionable fix.
fn overflow_operand_value_call_target(
    program: &TypedTrees,
    current_machine: &Machine,
    operand: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(operand) else {
        return None;
    };
    let call = call.clone();
    let return_type = call_return_type(program, current_machine, &call)?;
    program
        .primitive_type_reference(return_type)
        .filter(|primitive| primitive.accepts_integer_literal())
        .map(|_| call.target.as_str().to_string())
}

pub(crate) fn call_return_type(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if crate::proof_embeddings::is_exact_embed_call(program, call) {
        return crate::proof_embeddings::proof_int_type_reference(program);
    }
    if let Some(operator) = psi_typed_trees::operator::resolve_named_expression_call(program, call)
        && operator.return_type.is_valid()
    {
        return Some(operator.return_type);
    }

    let receiver_is_self = !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        );
    if !receiver_is_self {
        return None;
    }
    let target = call.target.as_str();
    let attached_data = current_machine.attached_data.as_ref()?;
    let mut returns = program
        .machines()
        .iter()
        .filter(|candidate| {
            candidate
                .attached_data
                .as_ref()
                .is_some_and(|data| data.as_str() == attached_data.as_str())
        })
        .filter_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name.as_str() == target)
        })
        .filter(|state| state.return_type.is_valid())
        .map(|state| state.return_type);
    let first = returns.next()?;
    // Unique match only -- bail on ambiguity (sound: fall back to unbounded).
    if returns.next().is_some() {
        return None;
    }
    Some(first)
}

thread_local! {
    /// One-level recursion guard for return-range INFERENCE (ch15 stage 2). While
    /// inferring a callee's return interval we analyze its body; if that body
    /// calls another machine, we must NOT recurse into inference again (would
    /// loop on recursive/mutually-recursive callees). The nested call simply
    /// stays NEUTRAL.
    static INFERRING_RETURN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The unique self/sibling machine+state a receiver-free `self.target(..)` call
/// resolves to (mirrors `call_return_type`'s resolution but keeps the state so
/// its body can be analyzed). `None` on a non-self receiver or an ambiguous
/// match (sound: bail rather than guess).
fn resolve_unique_self_call_state<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
) -> Option<(&'program Machine, &'program State)> {
    let receiver_is_self = !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        );
    if !receiver_is_self {
        return None;
    }
    let target = call.target.as_str();
    let attached_data = current_machine.attached_data.as_ref()?;
    let mut matches = program.machines().iter().filter_map(|candidate| {
        let same_data = candidate
            .attached_data
            .as_ref()
            .is_some_and(|data| data.as_str() == attached_data.as_str());
        if !same_data {
            return None;
        }
        let state = program
            .machine_states(candidate)
            .iter()
            .find(|state| state.name.as_str() == target)?;
        Some((candidate, state))
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

/// ch15 stage 2 -- MODULAR RETURN-RANGE INFERENCE: when a callee declares no
/// return range, infer one from its body so the caller's arithmetic on the
/// result can stay Exact without the callee writing `-> i32 [a..=b]`. Sound and
/// STRICTLY PERMISSIVE: the body is analyzed with an EMPTY env (params at full
/// type width -> the widest possible result, so any caller's actual return is
/// within it). All return paths of the callee state are UNIONed -- a terminal
/// expression and/or transition VALUE targets (`{ cond -> v1 _ -> v2 }`). SOUND:
/// every path must be captured, so the state must be a LEAF value state -- if any
/// transition target is Named/SelfTarget (a return could come from a state we are
/// not analyzing, or a loop), we bail. The interval is trusted only if every
/// path's analysis is clean and the union is fully bounded. Recursion-guarded to
/// one level.
fn infer_return_interval(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    target_primitive: Option<PrimitiveType>,
) -> Option<Interval> {
    use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
    if INFERRING_RETURN.with(std::cell::Cell::get) {
        return None;
    }
    let statements = program
        .statement_table
        .statements(callee_state.statement_nodes);

    // Collect every return expression, bailing if any exit could escape to an
    // uncaptured state. A terminal expression is the last statement; transition
    // arms return via VALUE targets.
    let mut return_expressions = Vec::new();
    if let Some(StatementNode::Expression(expression)) = statements.last() {
        return_expressions.push(*expression);
    }
    for statement in statements {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        for target in [transition.target, transition.continuation] {
            if !target.is_valid() {
                continue;
            }
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Value(expression) => return_expressions.push(*expression),
                TransitionTargetNode::Terminal => {}
                // Named (another state / recursion) or SelfTarget (loop): the
                // return may come from somewhere we are not analyzing -> bail.
                TransitionTargetNode::Named { .. } | TransitionTargetNode::SelfTarget => {
                    return None;
                }
            }
        }
    }
    if return_expressions.is_empty() {
        return None;
    }

    let env = ValueEnv::new();
    INFERRING_RETURN.with(|flag| flag.set(true));
    let mut union: Option<Interval> = None;
    let mut clean = true;
    for expression in return_expressions {
        let mut throwaway = Vec::new();
        let analysis = analyze(
            program,
            callee_machine,
            Some(callee_state),
            expression,
            &env,
            target_primitive,
            ArithmeticDomain::Exact,
            "inferred return",
            &mut throwaway,
        );
        if !throwaway.is_empty() {
            clean = false;
            break;
        }
        union = Some(match union {
            Some(current) => current.union(analysis.interval),
            None => analysis.interval,
        });
    }
    INFERRING_RETURN.with(|flag| flag.set(false));
    if !clean {
        return None;
    }
    let union = union?;
    (union.low().is_some() && union.high().is_some()).then_some(union)
}

/// S4 return-range ENFORCEMENT (companion to the call-site narrowing): when a
/// return type declares a literal `[a..=b]`, the returned value's proven
/// interval must fit inside it, else callers that trust the declared range are
/// unsound. No-op when the return type carries no range constraint (so plain
/// returns are unaffected -- range-constrained return types are a new
/// capability). `interval` is the return expression's already-analyzed interval.
pub(crate) fn enforce_declared_return_range(
    program: &TypedTrees,
    return_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(range) = range_constraint_interval(program, return_type)
        && !range.contains(interval)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} returns a value not provably within its declared range: callers rely on the \
             declared `[a..=b]` for exact arithmetic on the result, so the returned value must \
             be proven to honor it (decision 17). Constrain the returned value, or widen/remove the \
             return range constraint."
        )));
    }
}

/// S4: analyze a transition VALUE-return expression and enforce its declared
/// return range. Gated on the return type carrying a range constraint -- only
/// then is the (otherwise un-validated) transition-value return analyzed, so
/// existing plain returns are byte-for-byte unaffected.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_return_value_range(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    return_expression: ExpressionHandle,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let return_primitive = program.primitive_type_reference(state.return_type);
    let return_domain = program.arithmetic_domain_for_type_reference(state.return_type);
    if range_constraint_interval(program, state.return_type).is_some() {
        // Range-constrained return: analyze (emitting any overflow obligation, as
        // before), enforce the declared `[a..=b]`, and -- on a clean value -- the
        // narrowing store obligation too. A value proven within `[a..=b]` already
        // fits the type, so the narrowing check adds no rejection here; it is
        // present only for uniformity with the unconstrained branch.
        let before = diagnostics.len();
        let (interval, source) = validate_value_range(
            program,
            machine,
            Some(state),
            return_expression,
            env,
            return_primitive,
            return_domain,
            owner,
            diagnostics,
        );
        if diagnostics.len() == before {
            check_narrowing_assignment(return_primitive, interval, source, owner, diagnostics);
        }
        enforce_declared_return_range(program, state.return_type, interval, owner, diagnostics);
        return;
    }
    // Unconstrained return: analyze into the REAL diagnostics, emitting any exact-
    // arithmetic overflow obligation just like every other value-binding boundary
    // (`_ -> (x + y)` with full-range Exact operands would otherwise wrap silently).
    // On a clean value, add the narrowing store obligation too -- a value that fits
    // its source type but not the return type (`-> i8 { _ -> (300) }`) is a silent
    // truncation.
    let before = diagnostics.len();
    let (interval, source) = validate_value_range(
        program,
        machine,
        Some(state),
        return_expression,
        env,
        return_primitive,
        return_domain,
        owner,
        diagnostics,
    );
    if diagnostics.len() == before {
        check_narrowing_assignment(return_primitive, interval, source, owner, diagnostics);
    }
}

/// A type-constraint range bound as an i64: a literal integer (`[0..100]`) or
/// a CONSTANT integer expression (`[0 - 1..=40]` folds to `-1` -- expression
/// bounds used to silently behave UNBOUNDED). A non-constant bound is not
/// narrowed (the caller falls back to the full type range -- sound; the
/// declaration check in type_references.rs rejects it loudly).
pub(crate) fn literal_i64(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    program.expression_table.constant_integer_value(expression)
}

fn literal_u64(program: &TypedTrees, expression: ExpressionHandle) -> Option<u64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.value_bignum()?.to_u64(),
        ExpressionNode::Borrow(inner) => literal_u64(program, inner.target),
        _ => None,
    }
}

fn known_u64_value(
    program: &TypedTrees,
    env: &ValueEnv,
    expression: ExpressionHandle,
) -> Option<u64> {
    literal_u64(program, expression).or_else(|| {
        let path = place_path(program, expression)?;
        env.known_u64_values.get(&path).copied()
    })
}

/// Whether a place's declared type is an atomic integer (`AtomicU32`, ...),
/// whose arithmetic wraps by hardware semantics, through reference/constraint
/// shells.
fn is_atomic_type(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { name, .. } => name.as_str().starts_with("Atomic"),
        TypeReferenceNode::Reference { referee, .. } => is_atomic_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => is_atomic_type(program, *base_type),
        _ => false,
    }
}

fn primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::I8 => "i8",
        PrimitiveType::U8 => "u8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::U16 => "u16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::U32 => "u32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Addr => "addr",
        PrimitiveType::Bool => "bool",
        PrimitiveType::F32 => "f32",
        PrimitiveType::F64 => "f64",
    }
}

#[cfg(test)]
mod tests;

/// Containment of a cleanly-analyzed store interval in the target's declared
/// Exact `[a..=b]` range. Both ends must be proven: later reads trust this
/// range as an invariant, so an unknown store cannot establish it. Non-Exact
/// shells and unranged targets remain deliberately permissive.
pub(crate) fn check_range_containment(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declared) = enforced_declared_range_interval(program, target_type) else {
        return;
    };
    let contained_low = match (interval.low, declared.low) {
        (Some(low), Some(declared_low)) => low >= declared_low,
        _ => false,
    };
    let contained_high = match (interval.high, declared.high) {
        (Some(high), Some(declared_high)) => high <= declared_high,
        _ => false,
    };
    if !contained_low || !contained_high {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} stores a value not provably within its declared range: the range is a \
             store-enforced invariant every read trusts (indexes, exact arithmetic), so the \
             stored value must be proven to honor it. Narrow the value with a dominating \
             guard, a dependent bound, or a requires coupling -- or widen/remove the range",
        )));
    }
}

/// The declared literal `[a..=b]` of a type reference, ONLY under all-Exact
/// Constrained shells (non-Exact ranges are deliberately permissive).
fn enforced_declared_range_interval(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_declared_range_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { minimum, maximum } => Some(Interval {
                        low: Some(literal_i64(program, *minimum)?),
                        high: Some(literal_i64(program, *maximum)?),
                    }),
                    _ => None,
                })
                .or_else(|| enforced_declared_range_interval(program, *base_type))
        }
        _ => None,
    }
}
