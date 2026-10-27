//! Observations at one executed state prefix.
//!
//! The structural path owner supplies preceding locals, guards and captured
//! normal results to each visitor. A visitor cannot obtain facts from a later
//! statement or a recurrent successor. State Requires describe this invocation;
//! machine Requires apply here only when this is the machine's entry state.

mod private_arrivals;

pub(super) use private_arrivals::observe_private_arrivals;

use super::citations::{CitationFacts, machine_requires_facts};
use super::structural_case_arms::{
    recognize_structural_case_arms, recognize_structural_state_prefix,
    recognize_structural_state_prefixes, structural_arm_judge,
};
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

/// One reached arm of a visited prefix, in `observe`'s reading order: the
/// program facts, the site's machine and resolved state, the arm's substitute
/// judge carrying the selected call's published guarantees, and the
/// environment earlier siblings established.
pub(super) type PrefixObservation<'a, 'program> = (
    &'program dyn crate::CheckingInput,
    &'a Machine,
    &'program State,
    StructuralJudge<'program>,
    &'a [(String, StructuralTerm)],
);

pub(super) trait StructuralPrefixVisitor {
    fn site(&self) -> StructuralPrefixSite;

    /// One authenticated body arrival after its actuals completed. Coverage
    /// consumers distinguish these occurrences from lexical ancestry and from
    /// an unexecuted callable selection.
    fn entered_body(
        &mut self,
        _caller: SymbolHandle,
        _statement: usize,
        _expression: Option<typed_trees::expression::ExpressionHandle>,
        _entry: SymbolHandle,
    ) {
    }

    /// A ranked entry starts a fresh induction interval. Observers checking
    /// that interval stop at the next ranked invocation, before its body.
    fn stops_at_machine(&self, _machine: SymbolHandle) -> bool {
        false
    }

    /// True once no further path observation can change the visitor's verdict.
    fn is_complete(&self) -> bool {
        false
    }

    /// Every reached arm, before its premises are established - including an
    /// arm whose hypotheses contradict.
    fn entered(&mut self) {}

    /// The site's state could not be resolved at this arm.
    fn unresolved(&mut self, _machine: &Machine) {}

    /// One reached arm with established premises and a resolved site state. A
    /// visitor that owns a different entry point overrides `visit` and leaves
    /// this unreachable.
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>);

    /// A closed private invocation supplies an independently checked enclosing
    /// root and actual entry frame. Consumers own any additional theorem use:
    /// operand formation does not acquire recursive conclusions from this hook.
    fn observe_private_arrival<'program>(
        &mut self,
        _root: &Machine,
        observation: PrefixObservation<'_, 'program>,
    ) {
        self.observe(observation);
    }

    /// Establish this observation's premises - the arm's hypotheses and
    /// equations, its contradiction test, and the selected call's published
    /// guarantees - then resolve the site's state and hand the arm to
    /// `observe`. Observing a completed value never restates that sequence.
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
    ) {
        self.entered();
        if self.is_complete() {
            return;
        }
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site().state) else {
            self.unresolved(machine);
            return;
        };
        self.observe((program, machine, state, judge, environment));
    }
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

pub(super) fn observe_runtime_state_prefixes(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    visitors: &mut [&mut dyn StructuralPrefixVisitor],
) -> Vec<bool> {
    let Some(entry) = program.machine_states(machine).first() else {
        return vec![false; visitors.len()];
    };
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
    recognize_structural_state_prefixes(
        program,
        machine,
        state,
        &judge,
        &crate::proof_only_classification(program),
        visitors,
    )
}

/// An acyclic invocation retains every incoming substitution and guard. Start
/// at machine entry, including all named primary/continuation edges; checking
/// only the destination's outgoing graph would miss recurrent predecessors.
/// Recurrent observations instead use their independently established state
/// contract through `observe_runtime_state_prefix`.
pub(super) fn observe_acyclic_invocation_prefix(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    visitor: &mut dyn StructuralPrefixVisitor,
) -> Option<()> {
    let entry = program.machine_states(machine).first()?;
    if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    ) {
        return None;
    }
    let requires = machine_requires_facts(program, machine);
    let judge = StructuralJudge::from_runtime_state_requires(program, machine, entry, &requires);
    let mut diagnostics = Vec::new();
    recognize_structural_case_arms(
        program,
        machine,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        Some(visitor),
    )?;
    diagnostics.is_empty().then_some(())
}
