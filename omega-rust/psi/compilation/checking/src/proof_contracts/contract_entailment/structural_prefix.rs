//! Observations at one executed state prefix.
//!
//! The structural path owner supplies preceding locals, guards and captured
//! normal results to each visitor. A visitor cannot obtain facts from a later
//! statement or a recurrent successor. State Requires describe this invocation;
//! machine Requires apply here only when this is the machine's entry state.

use super::citations::{CitationFacts, machine_requires_facts};
use super::structural_case_arms::recognize_structural_state_prefix;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State};

#[derive(Clone, Copy)]
pub(super) struct StructuralPrefixSite {
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub in_transition_target: bool,
}

/// A primary transition operand executes under that transition's own guard.
/// Guard expressions and continuation targets must not inherit that premise.
/// All expression observers use this same occurrence-based classification.
pub(super) fn expression_is_transition_target(
    program: &dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
        .is_some_and(|statement| {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                return false;
            };
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_transition_target_expression_trees(
                program,
                transition.target,
                &mut expressions,
            );
            expressions.contains(&expression)
        })
}

/// The ordered expression roots through this observation's enclosing root.
/// A call in the second named argument must see the first argument's effects.
/// The root inventory includes control-flow alternatives, so it is not itself
/// an execution schedule: an established primary guard is not evaluated again,
/// and continuation observations remain with the transition control owner.
pub(super) fn expression_prefix_roots(
    program: &dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    use typed_trees::statement::{StatementNode, StatementRootRole};
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    if let StatementNode::Transition(transition) = statement
        && program
            .statement_table
            .transition_target_is_valid(transition.continuation)
    {
        let mut continuation = Vec::new();
        crate::expression_traversal::collect_transition_target_expression_trees(
            program,
            transition.continuation,
            &mut continuation,
        );
        if continuation.contains(&expression) {
            return None;
        }
    }
    let guarded = expression_is_transition_target(program, state, statement_index, expression);
    let mut roots = program
        .tables
        .expression_roots(statement)
        .into_iter()
        .filter(|(role, root)| {
            root.is_valid() && !(guarded && *role == StatementRootRole::TransitionGuard)
        })
        .map(|(_, root)| root)
        .collect::<Vec<_>>();
    let mut owners = roots.iter().enumerate().filter(|(_, root)| {
        crate::expression_owners::expression_contains(program, **root, expression)
    });
    let (position, _) = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    roots.truncate(position + 1);
    Some(roots)
}

pub(super) trait StructuralPrefixVisitor {
    fn site(&self) -> StructuralPrefixSite;

    #[allow(clippy::too_many_arguments)]
    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    );
}

pub(super) fn observe_runtime_state_prefix(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    visitor: &mut dyn StructuralPrefixVisitor,
) -> Option<()> {
    let entry = program.machine_states(machine).first()?;
    let mut requires = if state.symbol == entry.symbol {
        machine_requires_facts(program, machine)
    } else {
        Vec::new()
    };
    requires.extend(
        crate::proof_contracts::proof_facts::contract_expression_facts(
            program,
            program.state_contracts(state).iter(),
            typed_trees::signature::SignatureContractKind::Requires,
        ),
    );
    let judge = StructuralJudge::from_runtime_state_requires(program, machine, state, &requires);
    let mut diagnostics = Vec::new();
    recognize_structural_state_prefix(
        program,
        machine,
        state,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        visitor,
    )?;
    diagnostics.is_empty().then_some(())
}
