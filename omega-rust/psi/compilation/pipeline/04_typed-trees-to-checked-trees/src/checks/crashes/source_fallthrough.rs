//! Stable source-site consequences of selected and earlier unselected arms.
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
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

pub(super) struct SiteFallthrough {
    pub location: CrashSiteLocation,
    pub guards: Vec<(ExpressionHandle, bool)>,
    pub entry_value_consequences: Vec<CrashPredicateIdentity>,
}

/// A call in a selected target (including its argument evaluation) observes
/// that edge's guard. Calls evaluating the guard itself do not. Resolve the
/// occurrence with the shared call traversal, then retain only entry-stable
/// consequences captured where the guard was evaluated.
pub(super) fn selected_call_path(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    location: checked_trees::CrashCallSiteLocation,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Option<SiteFallthrough> {
    let mut guards = Vec::new();
    let Some(state) = program.state_in_machine(machine, location.state()) else {
        return None;
    };
    let ordinal = location.statement_ordinal() as usize;
    let Some(transition) = program
        .statement_table
        .statement_at(state, ordinal)
        .and_then(|statement| statement.as_transition())
    else {
        return None;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return None;
    };
    let Some(target) = validation::transition_call_target(
        program,
        machine,
        state,
        ordinal,
        location.call_ordinal() as usize,
    )
    .filter(|target| target.is_valid()) else {
        return None;
    };
    collect_stable_consequences(
        program,
        machine.symbol,
        state.symbol,
        ordinal,
        guard,
        target != transition.target,
        parameter_names,
        content_conservation,
        &mut guards,
    );
    Some(SiteFallthrough {
        location: CrashSiteLocation::new(state.symbol, location.statement_ordinal()),
        guards,
        entry_value_consequences: super::entry_guards::entry_value_consequences_at(
            program,
            machine.symbol,
            state.symbol,
            ordinal,
            guard,
            target != transition.target,
        ),
    })
}

pub(super) fn collect(
    program: &dyn ::validation::CheckingInput,
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
            .indexed_statements(state.statement_nodes)
        {
            // Every statement — a crash exit, a call-bearing statement, or an
            // ordinary step — observes the facts earlier unselected arms
            // established. Provenance was already resolved at each guard's
            // own statement, so the same entry-term conjuncts cover a checked
            // call exactly the way they cover a crash exit.
            let mut retained = Vec::new();
            let mut entry_value_consequences = Vec::new();
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
                entry_value_consequences.extend(super::entry_guards::entry_value_consequences_at(
                    program,
                    machine.symbol,
                    state.symbol,
                    evaluated_at,
                    guard,
                    negated,
                ));
            }
            let is_crash_exit = matches!(
                statement,
                StatementNode::Transition(transition)
                    if matches!(transition.exit, TransitionExit::Crash(_))
            );
            if is_crash_exit || !retained.is_empty() || !entry_value_consequences.is_empty() {
                sites.push(SiteFallthrough {
                    location: CrashSiteLocation::new(
                        state.symbol,
                        u32::try_from(ordinal).expect("statement ordinal fits u32"),
                    ),
                    guards: retained,
                    entry_value_consequences,
                });
            }
            let Some(transition) = statement.as_transition() else {
                continue;
            };
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
    program: &dyn ::validation::CheckingInput,
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
    match super::boolean_structure(program, expression, negated) {
        super::BooleanStructure::Operand(operand, polarity) => collect_stable_consequences(
            program,
            machine,
            state,
            evaluated_at,
            operand,
            polarity,
            parameter_names,
            content_conservation,
            output,
        ),
        super::BooleanStructure::Pair(left, right, true) => {
            collect_stable_consequences(
                program,
                machine,
                state,
                evaluated_at,
                left,
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
                right,
                negated,
                parameter_names,
                content_conservation,
                output,
            );
        }
        super::BooleanStructure::Pair(_, _, false) | super::BooleanStructure::Atom => {}
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
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => true,
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
