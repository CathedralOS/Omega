//! Bounds valid for every evaluation of immutable, builtin integer expressions.
//!
//! Closed constants retain their exact integer beside the compatibility interval.
//! A u64 intermediate may exceed that interval's signed window and later return
//! to it; losing the point would lose valid static endpoints. Fixed-width kernels
//! still check each typed operation and operand landing, before interval fallback.
//! Declared singleton ranges on parameters/fields do not become static values.
//! An immutable parameter also carries the literal comparisons its owning
//! `requires` clause states: every arrival proves them and nothing rewrites it.
//! An integer const or value binder does not change during the machine, so the
//! same comparisons bound it in every state. A runtime value compared with that
//! binder (`index < Bound` where `Bound <= 4`) then stands inside the binder's
//! ceiling, the same way it would under an immutable parameter bound.
//! A disjunction contributes only when every alternative bounds that parameter;
//! the bound is their convex hull, so `side == 0 || side == 1` is `[0, 1]`
//! while `side == 0 || flag == 1` does not bound `side`.
//! A field read carries its data's `where` facts, which every write preserves.
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, SignatureContractKind, State, TableCallExpression, TypeReferenceHandle,
    TypeReferenceNode, enforced_declared_range, literal_i64,
};
use crate::proof_contracts::arithmetic_domains::integer_ranges::primitive_range;
use language_core::OperatorSpelling;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

mod fields;

/// Bounds enforced by an exact owned integer type at storage boundaries.
/// References and atomic/policy carriers supply no invariant here. A caller
/// using a field type must separately establish its exact declaration identity.
pub fn enforced_integer_type_bounds(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    let primitive = exact_integer_primitive(program, type_reference)?;
    let carrier = primitive_range(primitive)?;
    let interval = enforced_declared_range(program, type_reference)
        .map_or(carrier, |range| range.intersect(carrier));
    let (low, high) = (interval.low?, interval.high?);
    (low <= high).then_some((low, high))
}

fn exact_integer_primitive(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut carrier_type = type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier_type)
    {
        carrier_type = *base_type;
    }
    let symbol = &program
        .type_reference_table
        .type_reference(carrier_type)
        .as_named_symbol()?;
    let primitive = PrimitiveType::from_builtin_atom(program.symbols.builtin_type_atom(*symbol)?)?;
    if program.arithmetic_domain_for_type_reference(type_reference) != ArithmeticDomain::Exact {
        return None;
    }
    Some(primitive)
}

/// Bound a literal or builtin arithmetic tree over exact immutable primitive
/// parameters or declaration-checked integer field paths. No initializer,
/// caller flow fact or callee body is replayed: field bounds come from the
/// selected storage declaration and its standing predicates, not a captured
/// value. A parameter's own `requires` bounds are arrival facts, not snapshot
/// facts, and mutable parameter arrival facts are excluded.
pub fn immutable_integer_expression_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    let value = bounds(program, machine, Some(state), expression, false)?;
    Some((value.interval.low?, value.interval.high?))
}

/// A place's standing bounds at any read: the immutable bounds above, or for
/// a mutable place what every store enforces -- its declared range and its
/// data's `where` facts.
pub(crate) fn standing_integer_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let place_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    );
    let interval = immutable_integer_expression_interval(program, machine, state, expression)
        .or_else(|| stored_place_interval(program, machine, state, expression))?;
    // A place's bare carrier range states nothing beyond its type; callers
    // that report a missing bound must still see none.
    (place_carrier_interval(program, place_type) != Some(interval)).then_some(interval)
}

/// The integer carrier range of a place's declared type, where it has one.
/// This is the range the type alone states, so a derived bound that restates
/// one of its endpoints carries no fact about the program: under ZII the
/// absence of a bound is the zero value, not the carrier's extreme. Every
/// query that reports a missing bound measures against this, so the judgment
/// lives here once rather than beside each caller.
///
/// A read of an `&mut u32` place yields a `u32`, and a `u32 in P` place
/// carries a `u32`, so the reference and qualification shells are peeled
/// first: the carrier is a property of the storage, and a judgment keyed to
/// the outermost shell would see none for exactly the places a reference
/// spells.
fn place_carrier_interval(
    program: &dyn crate::CheckingInput,
    place_type: Option<TypeReferenceHandle>,
) -> Option<Interval> {
    place_type
        .and_then(|handle| program.unwrapped_type_reference(handle))
        .and_then(|handle| program.primitive_type_reference(handle))
        .and_then(primitive_range)
}

/// An upper bound on `expression` that states something its own integer
/// carrier does not. A maximum at or above the carrier's own is what the
/// type already says -- the shape that let a `u32` offset be reported to an
/// author as the literal offset 4294967295 -- so it is absence, as
/// [`standing_integer_interval`] already treats a whole interval that
/// restates the carrier.
pub(crate) fn upper_bound_beyond_place_carrier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    high: i64,
) -> Option<i64> {
    let place_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    );
    let carrier_high = place_carrier_interval(program, place_type).and_then(|carrier| carrier.high);
    carrier_high
        .is_none_or(|carrier_high| high < carrier_high)
        .then_some(high)
}

/// What every store to a place enforces: its declared range and its data's
/// `where` facts. Reads no arrival fact, so it is safe inside `requires`
/// resolution.
fn stored_place_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Interval> {
    let declared = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    )
    .and_then(|handle| super::range_constraint_interval(program, handle));
    let facts = crate::proof_contracts::default_domains::where_fact_interval(
        program,
        machine,
        Some(state),
        expression,
    );
    match (declared, facts) {
        (Some(declared), Some(facts)) => Some(declared.intersect(facts)),
        (declared, facts) => declared.or(facts),
    }
}

/// [`standing_integer_interval`] as open-ended endpoints, for checkers
/// outside validation.
pub fn standing_integer_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    standing_integer_interval(program, machine, state, expression)
        .map(|interval| (interval.low, interval.high))
}

/// What every store to a place enforces -- its declared range and its data's
/// `where` facts -- completed by its integer carrier. Every read of the place
/// lies within them, and no arrival fact is read, so a caller may use them to
/// decide its callee's requirement. For checkers outside validation. `None`
/// for a non-integer place.
pub fn stored_integer_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    let carrier = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        expression,
    )
    .and_then(|handle| program.primitive_type_reference(handle))
    .and_then(super::integer_ranges::primitive_range)?;
    let interval = stored_place_interval(program, machine, state, expression)
        .map_or(carrier, |stored| stored.intersect(carrier));
    Some((interval.low, interval.high))
}

/// The bounds every evaluation of one state parameter satisfies: its exact
/// carrier and declared range, narrowed for an immutable parameter by its
/// state's `requires` comparisons. Either endpoint may be open.
pub fn state_parameter_integer_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    parameter: SymbolHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|candidate| candidate.symbol == parameter)?;
    if parameter.is_const {
        return None;
    }
    let mut interval = type_bounds(program, parameter.type_reference)?.interval;
    if !parameter.is_mutable {
        interval = interval.intersect(requires_interval(
            program,
            machine,
            state,
            parameter.symbol,
            false,
        ));
    }
    Some((interval.low, interval.high))
}

/// The same bounds with either endpoint allowed to be open: an unrestricted
/// u64 keeps its zero floor although its ceiling does not fit an i64.
pub(crate) fn immutable_integer_expression_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Interval> {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    Some(bounds(program, machine, Some(state), expression, false)?.interval)
}

/// Bound a literal or builtin arithmetic tree over exact primitive parameters
/// the way their declarations bound every evaluation. A mutable input is not
/// disqualified: its declared type is store-enforced, so the interval holds of
/// its live value wherever the expression is read. The interval says nothing
/// about WHICH live value a mutable input holds; callers judging an
/// invocation-fixed quantity must separately preserve or re-pin its inputs.
///
/// The verdict, not the interval, is the result: a successful `bounds` run
/// already proved every operand and result lands inside its carrier, which
/// is all an endpoint-formation caller needs. Returning the i64 pair would
/// lose exactly the verdict at full-width carriers -- `x + 0` over an
/// unbounded u64 lands, but its interval's high end does not fit i64.
pub fn declared_integer_expression_lands(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return false;
    }
    bounds(program, machine, Some(state), expression, true).is_some()
}

/// Retain one-sided carrier bounds when projecting an exact builtin guard.
/// An unrestricted u64 has a useful zero floor even though its ceiling does
/// not fit the interval engine's i64 endpoint representation.
pub(super) fn builtin_comparison_intervals(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Interval, Interval)> {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let left = bounds(program, machine, Some(state), binary.left, false)?;
    let right = bounds(program, machine, Some(state), binary.right, false)?;
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[left.type_reference, right.type_reference],
    )
    .then_some((left.interval, right.interval))
}

struct Bounds {
    interval: Interval,
    constant_value: Option<BigInt>,
    primitive: Option<PrimitiveType>,
    type_reference: Option<TypeReferenceHandle>,
}

impl Bounds {
    fn constant(
        value: BigInt,
        primitive: Option<PrimitiveType>,
        type_reference: Option<TypeReferenceHandle>,
    ) -> Self {
        let interval = match value.to_i64() {
            Some(value) => Interval::constant(value),
            None if value.is_negative() => Interval {
                low: None,
                high: Some(i64::MIN),
            },
            None => Interval {
                low: Some(i64::MAX),
                high: None,
            },
        };
        Self {
            interval,
            constant_value: Some(value),
            primitive,
            type_reference,
        }
    }
}

fn type_bounds(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<Bounds> {
    let primitive = exact_integer_primitive(program, type_reference)?;
    let carrier = primitive_range(primitive)?;
    Some(Bounds {
        interval: enforced_declared_range(program, type_reference)
            .map_or(carrier, |range| range.intersect(carrier)),
        constant_value: None,
        primitive: Some(primitive),
        type_reference: Some(type_reference),
    })
}

fn bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    // When true, a mutable parameter contributes its declared storage bounds:
    // every store maintains them, so the interval holds at every evaluation.
    // When false, mutable places are opaque, as a snapshot-free read requires.
    declared_mutable_leaves: bool,
) -> Option<Bounds> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    // Closed arithmetic has one value/selection owner shared with type
    // identity. Record projections have a separate custody-aware query; the
    // numeric evaluator never substitutes a field's initializer for its read.
    if let Some(value) =
        crate::value_custody::literals::closed_record_integer_projection(program, expression)
            .or_else(|| program.closed_integer_value_in(expression, machine.symbol))
    {
        // A closed value outside its carrier is an overflow verdict, not a
        // usable bound: a successful bounds run must prove carrier landing.
        if let Some(primitive) = value.primitive {
            typed_trees::closed_numeric::land_integer(&value.value, primitive)?;
        }
        return Some(Bounds::constant(
            value.value,
            value.primitive,
            value.type_reference,
        ));
    }
    match program.expression_table.expression(expression) {
        // Literal failure in the shared query is final, not a second landing path.
        ExpressionNode::Integer(_) => None,
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let state = state?;
            if let Some(parameter) = program.state_parameter_by_symbol(state, path.symbol) {
                if parameter.is_const || (parameter.is_mutable && !declared_mutable_leaves) {
                    return None;
                }
                let declared = type_bounds(program, parameter.type_reference)?;
                if parameter.is_mutable {
                    return Some(declared);
                }
                return Some(Bounds {
                    interval: declared.interval.intersect(requires_interval(
                        program,
                        machine,
                        state,
                        parameter.symbol,
                        false,
                    )),
                    ..declared
                });
            }
            integer_binder_bounds(program, machine, state, path.symbol)
        }
        ExpressionNode::Member(_) => {
            let declared = type_bounds(
                program,
                fields::type_reference(
                    program,
                    machine,
                    state?,
                    expression,
                    declared_mutable_leaves,
                )?,
            )?;
            // The owning data's `where` facts bound every legal read of the
            // field: each write must preserve them.
            let standing = crate::proof_contracts::default_domains::where_fact_interval(
                program, machine, state, expression,
            );
            Some(Bounds {
                interval: standing.map_or(declared.interval, |facts| {
                    declared.interval.intersect(facts)
                }),
                ..declared
            })
        }
        ExpressionNode::Cast(cast) => {
            // A total exact widening changes the carrier, not the value or
            // its established interval. It cannot erase the operand's own
            // operation meaning, qualify a domain, or reinterpret a policy.
            if !crate::proof_contracts::arithmetic_domains::plain_exact_widening_cast(program, cast)
            {
                return None;
            }
            let source = bounds(program, machine, state, cast.value, declared_mutable_leaves)?;
            let target = exact_integer_primitive(program, cast.target_type)?;
            if !super::integer_ranges::integer_widen_is_total(source.primitive?, target) {
                return None;
            }
            Some(Bounds {
                primitive: Some(target),
                type_reference: Some(cast.target_type),
                ..source
            })
        }
        // A call term's value carries the callee's contract: the declared
        // return range and the `ensures` conjuncts bounding `result` are
        // proven at every exit, so the interval holds of the produced value
        // wherever the call appears as an operand.
        ExpressionNode::Call(call) => Some(call_result_bounds(program, call)?),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::BitwiseAnd | BinaryOperator::ShiftRight
            ) =>
        {
            let (left, right) =
                binary_operand_bounds(program, machine, state, binary, declared_mutable_leaves)?;
            unsigned_bitwise_bounds(program, binary.operator, left, right)
        }
        ExpressionNode::Binary(binary) => {
            let spelling = typed_trees::operator::arithmetic_operator_spelling(binary.operator)?;
            let (left, right) =
                binary_operand_bounds(program, machine, state, binary, declared_mutable_leaves)?;
            // A wholly anonymous tree was handled by the closed query above.
            // Its failure cannot grant a second, integer-only evaluation route.
            if left.primitive.is_none() && right.primitive.is_none() {
                return None;
            }
            // A context-free endpoint has no owning specialization to select.
            // Retained late-bound occurrences may still acquire a trait meaning.
            // ponytail: veto any matching specialization until endpoints carry
            // their owner directly; an unrelated match may conservatively refuse.
            if state.is_none()
                && program
                    .machine_specializations
                    .iter()
                    .any(|specialization| {
                        !typed_trees::operator::selected_trait_operator_meanings(
                            program,
                            specialization.instance,
                            spelling,
                            &[left.type_reference, right.type_reference],
                        )
                        .is_empty()
                    })
            {
                return None;
            }
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &[left.type_reference, right.type_reference],
            ) {
                return None;
            }
            if left
                .primitive
                .zip(right.primitive)
                .is_some_and(|(left, right)| left != right)
            {
                return None;
            }
            let primitive = left.primitive.or(right.primitive)?;
            // A known operand must land even when its peer is a variable.
            for value in [&left.constant_value, &right.constant_value]
                .into_iter()
                .flatten()
            {
                typed_trees::closed_numeric::land_integer(value, primitive)?;
            }
            // Arithmetic results retain the carrier, not operand refinements.
            let mut result_type = left.type_reference.or(right.type_reference)?;
            while let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(result_type)
            {
                result_type = *base_type;
            }
            let carrier = primitive_range(primitive)?;
            // Anonymous operands land at the already-typed operation. A small
            // result is not evidence that an out-of-range operand can land.
            if !carrier.contains(left.interval) || !carrier.contains(right.interval) {
                return None;
            }
            // A later operation may produce small bounds, but cannot repair
            // an earlier Exact overflow hidden by the signed interval window.
            if primitive == PrimitiveType::U64
                && matches!(
                    binary.operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                )
                && !super::unsigned_representability::binary_fits(
                    binary.operator,
                    left.interval,
                    right.interval,
                    None,
                    None,
                )
            {
                return None;
            }
            if matches!(
                binary.operator,
                BinaryOperator::Divide | BinaryOperator::Modulo
            ) && primitive.is_signed_integer()
                && left.interval.contains(Interval::constant(carrier.low?))
                && right.interval.contains(Interval::constant(-1))
            {
                // Exact signed remainder shares the quotient's MIN / -1
                // definedness obligation, even though its result would be zero.
                return None;
            }
            // The shared remainder domain keeps signed-MIN divisor magnitudes
            // exact before projecting result bounds. Use that provider here;
            // original-operand landing and MIN / -1 formation remain above.
            let interval = match binary.operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
                BinaryOperator::Divide if primitive == PrimitiveType::U64 => {
                    super::unsigned_representability::divide_interval(
                        left.interval,
                        right.interval,
                        left.constant_value.as_ref().and_then(BigInt::to_u64),
                        right.constant_value.as_ref().and_then(BigInt::to_u64),
                    )
                }
                BinaryOperator::Divide => left.interval.divide(right.interval),
                BinaryOperator::Modulo
                    if left.interval.low == left.interval.high
                        && right.interval.low == right.interval.high =>
                {
                    Interval::constant(left.interval.low?.checked_rem(right.interval.low?)?)
                }
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                _ => return None,
            };
            carrier.contains(interval).then_some(Bounds {
                interval,
                constant_value: None,
                primitive: Some(primitive),
                type_reference: Some(result_type),
            })
        }
        _ => None,
    }
}

/// Both operands' bounds, `?`-threaded — the binary arms share this pair.
fn binary_operand_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    binary: &typed_trees::expression::TableBinaryExpression,
    declared_mutable_leaves: bool,
) -> Option<(Bounds, Bounds)> {
    let left = bounds(
        program,
        machine,
        state,
        binary.left,
        declared_mutable_leaves,
    )?;
    let right = bounds(
        program,
        machine,
        state,
        binary.right,
        declared_mutable_leaves,
    )?;
    Some((left, right))
}

/// Unsigned `&` and `>>` have builtin meaning. Preserve this immutable query's
/// operand landing and count-formation fences before asking the same carrier
/// provider as expression analysis. Its completed u64 ceiling and known-bit
/// hull retain information the compatibility interval alone cannot express;
/// they supply result bounds, never evidence that the operands can form.
fn unsigned_bitwise_bounds(
    program: &dyn crate::CheckingInput,
    operator: BinaryOperator,
    left: Bounds,
    right: Bounds,
) -> Option<Bounds> {
    let primitive = left.primitive?;
    if !matches!(
        primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        return None;
    }
    if operator == BinaryOperator::BitwiseAnd
        && right.primitive.is_some_and(|other| other != primitive)
    {
        return None;
    }
    let right_carrier = right.primitive.unwrap_or(primitive);
    for (value, carrier) in [
        (&left.constant_value, primitive),
        (&right.constant_value, right_carrier),
    ] {
        if let Some(value) = value {
            typed_trees::closed_numeric::land_integer(value, carrier)?;
        }
    }
    if left.interval.low? < 0 || right.interval.low? < 0 {
        return None;
    }
    let interval = match operator {
        BinaryOperator::BitwiseAnd => super::bitwise::binary(
            operator,
            primitive,
            left.interval,
            right.interval,
            left.constant_value.as_ref().and_then(BigInt::to_u64),
            right.constant_value.as_ref().and_then(BigInt::to_u64),
        ),
        BinaryOperator::ShiftRight => {
            let width = super::integer_bit_width(primitive)?;
            let high_count = right.interval.high?;
            if high_count >= width {
                return None;
            }
            super::bitwise::shift_right(
                primitive,
                left.interval,
                right.interval,
                left.constant_value.as_ref().and_then(BigInt::to_u64),
            )
        }
        _ => return None,
    };
    let mut result_type = left.type_reference?;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(result_type)
    {
        result_type = *base_type;
    }
    Some(Bounds {
        interval,
        constant_value: None,
        primitive: Some(primitive),
        type_reference: Some(result_type),
    })
}

/// What a call's integer result is contracted to: the declared return
/// carrier and range, intersected with the `ensures` conjuncts bounding
/// `result`. The callee proved its ensures at every exit, so the interval
/// holds wherever the call value is read. `None` for a call whose callee or
/// return type is not an exact fixed integer.
pub(crate) fn call_result_interval(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<Interval> {
    Some(call_result_bounds(program, call)?.interval)
}

fn call_result_bounds(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<Bounds> {
    let (callee, entry) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)?;
    let declared = type_bounds(program, entry.return_type)?;
    Some(Bounds {
        interval: declared
            .interval
            .intersect(ensures_result_interval(program, callee, entry)),
        ..declared
    })
}

/// The `result`-bounding conjuncts of a callable's `ensures` contracts, as an
/// interval over the returned value. The callee proved every ensures at its
/// exits, so the interval holds wherever the call result is read. Only
/// literal operands bound `result` here: a conjunct naming callee state
/// compares against a post-return snapshot the caller cannot see.
fn ensures_result_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
) -> Interval {
    let mut interval = Interval::UNBOUNDED;
    let ensures = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts));
    for fact in ensures {
        let Some(expression) = fact.as_expression() else {
            continue;
        };
        // Literal bounds only: a conjunct naming callee state would compare
        // against a post-return snapshot the caller cannot see.
        if let Some(bound) =
            boolean_subject_interval(program, machine, entry, *expression, true, &|subject| {
                program.expression_table.names_reserved_result(subject)
            })
        {
            interval = interval.intersect(bound);
        }
    }
    interval
}

/// Machine `requires` on an integer const or value binder. The binder is not
/// storage, so those comparisons remain premises after entry.
fn integer_binder_bounds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<Bounds> {
    let binder = program
        .machine_type_parameters(machine)
        .iter()
        .find(|parameter| parameter.symbol == symbol)?;
    let type_reference = match binder.kind {
        typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } => type_reference,
        _ => return None,
    };
    let declared = type_bounds(program, type_reference)?;
    Some(Bounds {
        interval: declared.interval.intersect(requires_interval(
            program,
            machine,
            state,
            binder.symbol,
            true,
        )),
        ..declared
    })
}

/// The comparisons on `parameter` in the clause every arrival proves: the
/// machine's `requires` for its entry state, the state's own otherwise.
/// `include_machine_contracts` also keeps the machine clause in later states,
/// for a binder that cannot be overwritten. Only exact builtin orderings and
/// equalities contribute, against a literal or a place whose stores enforce a
/// bound (`y <= self.max_y` with `where max_y <= 2` bounds `y` by 2).
/// Conjunctions intersect. A disjunction is the convex hull of alternatives
/// that each bound this parameter; one alternative that does not mention it
/// leaves the parameter unbounded, because that alternative can discharge the
/// clause alone.
fn requires_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    parameter: SymbolHandle,
    include_machine_contracts: bool,
) -> Interval {
    let entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let machine_contracts = if entry || include_machine_contracts {
        program.machine_contracts(machine)
    } else {
        &[]
    };
    let mut interval = Interval::UNBOUNDED;
    let requires = machine_contracts
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts));
    for fact in requires {
        let Some(expression) = fact.as_expression() else {
            continue;
        };
        if let Some(bound) =
            boolean_subject_interval(program, machine, state, *expression, false, &|subject| {
                names_parameter(program, subject, parameter)
            })
        {
            interval = interval.intersect(bound);
        }
    }
    interval
}

fn names_parameter(
    program: &dyn crate::CheckingInput,
    subject: ExpressionHandle,
    parameter: SymbolHandle,
) -> bool {
    matches!(
        program.expression_table.expression(subject),
        ExpressionNode::Name(path)
            if path.symbol == parameter && path.head_symbol == path.symbol
    )
}

/// Integer bound a boolean proof formula gives one subject. `None` means the
/// formula does not constrain that subject. Conjunction keeps a bound from
/// either side. Disjunction keeps a bound only when both sides constrain the
/// subject, and then only their convex hull: the hull is sound for overflow,
/// and a one-sided alternative would invent a bound the other alternative
/// never proved.
fn boolean_subject_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    literal_bounds_only: bool,
    names_subject: &impl Fn(ExpressionHandle) -> bool,
) -> Option<Interval> {
    let (operator, left, right) = {
        let binary = program.expression_table.binary_node(expression)?;
        (binary.operator, binary.left, binary.right)
    };
    match operator {
        BinaryOperator::And => {
            let left_bound = boolean_subject_interval(
                program,
                machine,
                state,
                left,
                literal_bounds_only,
                names_subject,
            );
            let right_bound = boolean_subject_interval(
                program,
                machine,
                state,
                right,
                literal_bounds_only,
                names_subject,
            );
            match (left_bound, right_bound) {
                (Some(left_bound), Some(right_bound)) => Some(left_bound.intersect(right_bound)),
                (Some(bound), None) | (None, Some(bound)) => Some(bound),
                (None, None) => None,
            }
        }
        BinaryOperator::Or => {
            let left_bound = boolean_subject_interval(
                program,
                machine,
                state,
                left,
                literal_bounds_only,
                names_subject,
            )?;
            let right_bound = boolean_subject_interval(
                program,
                machine,
                state,
                right,
                literal_bounds_only,
                names_subject,
            )?;
            Some(left_bound.union(right_bound))
        }
        _ => comparison_subject_interval(
            program,
            machine,
            state,
            expression,
            operator,
            left,
            right,
            literal_bounds_only,
            names_subject,
        ),
    }
}

fn comparison_subject_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    literal_bounds_only: bool,
    names_subject: &impl Fn(ExpressionHandle) -> bool,
) -> Option<Interval> {
    let builtin = match operator {
        BinaryOperator::Equal => {
            super::guard_narrowing::has_builtin_equality(program, machine, Some(state), expression)
        }
        _ => {
            super::guard_narrowing::has_builtin_ordering(program, machine, Some(state), expression)
        }
    };
    if !builtin {
        return None;
    }
    let mut interval: Option<Interval> = None;
    for (subject, operand, subject_on_left) in [(left, right, true), (right, left, false)] {
        if !names_subject(subject) {
            continue;
        }
        let Some(bound) = literal_i64(program, operand)
            .map(Interval::constant)
            .or_else(|| {
                if literal_bounds_only {
                    None
                } else {
                    stored_place_interval(program, machine, state, operand)
                }
            })
        else {
            continue;
        };
        let compared =
            super::guard_narrowing::comparison_interval(operator, bound, subject_on_left);
        if compared.low.is_none() && compared.high.is_none() {
            continue;
        }
        interval = Some(match interval {
            Some(existing) => existing.intersect(compared),
            None => compared,
        });
    }
    interval
}
