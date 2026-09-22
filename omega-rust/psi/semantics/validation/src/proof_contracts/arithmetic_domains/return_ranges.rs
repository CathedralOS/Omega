//! Inferred and declared return value ranges.

use crate::proof_contracts::arithmetic_domains::assignments::check_narrowing_assignment;
use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    validate_anonymous_integer_range, validate_value_range,
};
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::range_constraints::range_constraint_interval;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Suggest a declared range only when the exact selected callee returns an
/// integer. Spelling is diagnostic text, never callee selection authority.
pub(crate) fn overflow_operand_value_call_target(
    program: &TypedTrees,
    operand: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(operand) else {
        return None;
    };
    let return_type = call_return_type(program, call)?;
    program
        .primitive_type_reference(return_type)
        .filter(|primitive| primitive.accepts_integer_literal())
        .map(|_| call.target.as_str().to_string())
}

pub(crate) fn call_return_type(
    program: &TypedTrees,
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
pub(crate) fn resolve_unique_self_call_state<'program>(
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
                    [only] if only.is_self_receiver()
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
/// STRICTLY PERMISSIVE: the body is analyzed with an EMPTY environment (params at full
/// type width -> the widest possible result, so any caller's actual return is
/// within it). All return paths of the callee state are UNIONed -- a terminal
/// expression and/or transition VALUE targets (`{ cond -> v1 _ -> v2 }`). SOUND:
/// every path must be captured, so the state must be a LEAF value state -- if any
/// transition target is Named/SelfTarget (a return could come from a state we are
/// not analyzing, or a loop), we bail. The interval is trusted only if every
/// path's analysis is clean and the union is fully bounded. Recursion-guarded to
/// one level.
pub(crate) fn infer_return_interval(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    target_primitive: Option<PrimitiveType>,
) -> Option<Interval> {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
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

    let environment = ValueEnvironment::new();
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
            &environment,
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
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
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
    if let Some((interval, _)) = validate_anonymous_integer_range(
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
        let before = diagnostics.len();
        let (interval, source) = validate_value_range(
            program,
            machine,
            Some(state),
            return_expression,
            environment,
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
        environment,
        return_primitive,
        return_domain,
        owner,
        diagnostics,
    );
    if diagnostics.len() == before {
        check_narrowing_assignment(return_primitive, interval, source, owner, diagnostics);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn enforce_symbolic_range(
    program: &TypedTrees,
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
