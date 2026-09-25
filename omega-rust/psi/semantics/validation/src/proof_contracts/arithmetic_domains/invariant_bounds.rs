//! Bounds valid for every evaluation of immutable, builtin integer expressions.
//!
//! Closed constants retain their exact integer beside the compatibility interval.
//! A u64 intermediate may exceed that interval's signed window and later return
//! to it; losing the point would lose valid static endpoints. Fixed-width kernels
//! still check each typed operation and operand landing, before interval fallback.
//! Declared singleton ranges on parameters/fields do not become static values.
//! An immutable parameter also carries the literal comparisons its owning
//! `requires` clause states: every arrival proves them and nothing rewrites it.
//! A field read carries its data's `where` facts, which every write preserves.
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, ProofFact, SignatureContractKind, State, TypeReferenceHandle, TypeReferenceNode,
    TypedTrees, enforced_declared_range, literal_i64,
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut carrier_type = type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier_type)
    {
        carrier_type = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier_type)
    else {
        return None;
    };
    let primitive = match program.symbols.builtin_type_atom(*symbol)? {
        symbols::BuiltinTypeAtom::U8 => PrimitiveType::U8,
        symbols::BuiltinTypeAtom::U16 => PrimitiveType::U16,
        symbols::BuiltinTypeAtom::U32 => PrimitiveType::U32,
        symbols::BuiltinTypeAtom::U64 => PrimitiveType::U64,
        symbols::BuiltinTypeAtom::I8 => PrimitiveType::I8,
        symbols::BuiltinTypeAtom::I16 => PrimitiveType::I16,
        symbols::BuiltinTypeAtom::I32 => PrimitiveType::I32,
        symbols::BuiltinTypeAtom::I64 => PrimitiveType::I64,
        _ => return None,
    };
    if program.arithmetic_domain_for_type_reference(type_reference) != ArithmeticDomain::Exact {
        return None;
    }
    Some(primitive)
}

/// Bound a literal or builtin arithmetic tree over exact immutable primitive
/// parameters or their direct owned integer fields. No initializer, caller flow
/// fact, callee body, or mutable place is read: the interval is valid independently
/// of the evaluation snapshot. A parameter's own `requires` bounds are arrival
/// facts, not snapshot facts, so they are read.
pub fn immutable_integer_expression_bounds(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
        .or_else(|| {
            let declared =
                place_type.and_then(|handle| super::range_constraint_interval(program, handle));
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
        })?;
    // A place's bare carrier range states nothing beyond its type; callers
    // that report a missing bound must still see none.
    let carrier = place_type
        .and_then(|handle| program.primitive_type_reference(handle))
        .and_then(super::integer_ranges::primitive_range);
    (carrier != Some(interval)).then_some(interval)
}

/// [`standing_integer_interval`] as open-ended endpoints, for checkers
/// outside validation.
pub fn standing_integer_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    standing_integer_interval(program, machine, state, expression)
        .map(|interval| (interval.low, interval.high))
}

/// The bounds every evaluation of one state parameter satisfies: its exact
/// carrier and declared range, narrowed for an immutable parameter by its
/// state's `requires` comparisons. Either endpoint may be open.
pub fn state_parameter_integer_interval(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    parameter: SymbolHandle,
) -> Option<(Option<i64>, Option<i64>)> {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|candidate| candidate.symbol == parameter)?;
    if parameter.is_self || parameter.is_const {
        return None;
    }
    let mut interval = type_bounds(program, parameter.type_reference)?.interval;
    if !parameter.is_mutable {
        interval = interval.intersect(requires_interval(program, machine, state, parameter.symbol));
    }
    Some((interval.low, interval.high))
}

/// The same bounds with either endpoint allowed to be open: an unrestricted
/// u64 keeps its zero floor although its ceiling does not fit an i64.
pub(crate) fn immutable_integer_expression_interval(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
    program: &TypedTrees,
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
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
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

fn type_bounds(program: &TypedTrees, type_reference: TypeReferenceHandle) -> Option<Bounds> {
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
    program: &TypedTrees,
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
    // identity; the strict record projection is preferred for member reads so
    // a lost or foreign member symbol cannot name-match a literal field. A
    // failed closed query does not grant a value to the interval fallback.
    let mut closed_anonymous = None;
    if let Some(value) =
        crate::value_custody::literals::closed_record_integer_projection(program, expression)
            .or_else(|| program.closed_integer_value_in(expression, machine.symbol))
    {
        // Every member read folded into the closed value must satisfy strict
        // record custody: a member that resolves loosely but not through the
        // declared field's own symbol is a custody break, not a value.
        let mut has_member = false;
        let mut pending = vec![expression];
        while let Some(handle) = pending.pop() {
            let node = program.expression_table.expression(handle);
            if matches!(node, ExpressionNode::Member(_)) {
                has_member = true;
                if crate::value_custody::literals::closed_record_integer_projection(program, handle)
                    .is_none()
                    && program
                        .closed_integer_value_in(handle, machine.symbol)
                        .is_some()
                {
                    return None;
                }
            }
            crate::value_custody::literals::expression_children::children(program, node, |child| {
                pending.push(child)
            });
        }
        // A closed value outside its carrier is an overflow verdict, not a
        // usable bound: a successful bounds run must prove carrier landing.
        if let Some(primitive) = value.primitive {
            typed_trees::closed_numeric::land_integer(&value.value, primitive)?;
        }
        // Anonymous results of member-bearing trees may have lost a field's
        // declared carrier during closed evaluation; those trees prove their
        // bounds structurally instead of trusting the carrier-free value.
        if value.primitive.is_some() || !has_member {
            return Some(Bounds::constant(
                value.value,
                value.primitive,
                value.type_reference,
            ));
        }
        closed_anonymous = Some(Bounds::constant(
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
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            if parameter.is_self
                || parameter.is_const
                || (parameter.is_mutable && !declared_mutable_leaves)
            {
                return None;
            }
            let declared = type_bounds(program, parameter.type_reference)?;
            if parameter.is_mutable {
                return Some(declared);
            }
            Some(Bounds {
                interval: declared.interval.intersect(requires_interval(
                    program,
                    machine,
                    state,
                    parameter.symbol,
                )),
                ..declared
            })
        }
        ExpressionNode::Member(_) => {
            let declared = type_bounds(
                program,
                fields::type_reference(program, state?, expression, declared_mutable_leaves)?,
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
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::BitwiseAnd | BinaryOperator::ShiftRight
            ) =>
        {
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
            unsigned_bitwise_bounds(program, binary.operator, left, right)
        }
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return None,
            };
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
            // A wholly anonymous tree already proved its closed value and has
            // no carrier to check operand meanings against.
            if left.primitive.is_none() && right.primitive.is_none() {
                return closed_anonymous;
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
            // The shared i64 interval engine cannot represent abs(i64::MIN).
            // Do not use its saturating divisor magnitude as an exact bound.
            if binary.operator == BinaryOperator::Modulo && right.interval.low == Some(i64::MIN) {
                return None;
            }
            let interval = match binary.operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
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

/// Unsigned `&` and `>>` have no selectable spelling, so their meaning is
/// builtin. A mask keeps the smaller bounded operand's ceiling; a right shift
/// by a count inside the carrier's width scales both endpoints down. Signed
/// carriers and out-of-width counts stay unbounded here.
fn unsigned_bitwise_bounds(
    program: &TypedTrees,
    operator: BinaryOperator,
    left: Bounds,
    right: Bounds,
) -> Option<Bounds> {
    let primitive = left.primitive?;
    if !matches!(
        primitive,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
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
        BinaryOperator::BitwiseAnd => Interval {
            low: Some(0),
            high: match (left.interval.high, right.interval.high) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (bounded, None) | (None, bounded) => bounded,
            },
        },
        BinaryOperator::ShiftRight => {
            let width = super::integer_bit_width(primitive)?;
            let (low_count, high_count) = (right.interval.low?, right.interval.high?);
            if high_count >= width {
                return None;
            }
            Interval {
                low: Some(left.interval.low? >> high_count),
                high: left.interval.high.map(|high| high >> low_count),
            }
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

/// The literal comparisons on `parameter` in the clause every arrival proves:
/// the machine's `requires` for its entry state, the state's own otherwise.
/// Only exact builtin orderings and equalities over a literal contribute.
fn requires_interval(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    parameter: SymbolHandle,
) -> Interval {
    let entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let machine_contracts = if entry {
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
        let ProofFact::Expression(expression) = fact else {
            continue;
        };
        let mut conjuncts = vec![*expression];
        while let Some(conjunct) = conjuncts.pop() {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(conjunct)
            else {
                continue;
            };
            if binary.operator == BinaryOperator::And {
                conjuncts.extend([binary.left, binary.right]);
                continue;
            }
            let builtin = match binary.operator {
                BinaryOperator::Equal => super::guard_narrowing::has_builtin_equality(
                    program,
                    machine,
                    Some(state),
                    conjunct,
                ),
                _ => super::guard_narrowing::has_builtin_ordering(
                    program,
                    machine,
                    Some(state),
                    conjunct,
                ),
            };
            if !builtin {
                continue;
            }
            for (subject, operand, subject_on_left) in [
                (binary.left, binary.right, true),
                (binary.right, binary.left, false),
            ] {
                let names_parameter = matches!(
                    program.expression_table.expression(subject),
                    ExpressionNode::Name(path)
                        if path.symbol == parameter && path.head_symbol == path.symbol
                );
                if names_parameter && let Some(value) = literal_i64(program, operand) {
                    interval = interval.intersect(super::guard_narrowing::comparison_interval(
                        binary.operator,
                        Interval::constant(value),
                        subject_on_left,
                    ));
                }
            }
        }
    }
    interval
}

#[cfg(test)]
mod tests;
