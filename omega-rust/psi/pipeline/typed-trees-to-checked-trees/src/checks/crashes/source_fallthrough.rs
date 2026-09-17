//! Stable source-site consequences of earlier, unselected transition arms.
//!
//! A guard an unselected arm leaves behind is a fact about the values its
//! reads held at that statement. A leaf still speaks for an invocation-entry
//! operand only while the shared entry-provenance law proves — at the guard's
//! own statement — that the read denotes the operand its canonical spelling
//! claims. That is the same admission incoming edge guards and call-actual
//! substitution already use, so a pristine mutable snapshot, an immutable
//! field projection, or a state parameter uniformly bound across every named
//! arrival travels the fallthrough exactly like an immutable scalar, while a
//! written binding, a merely name-alike local, or an opaque read keeps no
//! entry identity. Facts project before canonicalizing names, so an opaque
//! sibling call cannot erase a safe conjunct or impersonate a parameter.

use checked_trees::{CrashPredicateIdentity, CrashSiteLocation};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

pub(super) struct SiteFallthrough {
    pub location: CrashSiteLocation,
    pub guards: Vec<(ExpressionHandle, bool)>,
}

pub(super) fn collect(
    program: &TypedTrees,
    machine: &Machine,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Vec<SiteFallthrough> {
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        // Each retained guard carries the ordinal of its own transition
        // statement: provenance is resolved where the read ran, and a later
        // write cannot unmake the entry fact the unselected arm established.
        let mut guards = Vec::new();
        for (ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            if matches!(transition.exit, TransitionExit::Crash(_)) {
                let mut retained = Vec::new();
                for &(guard, negated, evaluated_at) in &guards {
                    collect_stable_consequences(
                        program,
                        machine.symbol,
                        state.symbol,
                        evaluated_at,
                        guard,
                        negated,
                        parameter_names,
                        content_conservation,
                        &mut retained,
                    );
                }
                sites.push(SiteFallthrough {
                    location: CrashSiteLocation::new(
                        state.symbol,
                        u32::try_from(ordinal).expect("statement ordinal fits u32"),
                    ),
                    guards: retained,
                });
            }
            match transition.guard {
                TransitionGuardNode::When(guard)
                    if program
                        .statement_table
                        .transition_target_is_valid(transition.target)
                        && !transition.continuation.is_valid() =>
                {
                    guards.push((guard, true, ordinal));
                }
                _ => guards.clear(),
            }
        }
    }
    sites
}

#[allow(clippy::too_many_arguments)]
fn collect_stable_consequences(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    evaluated_at: usize,
    expression: ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<(ExpressionHandle, bool)>,
) {
    if holds_entry_meaning(
        program,
        machine,
        state,
        evaluated_at,
        expression,
        parameter_names,
        content_conservation,
    ) {
        output.push((expression, negated));
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_stable_consequences(
                program,
                machine,
                state,
                evaluated_at,
                unary.operand,
                !negated,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_stable_consequences(
                program,
                machine,
                state,
                evaluated_at,
                binary.left,
                negated,
                parameter_names,
                content_conservation,
                output,
            );
            collect_stable_consequences(
                program,
                machine,
                state,
                evaluated_at,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_stable_consequences(
                    program,
                    machine,
                    state,
                    evaluated_at,
                    operand,
                    equality_is_negated == literal,
                    parameter_names,
                    content_conservation,
                    output,
                );
            }
        }
        _ => {}
    }
}

/// Whether every leaf of `expression` denotes, at `evaluated_at`, the
/// invocation-entry operand its canonical spelling claims. Literals need no
/// provenance; a binding-rooted read holds only when
/// `facts::crash_entry_operand` — the same law incoming-guard admission and
/// call-actual substitution share — resolves the leaf to exactly the identity
/// its spelling encodes. A pristine `mut` parameter or local read therefore
/// qualifies, as does an immutable field projection or a parameter every
/// named arrival binds to one entry operand; a write or exclusive borrow
/// before the read, a divergent arrival, or a local whose own name claims no
/// parameter position does not. Anything else — a mutated or computed place,
/// a machine root, a call — stays opaque rather than launder current storage
/// into an entry snapshot.
fn holds_entry_meaning(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    evaluated_at: usize,
    expression: ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => true,
        ExpressionNode::Unary(unary) => holds_entry_meaning(
            program,
            machine,
            state,
            evaluated_at,
            unary.operand,
            parameter_names,
            content_conservation,
        ),
        ExpressionNode::Binary(binary) => {
            holds_entry_meaning(
                program,
                machine,
                state,
                evaluated_at,
                binary.left,
                parameter_names,
                content_conservation,
            ) && holds_entry_meaning(
                program,
                machine,
                state,
                evaluated_at,
                binary.right,
                parameter_names,
                content_conservation,
            )
        }
        _ => {
            crate::facts::crash_entry_operand(program, machine, state, evaluated_at, expression)
                .map(CrashPredicateIdentity::from_expression)
                == Some(crate::facts::canonical_crash_path_predicate(
                    program,
                    expression,
                    false,
                    parameter_names,
                    content_conservation,
                ))
        }
    }
}
