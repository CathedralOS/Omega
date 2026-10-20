//! Inferred and declared return value ranges.

use crate::proof_contracts::arithmetic_domains::assignments::{
    check_narrowing_assignment, stored_value_carrier,
};
use crate::proof_contracts::arithmetic_domains::integer_ranges::validate_anonymous_integer_range;
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::range_constraints::range_constraint_interval;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Suggest a declared range only when the exact selected callee returns an
/// integer. Spelling is diagnostic text, never callee selection authority.
pub(crate) fn overflow_operand_value_call_target(
    program: &dyn crate::CheckingInput,
    operand: ExpressionHandle,
) -> Option<String> {
    let call = program.expression_table.call_node(operand)?;
    let return_type = call_return_type(program, call)?;
    program
        .primitive_type_reference(return_type)
        .filter(|primitive| primitive.accepts_integer_literal())
        .map(|_| call.target.as_str().to_string())
}

pub(crate) fn call_return_type(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call) {
        return crate::proof_contracts::proof_embeddings::proof_int_type_reference(program);
    }
    if let Some(operator) = typed_trees::operator::resolve_named_expression_call(program, call)
        && operator.return_type.is_valid()
    {
        return Some(operator.return_type);
    }

    // A declared result belongs to the selected state, including calls through
    // ordinary borrowed receivers. Keep its full type reference for consumers
    // to check qualifications and bounds; do not infer from the caller's places.
    crate::machine_calls::calls::resolved_call_result_type(program, call)
}

/// The literal comparisons a machine-head callee's `ensures` states about
/// its reserved `result`. Exit checking proves them on every return, so they
/// bound each value the call produces exactly as a declared return range does.
pub(crate) fn ensured_call_result_interval(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    actuals: &super::call_result_bounds::ArgumentBounds,
) -> Option<Interval> {
    if let Some((machine, state)) =
        crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
        && machine.target_family_interface
        && program.machine_states(machine).first()?.symbol == state.symbol
    {
        let parameters = program.state_parameters(state);
        let members = crate::machine_calls::calls::target_family_members(program, machine)?;
        let mut union: Option<Interval> = None;
        for member in members {
            let member_machine = program.machine(member.machine)?;
            let member_entry = program.machine_states(member_machine).first()?;
            let member_callee =
                crate::machine_calls::calls::callee_entry_contract(program, member_entry.symbol)?;
            if member_callee.parameters.len() != parameters.len() {
                return None;
            }
            let remap = |items: &[(symbols::SymbolHandle, Interval)]| {
                parameters
                    .iter()
                    .zip(member_callee.parameters)
                    .filter_map(|(interface_param, member_param)| {
                        items
                            .iter()
                            .find(|(symbol, _)| *symbol == interface_param.symbol)
                            .map(|(_, interval)| (member_param.symbol, *interval))
                    })
                    .collect::<Vec<_>>()
            };
            let member_actuals = super::call_result_bounds::ArgumentBounds {
                values: remap(&actuals.values),
                lengths: remap(&actuals.lengths),
            };
            let member_interval = super::call_result_bounds::normal_return_interval(
                program,
                &member_callee,
                Interval::UNBOUNDED,
                &member_actuals,
            );
            union = Some(match union {
                Some(existing) => existing.union(member_interval),
                None => member_interval,
            });
        }
        let interval = union?;
        return (interval != Interval::UNBOUNDED).then_some(interval);
    }
    let callee = crate::machine_calls::calls::callee_entry_contract(program, call.target_symbol)?;
    // One contract projector serves every selected call. The old static-call
    // shortcut and the weaker ordinary-call parser disagreed on the same
    // result guarantee solely because of receiver spelling.
    let interval = super::call_result_bounds::normal_return_interval(
        program,
        &callee,
        Interval::UNBOUNDED,
        actuals,
    );
    (interval != Interval::UNBOUNDED).then_some(interval)
}

/// Declaration-owned bounds on this selected call's normal result, without
/// replaying its body or rereading mutable actuals. Call admission and normal
/// exit checking still owe the declaration's prerequisites and guarantees.
pub fn normal_return_integer_bounds(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<(Option<i64>, Option<i64>)> {
    if !call.selects_only_nominal_route()
        || !program
            .primitive_type_reference(call_return_type(program, call)?)
            .is_some_and(PrimitiveType::is_fixed_integer)
    {
        return None;
    }
    let interval = ensured_call_result_interval(
        program,
        call,
        &super::call_result_bounds::ArgumentBounds::default(),
    )?;
    if interval
        .low
        .zip(interval.high)
        .is_some_and(|(low, high)| low > high)
    {
        return None;
    }
    Some((interval.low, interval.high))
}

/// Invocation-specific return bounds consume the complete ordinary arrival
/// graph, including copied immutable locals and named state transfers. The
/// arrival owner retains effects and branch facts; this facade contributes no
/// separate expression evaluator or initial-state-only return recognizer.
pub(crate) fn infer_return_interval(
    program: &dyn crate::CheckingInput,
    callee_machine: &Machine,
    callee_state: &State,
    target_primitive: Option<PrimitiveType>,
    actuals: &[(
        symbols::SymbolHandle,
        Interval,
        Option<checked_trees::IntegerRangeRecipe>,
    )],
) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
    super::guard_narrowing::inferred_normal_return_interval(
        program,
        callee_machine,
        callee_state,
        target_primitive,
        actuals,
    )
}

/// S4 return-range ENFORCEMENT (companion to the call-site narrowing): when a
/// return type declares a literal `[a..=b]`, the returned value's proven
/// interval must fit inside it, else callers that trust the declared range are
/// unsound. No-op when the return type carries no range constraint (so plain
/// returns are unaffected -- range-constrained return types are a new
/// capability). `interval` is the return expression's already-analyzed interval.
pub(crate) fn enforce_declared_return_range(
    program: &dyn crate::CheckingInput,
    return_type: TypeReferenceHandle,
    interval: Interval,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // This enforcement owns the BRACKETED spelling, as its message says. A
    // return type whose bound comes from a declared DOMAIN belongs to the
    // scalar-result-domain check, which names the domain and its predicate;
    // letting this one speak first reported a `[a..=b]` the program never
    // wrote.
    if !super::range_constraints::declares_bracketed_range(program, return_type) {
        return;
    }
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

/// The shared return-range tail: analyze the return value retaining
/// decisions, then on a clean value re-resolve its stored carrier and add
/// the narrowing store obligation. Returns the analyzed interval.
#[allow(clippy::too_many_arguments)]
fn analyzed_return_interval_and_narrowing(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    return_expression: ExpressionHandle,
    environment: &ValueEnvironment,
    return_primitive: Option<PrimitiveType>,
    return_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> Interval {
    let before = diagnostics.len();
    let (interval, source, source_domain) = super::validate_value_range_retaining_decisions(
        program,
        machine,
        Some(state),
        return_expression,
        environment,
        return_primitive,
        return_domain,
        owner,
        diagnostics,
        decisions,
    );
    if diagnostics.len() == before {
        let (source, source_domain) = stored_value_carrier(
            program,
            machine,
            Some(state),
            return_expression,
            source,
            source_domain,
        );
        check_narrowing_assignment(
            return_primitive,
            return_domain,
            interval,
            source,
            source_domain,
            owner,
            diagnostics,
        );
    }
    interval
}

/// S4: analyze a transition VALUE-return expression and enforce its declared
/// return range. Gated on the return type carrying a range constraint -- only
/// then is the (otherwise un-validated) transition-value return analyzed, so
/// existing plain returns are byte-for-byte unaffected.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_return_value_range(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    return_expression: ExpressionHandle,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) {
    let return_primitive = program.primitive_type_reference(state.return_type);
    let return_domain = program.arithmetic_domain_for_type_reference(state.return_type);
    enforce_symbolic_range(
        program,
        machine,
        Some(state),
        state.return_type,
        return_expression,
        environment,
        owner,
        diagnostics,
    );
    if let Some((interval, _, _)) = validate_anonymous_integer_range(
        program,
        state.return_type,
        return_expression,
        owner,
        diagnostics,
    ) {
        enforce_declared_return_range(program, state.return_type, interval, owner, diagnostics);
        return;
    }
    if range_constraint_interval(program, state.return_type).is_some() {
        // Range-constrained return: analyze (emitting any overflow obligation, as
        // before), enforce the declared `[a..=b]`, and -- on a clean value -- the
        // narrowing store obligation too. A value proven within `[a..=b]` already
        // fits the type, so the narrowing check adds no rejection here; it is
        // present only for uniformity with the unconstrained branch.
        let interval = analyzed_return_interval_and_narrowing(
            program,
            machine,
            state,
            return_expression,
            environment,
            return_primitive,
            return_domain,
            owner,
            diagnostics,
            decisions,
        );
        enforce_declared_return_range(program, state.return_type, interval, owner, diagnostics);
        return;
    }
    // Unconstrained return: analyze into the REAL diagnostics, emitting any exact-
    // arithmetic overflow obligation just like every other value-binding boundary
    // (`_ -> (x + y)` with full-range Exact operands would otherwise wrap silently).
    // On a clean value, add the narrowing store obligation too -- a value that fits
    // its source type but not the return type (`-> i8 { _ -> (300) }`) is a silent
    // truncation.
    analyzed_return_interval_and_narrowing(
        program,
        machine,
        state,
        return_expression,
        environment,
        return_primitive,
        return_domain,
        owner,
        diagnostics,
        decisions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn enforce_symbolic_range(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    return_type: TypeReferenceHandle,
    return_expression: ExpressionHandle,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if crate::proof_contracts::contract_entailment::symbolic_range_contains(
        program,
        machine,
        state,
        return_type,
        return_expression,
        environment,
    ) == Some(false)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} has a value not provably within its declared symbolic const range"
        )));
    }
}
