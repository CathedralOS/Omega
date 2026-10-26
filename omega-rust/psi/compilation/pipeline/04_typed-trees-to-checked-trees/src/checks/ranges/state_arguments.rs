//! State argument facts: what every edge into a state proves about its
//! arguments, merged per state parameter so the range check can seed those
//! facts before the state's statements run.
//!
//! `ranges::check_indexed_accesses` calls `collect_state_argument_facts` once
//! per machine, and `seed_state_argument_facts` for each state after seeding
//! the state's `requires`. `collect_state_argument_facts` iterates to a fixed
//! point. The first pass runs the entry state; each later pass reruns the
//! states whose incoming facts changed. Running a state builds a new
//! `facts::RangeFacts` from its parameters, its `requires` and the facts
//! collected for it so far, then replays each statement through the shared
//! statement transfer (`statements`), recording what every call and
//! transition to a state of the same machine carries. The recorded edges are
//! then merged per target state:
//!
//! - an exact length, an exact integer value and non-negativity survive only
//!   if every edge agrees;
//! - a minimum length takes the smallest value and an exclusive upper bound
//!   the largest, and one edge without a bound removes it;
//! - a collection and index pair proved in range, and a receiver length,
//!   survive only if every edge carries them.
//!
//! Edges into the entry state are dropped, since the entry can be called with
//! any argument its declaration permits. If the facts have not settled after
//! `MAX_PROPAGATION_PASSES` passes, no facts are published.
//!
//! `statements` implements the statement transfer's visitor hooks,
//! `expressions` walks call and `match` expressions, and `calls` records one
//! edge's facts for the target state's parameters. Tests compare the result
//! with `collect_state_argument_facts_whole_pass`, which reruns every reached
//! state on each pass.

pub(crate) mod calls;
mod expressions;
mod statements;

use super::expression_transfer::ExpressionTransfer;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::facts::RangeFacts;
use super::seed_state_requires;

use self::statements::collect_state_argument_facts_from_statement;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StateArgumentFacts {
    state: SymbolHandle,
    machine: SymbolHandle,
    parameters: Vec<ParameterFacts>,
    index_proofs: MergedIndexProofs,
    receiver_lengths: Option<Vec<super::facts::ReceiverLength>>,
    /// Argument-referent storage facts, spelled under the destination
    /// parameter's name (`put(&mut buf)` delivers `buf.i`'s bound as
    /// `param.i`'s bound). `None` is "not yet collected"; an edge that
    /// supplies no row for a label drops it, exactly like the receiver
    /// lengths above.
    storage: Option<Vec<StorageFacts>>,
}

/// Carry the dependency with its bounds. A label alone cannot survive a
/// renamed parameter or an intermediate state that never reads the field;
/// the canonical place also lets ordinary mutation invalidate the fact.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StorageFacts {
    expression: typed_trees::expression::ExpressionHandle,
    place: validation::CanonicalPlace,
    label: String,
    upper_bound: Option<i64>,
    non_negative: bool,
    minimum_length: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterFacts {
    symbol: SymbolHandle,
    name: String,
    length: MergedFact<usize>,
    /// A floor shared by all incoming collection values, not an exact extent.
    minimum_length: MergedBound,
    integer: MergedFact<i64>,
    /// The tightest EXCLUSIVE upper bound every incoming edge proves for
    /// this parameter's argument (R4 transport: an ensures-bounded value
    /// passed as a transition argument carries its bound into the param).
    /// The meet is the MAX over edges -- the weakest bound all satisfy;
    /// one unbounded edge poisons it.
    upper_bound: MergedBound,
    /// Every incoming edge proved its argument non-negative -- a literal
    /// `>= 0`, a proven label fact, or the callee's ensured `result >= 0`
    /// exit proof. A signed parameter owes this half to the index check;
    /// one unproven edge poisons the lane.
    non_negative: MergedFact<()>,
    /// Const binders this argument is strictly below on every incoming edge.
    /// `index < Count` in the source becomes the destination parameter's
    /// proof against `[T; Count]`. An edge that does not carry a binder
    /// drops it.
    below_const_binders: MergedBinders,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MergedBound {
    #[default]
    Unseen,
    Known(i64),
    Unbounded,
}

impl MergedBound {
    fn merge_lower(&mut self, value: Option<i64>) {
        self.merge_with(value, i64::min)
    }

    fn merge(&mut self, value: Option<i64>) {
        self.merge_with(value, i64::max)
    }

    fn merge_with(&mut self, value: Option<i64>, combine: impl Fn(i64, i64) -> i64) {
        *self = match (*self, value) {
            (Self::Unseen, Some(bound)) => Self::Known(bound),
            (Self::Known(existing), Some(bound)) => Self::Known(combine(existing, bound)),
            (_, None) | (Self::Unbounded, _) => Self::Unbounded,
        };
    }

    fn get(self) -> Option<i64> {
        match self {
            Self::Known(bound) => Some(bound),
            Self::Unseen | Self::Unbounded => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MergedFact<T> {
    #[default]
    Unseen,
    Known(T),
    Conflicting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParameterIndexProofSide {
    /// A destination parameter position -- the callee names it.
    Parameter(usize),
    /// Storage spelled under a root that survives the edge: machine storage
    /// (`self.field` names the same place in every state of this machine) or
    /// a transition argument's referent spelled under the destination
    /// parameter's name (`put(&mut buf)` delivers `buf.i` as `param.i`).
    Storage(String),
}

impl ParameterIndexProofSide {
    fn label<'a>(&'a self, parameters: &'a [ParameterFacts]) -> Option<&'a str> {
        match self {
            Self::Parameter(position) => parameters.get(*position).map(|p| p.name.as_str()),
            Self::Storage(label) => Some(label.as_str()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterIndexProof {
    collection: ParameterIndexProofSide,
    index: ParameterIndexProofSide,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MergedIndexProofs {
    proofs: Option<Vec<ParameterIndexProof>>,
}

impl<T: Copy + Eq> MergedFact<T> {
    fn merge(&mut self, value: Option<T>) {
        match (*self, value) {
            (Self::Unseen, Some(value)) => *self = Self::Known(value),
            (Self::Unseen | Self::Known(_), None) => *self = Self::Conflicting,
            (Self::Known(existing), Some(value)) if existing == value => {}
            (Self::Known(_), Some(_)) | (Self::Conflicting, _) => *self = Self::Conflicting,
        }
    }

    fn get(self) -> Option<T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unseen | Self::Conflicting => None,
        }
    }
}

/// Binders present on every incoming edge. `None` is unseen; the first edge
/// installs its set and later edges keep the intersection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MergedBinders {
    symbols: Option<Vec<SymbolHandle>>,
}

impl MergedBinders {
    fn merge(&mut self, incoming: &[SymbolHandle]) {
        let Some(existing) = &mut self.symbols else {
            self.symbols = Some(incoming.to_vec());
            return;
        };
        existing.retain(|symbol| incoming.contains(symbol));
    }

    fn get(&self) -> &[SymbolHandle] {
        self.symbols.as_deref().unwrap_or(&[])
    }
}

impl MergedIndexProofs {
    fn merge(&mut self, incoming: Vec<ParameterIndexProof>) {
        let Some(existing) = &mut self.proofs else {
            self.proofs = Some(incoming);
            return;
        };

        existing.retain(|proof| incoming.contains(proof));
    }

    fn get(&self) -> &[ParameterIndexProof] {
        self.proofs.as_deref().unwrap_or(&[])
    }
}

/// A defensive bound, not proof of convergence. No inferred argument facts
/// are published if any contribution remains unsettled when it is reached.
const MAX_PROPAGATION_PASSES: usize = 64;

pub(super) fn collect_state_argument_facts<'program>(
    program: &'program dyn ::validation::CheckingInput,
    field_lengths: &[(SymbolHandle, String, usize)],
    bound_lookup: &validation::ImmutableBoundLookup<'program>,
    machine: &'program Machine,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    calls: &[super::facts::RangeCallContext<'_>],
    operators: &checked_trees::CheckedOperatorFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Vec<StateArgumentFacts> {
    let states = program.machine_states(machine);
    if states.len() <= 1 {
        return Vec::new();
    }
    let Some((entry, non_entry_states)) = states.split_first() else {
        return Vec::new();
    };
    let has_internal_edge: Vec<bool> = states
        .iter()
        .map(|state| state_has_internal_edge(program, non_entry_states, state))
        .collect();
    if !has_internal_edge[0] {
        return Vec::new();
    }
    let mut collected: Vec<StateArgumentFacts> = Vec::new();
    // Source-owned outputs preserve statement encounter order. A dirty source
    // replaces its complete contribution, including edges that lost facts.
    let mut contributions = vec![Vec::new(); states.len()];
    let mut dirty = vec![false; states.len()];
    dirty[0] = true;
    for _ in 0..MAX_PROPAGATION_PASSES {
        for (state_index, (state, calls)) in states.iter().zip(calls).enumerate() {
            if !dirty[state_index] {
                continue;
            }
            let outgoing = &mut contributions[state_index];
            outgoing.clear();
            if !has_internal_edge[state_index]
                || (state_index != 0 && !collected.iter().any(|facts| facts.state == state.symbol))
            {
                continue;
            }
            let context = ExpressionTransfer {
                program,
                machine,
                state,
                call_frames,
            };
            let mut facts = RangeFacts::new(machine.symbol, field_lengths, bound_lookup);
            facts.mutation_summaries = std::borrow::Cow::Borrowed(mutation_summaries);
            facts.checked_calls = Some(calls);
            facts.checked_operators = Some(operators);
            for parameter in program.state_parameters(state) {
                facts.define_local(
                    parameter.symbol,
                    parameter.name.to_string(),
                    super::arrays::fixed_array_type_length(program, parameter.type_reference),
                    None,
                );
            }
            seed_state_requires(program, &mut facts, machine, state);
            if state_index != 0 {
                seed_state_argument_facts(&mut facts, state, &collected);
            }
            for (statement_index, statement) in program
                .statement_table
                .admitted_execution_statements(state.statement_nodes)
            {
                facts.statement_index = statement_index;
                collect_state_argument_facts_from_statement(
                    &context, &mut facts, statement, outgoing,
                );
            }
            // An internal edge never narrows the externally callable head.
            outgoing.retain(|incoming| incoming.state != entry.symbol);
        }
        let mut next = Vec::new();
        for outgoing in &contributions {
            for incoming in outgoing {
                merge_contribution(&mut next, incoming);
            }
        }
        if next == collected {
            return next;
        }
        for (state_index, state) in states.iter().enumerate() {
            dirty[state_index] = state_index != 0
                && has_internal_edge[state_index]
                && next.iter().find(|facts| facts.state == state.symbol)
                    != collected.iter().find(|facts| facts.state == state.symbol);
        }
        collected = next;
    }
    // Keep synchronous rounds and the original budget: an unfinished fixed
    // point cannot publish provisional facts, even if only one state is dirty.
    Vec::new()
}

fn state_has_internal_edge(
    program: &dyn ::validation::CheckingInput,
    non_entry_states: &[State],
    state: &State,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    let is_internal =
        |symbol: SymbolHandle| non_entry_states.iter().any(|state| state.symbol == symbol);
    for (_, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        let has_edge = match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
            StatementNode::Assignment(assignment) => {
                expression_has_internal_call(program, non_entry_states, assignment.target, 0)
                    || expression_has_internal_call(program, non_entry_states, assignment.value, 0)
            }
            StatementNode::Call(call) => {
                is_internal(call.target_symbol)
                    || program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|arg| expression_has_internal_call(program, non_entry_states, *arg, 0))
            }
            StatementNode::Expression(expression) => {
                expression_has_internal_call(program, non_entry_states, *expression, 0)
            }
            StatementNode::LocalData(local) => {
                expression_has_internal_call(program, non_entry_states, local.initial_value, 0)
            }
            StatementNode::Transition(transition) => {
                (if let TransitionGuardNode::When(guard) = transition.guard {
                    expression_has_internal_call(program, non_entry_states, guard, 0)
                } else {
                    false
                }) || [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                    .any(
                        |target| match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Named {
                                path, arguments, ..
                            } => {
                                is_internal(path.symbol)
                                    || program
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .iter()
                                        .any(|arg| {
                                            expression_has_internal_call(
                                                program,
                                                non_entry_states,
                                                *arg,
                                                0,
                                            )
                                        })
                            }
                            TransitionTargetNode::Value(value) => {
                                expression_has_internal_call(program, non_entry_states, *value, 0)
                            }
                            TransitionTargetNode::Terminal => false,
                        },
                    )
            }
        };
        if has_edge {
            return true;
        }
    }
    false
}

fn expression_has_internal_call(
    program: &dyn ::validation::CheckingInput,
    non_entry_states: &[State],
    expression: typed_trees::expression::ExpressionHandle,
    depth: usize,
) -> bool {
    use typed_trees::expression::ExpressionNode;
    let Some(node) = program
        .expression_table
        .expression_at_depth(expression, depth)
    else {
        return false;
    };
    let next = depth + 1;
    match node {
        ExpressionNode::SavedSubject(_) => false,
        ExpressionNode::Call(call) => {
            non_entry_states
                .iter()
                .any(|state| state.symbol == call.target_symbol)
                || expression_has_internal_call(program, non_entry_states, call.receiver, next)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|arg| expression_has_internal_call(program, non_entry_states, *arg, next))
        }
        ExpressionNode::Match(dispatch) => {
            expression_has_internal_call(program, non_entry_states, dispatch.subject, next)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        program
                            .expression_table
                            .match_pattern_operands(arm.pattern)
                            .any(|operand| {
                                expression_has_internal_call(
                                    program,
                                    non_entry_states,
                                    operand,
                                    next,
                                )
                            })
                            || expression_has_internal_call(
                                program,
                                non_entry_states,
                                arm.value,
                                next,
                            )
                    })
        }
        ExpressionNode::Binary(binary) => {
            expression_has_internal_call(program, non_entry_states, binary.left, next)
                || expression_has_internal_call(program, non_entry_states, binary.right, next)
        }
        ExpressionNode::Cast(cast) => {
            expression_has_internal_call(program, non_entry_states, cast.value, next)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_has_internal_call(program, non_entry_states, indexed.collection, next)
                || expression_has_internal_call(program, non_entry_states, indexed.index, next)
        }
        ExpressionNode::Atomic(atomic) => {
            if let Some(carrier) = validation::atomic_once_carrier(program, expression) {
                expression_has_internal_call(program, non_entry_states, carrier.place, next)
                    || carrier.operands.into_iter().any(|operand| {
                        expression_has_internal_call(program, non_entry_states, operand, next)
                    })
            } else {
                expression_has_internal_call(program, non_entry_states, atomic.value, next)
            }
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_has_internal_call(program, non_entry_states, *value, next)),
        ExpressionNode::Member(member) => {
            expression_has_internal_call(program, non_entry_states, member.receiver, next)
        }
        ExpressionNode::Borrow(borrow) => {
            expression_has_internal_call(program, non_entry_states, borrow.target, next)
        }
        ExpressionNode::Unary(unary) => {
            expression_has_internal_call(program, non_entry_states, unary.operand, next)
        }
        ExpressionNode::Range(range) => {
            expression_has_internal_call(program, non_entry_states, range.start, next)
                || expression_has_internal_call(program, non_entry_states, range.end, next)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                expression_has_internal_call(program, non_entry_states, field.value, next)
            }),
        typed_trees::literal_expression_pattern!() => false,
    }
}

fn merge_contribution(collected: &mut Vec<StateArgumentFacts>, incoming: &StateArgumentFacts) {
    let Some(existing) = collected
        .iter_mut()
        .find(|facts| facts.state == incoming.state)
    else {
        collected.push(incoming.clone());
        return;
    };
    for (existing, incoming) in existing.parameters.iter_mut().zip(&incoming.parameters) {
        if incoming.length != MergedFact::Unseen {
            existing.length.merge(incoming.length.get());
        }
        if incoming.integer != MergedFact::Unseen {
            existing.integer.merge(incoming.integer.get());
        }
        if incoming.minimum_length != MergedBound::Unseen {
            existing
                .minimum_length
                .merge_lower(incoming.minimum_length.get());
        }
        if incoming.upper_bound != MergedBound::Unseen {
            existing.upper_bound.merge(incoming.upper_bound.get());
        }
        if incoming.non_negative != MergedFact::Unseen {
            existing.non_negative.merge(incoming.non_negative.get());
        }
        existing
            .below_const_binders
            .merge(incoming.below_const_binders.get());
    }
    if let Some(incoming) = &incoming.index_proofs.proofs {
        if let Some(existing) = &mut existing.index_proofs.proofs {
            existing.retain(|proof| incoming.contains(proof));
        } else {
            existing.index_proofs.proofs = Some(incoming.clone());
        }
    }
    if let Some(incoming) = &incoming.receiver_lengths {
        if let Some(existing) = &mut existing.receiver_lengths {
            existing.retain(|length| incoming.iter().any(|incoming| length.same_extent(incoming)));
        } else {
            existing.receiver_lengths = Some(incoming.clone());
        }
    }
    merge_storage_rows(&mut existing.storage, &incoming.storage);
}

/// Meet by destination storage identity, not by the source expression or
/// spelling. Each retained property must hold on every incoming edge.
fn merge_storage_rows(
    existing: &mut Option<Vec<StorageFacts>>,
    incoming: &Option<Vec<StorageFacts>>,
) {
    let Some(incoming) = incoming else {
        return;
    };
    if let Some(existing) = existing {
        existing.retain_mut(|row| {
            if let Some(incoming) = incoming
                .iter()
                .find(|candidate| candidate.place == row.place)
            {
                row.upper_bound = row
                    .upper_bound
                    .zip(incoming.upper_bound)
                    .map(|(a, b)| a.max(b));
                row.minimum_length = row
                    .minimum_length
                    .zip(incoming.minimum_length)
                    .map(|(a, b)| a.min(b));
                row.non_negative &= incoming.non_negative;
                row.upper_bound.is_some() || row.minimum_length.is_some() || row.non_negative
            } else {
                false
            }
        });
    } else {
        *existing = Some(incoming.clone());
    }
}

pub(super) fn seed_state_argument_facts(
    facts: &mut RangeFacts<'_>,
    state: &State,
    collected: &[StateArgumentFacts],
) {
    let Some(state_facts) = collected.iter().find(|entry| entry.state == state.symbol) else {
        return;
    };

    facts.seed_receiver_lengths(
        state_facts.machine,
        state.symbol,
        state_facts.receiver_lengths.as_deref().unwrap_or(&[]),
    );

    for parameter in &state_facts.parameters {
        facts.define_local(
            parameter.symbol,
            parameter.name.clone(),
            parameter.length.get(),
            parameter.integer.get(),
        );
        if let Some(bound) = parameter.upper_bound.get() {
            facts.prove_index_upper_bound(parameter.name.clone(), bound);
        }
        if parameter.non_negative.get().is_some() {
            facts.prove_non_negative(parameter.name.clone());
        }
        for binder in parameter.below_const_binders.get() {
            facts.prove_below_const_binder(parameter.name.clone(), *binder);
        }
        if let Some(minimum) = parameter.minimum_length.get() {
            facts.prove_minimum_length(parameter.name.clone(), minimum);
        }
    }

    for row in state_facts.storage.as_deref().unwrap_or(&[]) {
        facts.seed_storage_dependency(
            state_facts.machine,
            state.symbol,
            row.expression,
            &row.label,
            &row.place,
        );
        if let Some(bound) = row.upper_bound {
            facts.prove_index_upper_bound(row.label.clone(), bound);
        }
        if row.non_negative {
            facts.prove_non_negative(row.label.clone());
        }
        if let Some(minimum) = row.minimum_length {
            facts.prove_minimum_length(row.label.clone(), minimum);
        }
    }

    for proof in state_facts.index_proofs.get() {
        let Some(collection) = proof.collection.label(&state_facts.parameters) else {
            continue;
        };
        let Some(index) = proof.index.label(&state_facts.parameters) else {
            continue;
        };
        facts.prove_index(collection.to_owned(), index.to_owned());
        facts.prove_range_bound(collection.to_owned(), index.to_owned());
    }
}
