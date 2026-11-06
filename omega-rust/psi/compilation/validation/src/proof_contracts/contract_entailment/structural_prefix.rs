//! Observations at one executed state prefix.
//!
//! The structural path owner supplies preceding locals, guards and captured
//! normal results to each visitor. A visitor cannot obtain facts from a later
//! statement or a recurrent successor. State Requires describe this invocation;
//! machine Requires apply here only when this is the machine's entry state.

mod private_arrivals;
pub use private_arrivals::{private_invocation_entry_is_closed, with_private_invocation_inputs};

pub(crate) use private_arrivals::{
    ClosedArrivals, ProgramPrivateArrivalIndex, private_invocation_index,
};
pub(super) use private_arrivals::{observe_private_arrivals, observe_private_arrivals_batch};

use super::citations::{CitationFacts, machine_requires_facts};
use super::structural_case_arms::{
    recognize_structural_case_arms, recognize_structural_case_arms_batched,
    recognize_structural_state_prefix_batched, recognize_structural_state_prefixes,
    structural_arm_judge,
};
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State};

#[derive(Clone, Copy, PartialEq, Eq)]
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
            crate::queries::expression_traversal::collect_transition_target_expression_trees(
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
        crate::queries::expression_traversal::collect_transition_target_expression_trees(
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

/// Maximal compatible schedules supplied by the existing prefix owner. Guard,
/// target and continuation boundaries remain exactly those of that operation;
/// grouping does not infer execution order from an inventory of syntax.
#[derive(Default)]
pub(super) struct ExpressionPrefixGroups {
    pub groups: Vec<Vec<typed_trees::expression::ExpressionHandle>>,
    pub unavailable: Vec<typed_trees::expression::ExpressionHandle>,
}

pub(super) fn expression_prefix_groups(
    program: &dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
) -> ExpressionPrefixGroups {
    let mut plan = ExpressionPrefixGroups::default();
    let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
        return plan;
    };
    for (_, expression) in program.tables.expression_roots(statement) {
        if !expression.is_valid() {
            continue;
        }
        let Some(prefix) = expression_prefix_roots(program, state, statement_index, expression)
        else {
            plan.unavailable.push(expression);
            continue;
        };
        if let Some(group) = plan
            .groups
            .iter_mut()
            .find(|group| group.starts_with(&prefix) || prefix.starts_with(group))
        {
            if prefix.len() > group.len() {
                *group = prefix;
            }
        } else {
            plan.groups.push(prefix);
        }
    }
    plan
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

/// Output selection for the common statement completion, never execution policy.
#[derive(Default)]
pub(super) struct StatementObservationRequest {
    pub values: Vec<typed_trees::expression::ExpressionHandle>,
    pub operations: Vec<typed_trees::expression::ExpressionHandle>,
}

pub(super) trait StructuralPrefixVisitor {
    fn site(&self) -> StructuralPrefixSite;

    fn statement_observations(&self) -> Option<StatementObservationRequest> {
        None
    }

    /// The statement owner supplies the same normal completion used to advance
    /// its frontier. The input remains the independently admitted entry cut.
    fn observe_completed<'program>(
        &mut self,
        observation: PrefixObservation<'_, 'program>,
        _completed: &super::structural_judgment::normal_returns::CompletedStatement<'program>,
    ) {
        self.observe(observation);
    }

    /// One authenticated body arrival after its actuals completed. Coverage
    /// consumers distinguish these occurrences from lexical ancestry and from
    /// an unexecuted callable selection.
    fn entered_body(
        &mut self,
        _caller: SymbolHandle,
        _statement: usize,
        _expression: Option<typed_trees::expression::ExpressionHandle>,
        _transition_target: typed_trees::statement::TransitionTargetHandle,
        _entry: SymbolHandle,
    ) {
    }

    /// A formed guard excludes this call before any of its operands execute.
    /// This covers a syntactic use without inventing an invocation or values.
    fn excluded_body(
        &mut self,
        _caller: SymbolHandle,
        _statement: usize,
        _expression: Option<typed_trees::expression::ExpressionHandle>,
        _transition_target: typed_trees::statement::TransitionTargetHandle,
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

    /// A complete positive judgment may be published from an independently
    /// checked caller frontier. Negative or partial trials never replace the
    /// original lexical-root observation.
    fn has_positive_proof(&self) -> bool {
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
        let Some(judge) = arm_premises(program, judge, hypotheses, equations, guarantees) else {
            return;
        };
        let Some(state) = program.state_in_machine(machine, self.site().state) else {
            self.unresolved(machine);
            return;
        };
        self.observe((program, machine, state, judge, environment));
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_non_contradictory<'program>(
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
        let _arm_guard = phase_profile::guard("psi.05.numeric.visit.arm_judge");
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        drop(_arm_guard);
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
    mut visitor: &mut dyn StructuralPrefixVisitor,
) -> Option<()> {
    observe_runtime_state_prefix_batched(
        program,
        machine,
        state,
        std::slice::from_mut(&mut visitor),
    )
}

/// One walk of `state`'s prefix shared by visitors that sit at one site and
/// complete alike; each receives what its own walk would have delivered.
pub(super) fn observe_runtime_state_prefix_batched(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    visitors: &mut [&mut (dyn StructuralPrefixVisitor + '_)],
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
    recognize_structural_state_prefix_batched(
        program,
        machine,
        state,
        &judge,
        &crate::proof_only_classification(program),
        &mut diagnostics,
        visitors,
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

/// One arm delivered at one site to every visitor waiting there.
///
/// Visitors at the same site receive identical premises: the walk's judge,
/// the arm's case hypotheses and equations, and the guarantees established so
/// far. `StructuralPrefixVisitor::visit` establishes those premises for each
/// visitor separately; in a long proof the shared walk of
/// `rat_close_triangle_split` (`core/rat.omg`) rebuilt the same arm judge for
/// each of a call's preconditions. Here the arm judge, its guarantee intake
/// and its contradiction test run once, at the first visitor that needs them,
/// and each visitor observes its own copy. The sequence each visitor sees is
/// exactly `visit`'s.
pub(super) struct SiteArm<'arm, 'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'arm Machine,
    judge: &'arm StructuralJudge<'program>,
    environment: &'arm [(String, StructuralTerm)],
    hypotheses: &'arm [(String, StructuralTerm)],
    equations: &'arm [(StructuralTerm, StructuralTerm)],
    guarantees: &'arm CitationFacts,
    /// `Some(None)`: the arm's premises are contradictory.
    prepared: Option<Option<StructuralJudge<'program>>>,
}

impl<'arm, 'program> SiteArm<'arm, 'program> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'arm Machine,
        judge: &'arm StructuralJudge<'program>,
        environment: &'arm [(String, StructuralTerm)],
        hypotheses: &'arm [(String, StructuralTerm)],
        equations: &'arm [(StructuralTerm, StructuralTerm)],
        guarantees: &'arm CitationFacts,
    ) -> Self {
        Self {
            program,
            machine,
            judge,
            environment,
            hypotheses,
            equations,
            guarantees,
            prepared: None,
        }
    }

    /// `visitor.visit(..)` with this arm's premises.
    pub(super) fn deliver<V: StructuralPrefixVisitor + ?Sized>(&mut self, visitor: &mut V) {
        visitor.entered();
        if visitor.is_complete() {
            return;
        }
        if self.prepared.is_none() {
            self.prepared = Some(arm_premises(
                self.program,
                self.judge,
                self.hypotheses,
                self.equations,
                self.guarantees,
            ));
        }
        let Some(Some(judge)) = &self.prepared else {
            return;
        };
        let Some(state) = self
            .program
            .state_in_machine(self.machine, visitor.site().state)
        else {
            visitor.unresolved(self.machine);
            return;
        };
        visitor.observe((
            self.program,
            self.machine,
            state,
            judge.clone(),
            self.environment,
        ));
    }
}

/// The arm judge an observation receives: the walk's judge with the arm's case
/// hypotheses and equations and the guarantees established so far, or `None`
/// when those premises contradict each other.
fn arm_premises<'program>(
    program: &'program dyn crate::CheckingInput,
    judge: &StructuralJudge<'program>,
    hypotheses: &[(String, StructuralTerm)],
    equations: &[(StructuralTerm, StructuralTerm)],
    guarantees: &CitationFacts,
) -> Option<StructuralJudge<'program>> {
    let arm_guard = phase_profile::guard("psi.05.numeric.visit.arm_judge");
    let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
    if judge.hypotheses_contradictory {
        return None;
    }
    guarantees.intake(&mut judge);
    drop(arm_guard);
    // Short-circuit guards can create numerically impossible alternatives
    // even when their constructor hypotheses are consistent. Refute only
    // from admitted prefix premises, before observing the requested value;
    // failed goal formation must never become evidence of unreachability.
    (!phase_profile::scope("psi.05.numeric.visit.contra", || {
        judge.integer_premises_contradictory()
    }))
    .then_some(judge)
}
