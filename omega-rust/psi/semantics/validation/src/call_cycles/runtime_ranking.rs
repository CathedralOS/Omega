//! Runtime call components share an authored ranking. Equality edges may
//! forward that rank, but must form a DAG: every complete cycle then contains
//! a strict decrease. No source subject or public termination claim is added.

mod comparison;
mod meaning;
mod prefix;
mod projection;

#[cfg(test)]
mod tests;

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use comparison::Comparison;
use projection::RankProjection;

pub(super) fn extend_runtime_adjacency(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    adjacency: &mut [Vec<usize>],
) {
    for (index, machine) in program.machines().iter().enumerate() {
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let mut targets = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    super::collect_statement_dependency_symbols(program, statement, &mut targets);
                }
            }
        }
        for target in targets {
            if let Some(callee) = target_machine(program, target)
                && callee != index
            {
                adjacency[index].push(callee);
            }
        }
        adjacency[index].sort_unstable();
        adjacency[index].dedup();
    }
}

pub(super) fn admitted_components(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    adjacency: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    super::strongly_connected_components(adjacency)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                && component
                    .iter()
                    .all(|index| !proof_only.is_proof_machine(program, &program.machines()[*index]))
                && check_component(program, adjacency, component).is_ok()
        })
        .collect()
}

pub(super) fn check_component(
    program: &TypedTrees,
    adjacency: &[Vec<usize>],
    component: &[usize],
) -> Result<(), &'static str> {
    let Some(ranks) = component
        .iter()
        .map(|index| RankProjection::resolve(program, &program.machines()[*index]))
        .collect::<Option<Vec<_>>>()
    else {
        return Err("a member lacks a supported exact ranking witness");
    };
    if !ranks.iter().all(|rank| rank.same_order(&ranks[0])) {
        return Err("members do not share the same ranking order");
    }
    let frames = crate::calls::CallFrameResolver::new(program);
    let mut equal_edges = vec![Vec::new(); component.len()];
    for (position, index) in component.iter().copied().enumerate() {
        let machine = &program.machines()[index];
        // This slice transports the entry's exact ranked parameter. A state
        // rebinding or internal loop needs its own arrival judgment, not a
        // same-spelled parameter in another state.
        let [state] = program.machine_states(machine) else {
            return Err("the ranking needs entry-to-state arrival evidence");
        };
        let mut observed = Vec::new();
        let mut guards = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                if prefix::preserves_rank(
                    program,
                    machine,
                    &ranks[position],
                    statement,
                    frames.as_ref(),
                ) {
                    continue;
                }
                match statement {
                    StatementNode::AssemblyFact(_) => continue,
                    StatementNode::LocalData(local)
                        if expression_is_inert(program, machine.symbol, local.initial_value) =>
                    {
                        continue;
                    }
                    StatementNode::Expression(expression)
                        if expression_is_inert(program, machine.symbol, *expression) =>
                    {
                        continue;
                    }
                    // Do not replay an entry-relative projection or a guard
                    // across writes, calls, aliases, or unknown effects.
                    _ => {
                        return Err(
                            "a write, call, or alias invalidates the entry-relative ranking",
                        );
                    }
                }
            };
            if transition.continuation.is_valid() {
                return Err("a non-tail call edge returns into a continuation");
            }
            let guard = match transition.guard {
                TransitionGuardNode::Always => ExpressionHandle::invalid(),
                TransitionGuardNode::When(guard)
                    if expression_is_inert(program, machine.symbol, guard) =>
                {
                    guard
                }
                TransitionGuardNode::When(_) => {
                    return Err("a guard has effects or non-builtin operator meaning");
                }
            };
            let mut site_guards = guards.clone();
            if guard.is_valid() {
                site_guards.push((guard, true));
                guards.push((guard, false));
            }
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    let Some(callee) = target_machine(program, path.symbol) else {
                        return Err("a call target has no exact machine identity");
                    };
                    let callee_machine = &program.machines()[callee];
                    let Some(entry) = program.machine_states(callee_machine).first() else {
                        return Err("a call target has no checked entry binding");
                    };
                    if path.symbol != callee_machine.symbol && path.symbol != entry.symbol {
                        return Err("a subordinate-state call needs its own arrival evidence");
                    }
                    if callee == index {
                        return Err("an internal state loop needs its own ranking evidence");
                    }
                    let arguments = program.statement_table.expression_handles(*arguments);
                    if !arguments
                        .iter()
                        .all(|argument| expression_is_inert(program, machine.symbol, *argument))
                    {
                        return Err("a call argument has effects or non-builtin operator meaning");
                    }
                    observed.push(callee);
                    let Some(callee_position) = component.iter().position(|index| *index == callee)
                    else {
                        continue;
                    };
                    let parameters = program.state_parameters(entry);
                    if arguments.len()
                        != parameters
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .count()
                    {
                        return Err("call arguments do not match the exact entry parameters");
                    }
                    let Some(argument) = arguments.get(ranks[callee_position].argument_position)
                    else {
                        return Err("the ranked parameter has no corresponding actual argument");
                    };
                    match comparison::argument_comparison(
                        program,
                        &ranks[position],
                        *argument,
                        &site_guards,
                    ) {
                        Some(Comparison::Strict) => {}
                        Some(Comparison::Equal) => equal_edges[position].push(callee_position),
                        None => {
                            return Err(
                                "ranking preservation or strict DECREASE is unproven at a call site",
                            );
                        }
                    }
                }
                TransitionTargetNode::Value(value)
                    if expression_is_inert(program, machine.symbol, *value) => {}
                TransitionTargetNode::Terminal => {}
                _ => return Err("a non-tail call or unknown effect prevents ranking admission"),
            }
        }
        // A pair-level strict occurrence cannot hide an unclassified call,
        // and a legacy spelling edge cannot supply missing exact custody.
        if adjacency[index]
            .iter()
            .filter(|target| component.contains(target))
            .any(|target| !observed.contains(target))
        {
            return Err("an internal call occurrence lacks a classified tail edge");
        }
    }
    if equality_edges_are_acyclic(&equal_edges) {
        Ok(())
    } else {
        Err("a preserving cycle has no strict measure DECREASE")
    }
}

fn equality_edges_are_acyclic(adjacency: &[Vec<usize>]) -> bool {
    super::strongly_connected_components(adjacency)
        .iter()
        .all(|component| component.len() == 1 && !adjacency[component[0]].contains(&component[0]))
}

fn target_machine(program: &TypedTrees, symbol: SymbolHandle) -> Option<usize> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().position(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    })
}

fn expression_is_inert(
    program: &TypedTrees,
    machine: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return true;
    }
    let inert = |expression| expression_is_inert(program, machine, expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => inert(atomic.value),
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Member(member) => inert(member.receiver),
        ExpressionNode::Binary(binary) => {
            meaning::binary_is_builtin(program, machine, expression, binary)
                && inert(binary.left)
                && inert(binary.right)
        }
        ExpressionNode::Unary(unary) => inert(unary.operand),
        ExpressionNode::Cast(cast) => inert(cast.value),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| inert(field.value)),
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .all(|item| inert(*item)),
        // A borrow can expose the ranked value; a call or indexed operation
        // can have selected behavior not described by this pure rank slice.
        ExpressionNode::Borrow(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Range(_) => false,
    }
}
