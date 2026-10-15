//! State arrivals use the source evaluation environment and the exact jump
//! argument telescope. Each round starts from an overapproximation, including
//! every backedge; stopping before convergence only loses precision.
use super::super::{
    TypeReferenceHandle, TypeReferenceNode, enforced_declared_range, record_assignment,
    record_unsigned_literal_assignment,
};
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    SignatureContractKind, State, ValueEnvironment, declared_place_type_raw,
    incoming_guard_environment, literal_i64, narrow_environment_by_condition, ordered_values,
    place_path,
};
use crate::CallFrameResolver;
use crate::proof_contracts::arithmetic_domains::CopiedIntegerValue;
use crate::proof_contracts::arithmetic_domains::expression_analysis::{CheckedAnalysis, analyze};
use symbols::SymbolHandle;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

mod collection_bounds;
mod transition_arguments;
pub use collection_bounds::CollectionBoundsObservations;

struct EvaluatedValue {
    interval: Interval,
    copied: CopiedIntegerValue,
}

/// Bound one exact terminal expression using every arrival and the effects of
/// its state prefix, or a named target-arm argument under its co-located guard.
/// The latter uses declared bounds only, without inherited arrival facts.
/// Authored requirements are assumptions of body checking;
/// call/transition validation must independently discharge those requirements.
pub fn arrival_integer_expression_bounds(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if let Some(StatementNode::Transition(transition)) = statements.get(statement_index) {
        return transition_arguments::integer_bounds(
            program, machine, state, transition, expression,
        );
    }
    if !matches!(statements.get(statement_index), Some(StatementNode::Expression(value)) if *value == expression)
    {
        return None;
    }
    occurrence_integer_expression_bounds(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        expression,
    )
}

/// Bound the value observed at one exact expression occurrence. Entry and
/// predecessor facts survive only the effects before that occurrence; later
/// operands or statements cannot retarget an already evaluated value.
/// This is body checking, not a way to discharge an incoming requirement from
/// the requirement itself. Transition-argument admission keeps its separate
/// declaration-only premise discipline above.
pub fn occurrence_integer_expression_bounds(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let interval = expression_observation(
        program,
        machine_symbol,
        state_symbol,
        Some(statement_index),
        expression,
    )?
    .interval?;
    let (low, high) = (interval.low?, interval.high?);
    (low <= high).then_some((low, high))
}

/// Judge a closed bound at one exact value occurrence. Independent operand
/// intervals can lose correlations such as `amount <= limit - used`; the live
/// ordered-value judgment retains those relations across the same effects and
/// all-arrival joins as ordinary arithmetic checking.
pub fn occurrence_integer_expression_fits_range(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    minimum: &numerics::bignum::BigInt,
    maximum: &numerics::bignum::BigInt,
) -> bool {
    let Some(observation) = expression_observation(
        program,
        machine_symbol,
        state_symbol,
        Some(statement_index),
        expression,
    ) else {
        return false;
    };
    let lower_proven = observation.interval.is_some_and(|interval| {
        interval
            .low
            .is_some_and(|low| numerics::bignum::BigInt::from_i64(low) >= *minimum)
    });
    let upper_proven = observation.interval.is_some_and(|interval| {
        interval
            .high
            .is_some_and(|high| numerics::bignum::BigInt::from_i64(high) <= *maximum)
    });
    if lower_proven && upper_proven {
        return true;
    }
    let Some(environment) = observation.environment else {
        return false;
    };
    let Some(machine) = program.machine(machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, state_symbol) else {
        return false;
    };
    super::super::live_integer_constraints::proves_range(
        program,
        machine,
        state,
        &environment,
        expression,
        (!lower_proven).then_some(minimum),
        (!upper_proven).then_some(maximum),
    )
}

/// Facts at an exact source observation, including non-interval premises such
/// as a nonzero divisor. An unbounded result is not failed arithmetic: callers
/// validate under this environment rather than requiring finite endpoints.
pub(crate) fn occurrence_value_environment(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<ValueEnvironment> {
    occurrence_value_environments(program, machine_symbol, state_symbol, vec![expression])
        .get(expression)
        .cloned()
}

/// Caller-owned snapshots for an exact requested set of whole-state queries.
/// Requests are deduplicated, but the walk still joins every evaluated visit
/// of a shared expression handle. An unvisited request has no environment.
#[derive(Default)]
pub(crate) struct OccurrenceValueEnvironments {
    observations: Vec<ExpressionObservation>,
}

impl OccurrenceValueEnvironments {
    pub(crate) fn get(&self, expression: ExpressionHandle) -> Option<&ValueEnvironment> {
        let position = self
            .observations
            .binary_search_by_key(
                &(expression.arena_index(), expression.generation()),
                |observation| {
                    (
                        observation.expression.arena_index(),
                        observation.expression.generation(),
                    )
                },
            )
            .ok()?;
        self.observations[position].environment.as_ref()
    }
}

/// Walk once for all requested observations, not once per construction field.
/// This has the same unscoped occurrence meaning as a single query: prefix
/// effects and every visit participate, even after a request was first seen.
pub(crate) fn occurrence_value_environments(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    mut expressions: Vec<ExpressionHandle>,
) -> OccurrenceValueEnvironments {
    expressions.retain(|expression| expression.is_valid());
    expressions
        .sort_unstable_by_key(|expression| (expression.arena_index(), expression.generation()));
    expressions.dedup();
    if expressions.is_empty() {
        return OccurrenceValueEnvironments::default();
    }
    let Some((machine, state)) = program.machine_state(machine_symbol, state_symbol) else {
        return OccurrenceValueEnvironments::default();
    };
    let frames = CallFrameResolver::new(program);
    let mut environment = incoming_guard_environment(program, machine, state, frames.as_ref());
    let mut walk = ArrivalWalk {
        program,
        machine,
        frames: frames.as_ref(),
        arrivals: Vec::new(),
        observations: expressions
            .into_iter()
            .map(ExpressionObservation::environment)
            .collect(),
        infer_call_bodies: true,
    };
    walk.statements(
        state,
        program.statement_table.statements(state.statement_nodes),
        &mut environment,
    );
    OccurrenceValueEnvironments {
        observations: walk.observations,
    }
}

fn expression_observation(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: Option<usize>,
    expression: ExpressionHandle,
) -> Option<ExpressionObservation> {
    let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let (prefix, observed_statements) = match statement_index {
        Some(index) => (
            statements.get(..index)?,
            std::slice::from_ref(statements.get(index)?),
        ),
        None => (&statements[..0], statements),
    };
    let frames = CallFrameResolver::new(program);
    let mut environment = incoming_guard_environment(program, machine, state, frames.as_ref());
    let mut walk = ArrivalWalk {
        program,
        machine,
        frames: frames.as_ref(),
        arrivals: Vec::new(),
        observations: Vec::new(),
        infer_call_bodies: true,
    };
    if !walk.statements(state, prefix, &mut environment) {
        return None;
    }
    walk.observations.push(ExpressionObservation {
        include_bounds: true,
        recipe: None,
        expression,
        interval: None,
        environment: None,
    });
    walk.statements(state, observed_statements, &mut environment);
    walk.observations.pop()
}

pub(super) fn seed_state_requirements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    frames: Option<&CallFrameResolver>,
    environment: &mut ValueEnvironment,
) {
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let machine_written = (!is_entry)
        .then(|| machine_written_paths(program, machine, frames))
        .flatten();
    RequirementSeeds::new(
        program,
        machine,
        state,
        frames,
        is_entry,
        machine_written.as_deref(),
    )
    .apply(program, machine, state, environment);
}

/// Every place the machine's complete state write frames can write, or
/// `None` when a frame is opaque. Only a machine-level `requires` condition
/// reads it, so a machine without one never infers its frames here.
fn machine_written_paths(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    frames: Option<&CallFrameResolver>,
) -> Option<Vec<String>> {
    let has_machine_requires = program
        .machine_contracts(machine)
        .iter()
        .any(|contract| contract.kind == SignatureContractKind::Requires);
    if !has_machine_requires {
        return None;
    }
    frames.and_then(|frames| {
        frames
            .inferred_machine_state_write_frames(machine)
            .into_iter()
            .map(|frame| frame.into_complete_paths())
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.concat())
    })
}

/// The `requires` conditions that seed one state's arrival, and the
/// environment they establish on their own. Neither depends on the arriving
/// environment, so a fixed-point walk selects them once per state and only
/// re-narrows each round's arrival by them.
pub(super) struct RequirementSeeds {
    conditions: Vec<ExpressionHandle>,
    required: ValueEnvironment,
}

impl RequirementSeeds {
    pub(super) fn new(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        frames: Option<&CallFrameResolver>,
        is_entry: bool,
        machine_written: Option<&[String]>,
    ) -> Self {
        let mut conditions = Vec::new();
        // A machine-level `requires` fact is a precondition on entry. It still
        // holds at a later state's entry exactly when every place the recorded
        // facts name survives the whole machine's write frame; otherwise a
        // preheader or loop-body write could leave the contract stale before
        // this arrival (the same machine-preservation law the loop-invariant
        // index discharge applies to authored bound chains).
        for condition in program.contract_expressions(
            program.machine_contracts(machine).iter(),
            SignatureContractKind::Requires,
        ) {
            if condition_belongs_to_state(program, machine, state, condition, frames)
                && (is_entry
                    || machine_requires_survives(
                        program,
                        machine,
                        state,
                        machine_written,
                        condition,
                    ))
            {
                conditions.push(condition);
            }
        }
        for condition in program.contract_expressions(
            program.state_contracts(state).iter(),
            SignatureContractKind::Requires,
        ) {
            if condition_belongs_to_state(program, machine, state, condition, frames) {
                conditions.push(condition);
            }
        }
        let mut required = ValueEnvironment::new();
        for condition in &conditions {
            narrow_environment_by_condition(
                program,
                machine,
                Some(state),
                &mut required,
                *condition,
                true,
            );
        }
        Self {
            conditions,
            required,
        }
    }

    pub(super) fn apply(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        environment: &mut ValueEnvironment,
    ) {
        for condition in &self.conditions {
            narrow_environment_by_condition(
                program,
                machine,
                Some(state),
                environment,
                *condition,
                true,
            );
        }
        if environment.intervals.values().any(|interval| {
            interval
                .low
                .zip(interval.high)
                .is_some_and(|(low, high)| low > high)
        }) {
            // An arrival violating requires is rejected by the call-contract
            // checker. Check this body's return under its authored
            // assumptions, without using an empty interval as evidence for a
            // produced value.
            *environment = self.required.clone();
        }
    }
}

/// A machine-level `requires` condition carries into a non-entry state only
/// when the machine can never overwrite a place its recorded facts mention.
/// Re-recording the condition and applying the ordinary write-invalidation
/// law keeps the stability test on exactly the operands the environment
/// would use, `.len` operands included. An opaque write frame or any dropped
/// fact fails closed: the fact stays entry-only, as before.
fn machine_requires_survives(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    machine_written: Option<&[String]>,
    condition: ExpressionHandle,
) -> bool {
    let Some(written) = machine_written else {
        return false;
    };
    let mut seeded = ValueEnvironment::new();
    narrow_environment_by_condition(program, machine, Some(state), &mut seeded, condition, true);
    let mut retained = seeded.clone();
    retained.invalidate_written_paths(written);
    retained == seeded
}

fn condition_belongs_to_state(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&CallFrameResolver>,
) -> bool {
    if literal_i64(program, expression).is_some() {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            path.symbol.is_valid()
                && path.head_symbol.is_valid()
                && ((program
                    .state_parameter_by_symbol(state, path.head_symbol)
                    .is_some())
                    || (path.symbol == path.head_symbol
                        && program
                            .machine_type_parameters(machine)
                            .iter()
                            .any(|parameter| {
                                parameter.symbol == path.symbol
                                    && matches!(
                                        parameter.kind,
                                        typed_trees::data::TypeParameterKind::Value { .. }
                                            | typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                            })))
        }
        ExpressionNode::Member(member) => {
            // The builtin `.len` selector carries no declared field symbol;
            // admit it through the same structural receiver check
            // `ordered_values::build_operand` uses for its CollectionLength
            // operand.
            (member.member_symbol.is_valid()
                || crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .is_some())
                && condition_belongs_to_state(program, machine, state, member.receiver, frames)
        }
        ExpressionNode::Indexed(indexed) => {
            // A requirement belongs to this arrival only when both the
            // collection and selector belong to its exact parameter scope.
            // Its live identity and write lifetime are owned by ordered_values.
            crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
                && condition_belongs_to_state(program, machine, state, indexed.collection, frames)
                && condition_belongs_to_state(program, machine, state, indexed.index, frames)
        }
        ExpressionNode::Binary(binary) => {
            condition_belongs_to_state(program, machine, state, binary.left, frames)
                && condition_belongs_to_state(program, machine, state, binary.right, frames)
        }
        // A carrier conversion does not change the operand's lexical owner.
        // Its meaning and formation remain obligations of the bound reader;
        // scope admission alone grants no converted value or qualification.
        ExpressionNode::Cast(cast) => {
            condition_belongs_to_state(program, machine, state, cast.value, frames)
        }
        ExpressionNode::Unary(unary) => {
            condition_belongs_to_state(program, machine, state, unary.operand, frames)
        }
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_) => true,
        ExpressionNode::Call(_) => {
            // The proof-only projection has no executable operand, but its
            // lexical scope is exactly its source's scope. Rejecting it here
            // discarded unrelated conjuncts of a state invariant before the
            // narrowing owner could retain their useful ordinary bounds.
            if let Some((_, argument)) = crate::integer_embedding_argument(program, expression) {
                condition_belongs_to_state(program, machine, state, argument, frames)
            } else {
                ordered_values::operand(program, machine, state, expression, frames).is_some()
            }
        }
        _ => false,
    }
}

/// A captured invocation supplies immutable entry values. This uses the same
/// arrival joins and effect transfer as body checking; descendant calls retain
/// declared guarantees so a recursive body cannot justify its own bound.
pub(super) fn incoming_environments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    frames: Option<&CallFrameResolver>,
    invocation: Option<(
        &State,
        &[(
            SymbolHandle,
            Interval,
            Option<checked_trees::IntegerRangeRecipe>,
        )],
    )>,
) -> Vec<(SymbolHandle, ValueEnvironment)> {
    let states = program.machine_states(machine);
    let mut current = states
        .iter()
        .map(|state| (state.symbol, ValueEnvironment::new()))
        .collect::<Vec<_>>();
    let machine_written = (states.len() > 1)
        .then(|| machine_written_paths(program, machine, frames))
        .flatten();
    let seeds = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            RequirementSeeds::new(
                program,
                machine,
                state,
                frames,
                index == 0,
                machine_written.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    // A source body starts at its ordinary entry; a selected state call
    // starts at that exact declaration. The external arrival is immutable in
    // every round, and backedges still join with it rather than overwrite it.
    let external_index = invocation
        .and_then(|(entry, _)| typed_trees::state::state_position(states, entry.symbol))
        .unwrap_or(0);
    let external = states
        .get(external_index)
        .zip(seeds.get(external_index))
        .map(|(entry, entry_seeds)| {
            let mut external = ValueEnvironment::new();
            entry_seeds.apply(program, machine, entry, &mut external);
            if let Some((_, actuals)) = invocation {
                for parameter in program.state_parameters(entry) {
                    if parameter.is_mutable || parameter.is_const {
                        continue;
                    }
                    if let Some(carrier) = program
                        .primitive_type_reference(parameter.type_reference)
                        .and_then(super::super::primitive_range)
                        && let Some((_, actual, recipe)) = actuals
                            .iter()
                            .find(|(symbol, _, _)| *symbol == parameter.symbol)
                    {
                        external.narrow(
                            parameter.name.as_str().to_owned(),
                            carrier.intersect(*actual),
                        );
                        if let Some(recipe) = recipe {
                            external.integer_ranges.push(
                                super::super::value_environment::StoredIntegerRange {
                                    place: crate::value_custody::places::CanonicalPlace {
                                        root: facts::PlaceRoot::Symbol(parameter.symbol),
                                        segments: Vec::new(),
                                    },
                                    path: parameter.name.as_str().to_owned(),
                                    recipe: recipe.clone(),
                                },
                            );
                        }
                    }
                }
            }
            external
        });
    // One round per state propagates acyclic chains. Cycles also contribute in
    // every round, starting with their full declared parameter domains. There
    // is no assumption that a seed guard is an inductive loop invariant.
    // A state's walk reads only its own incoming environment, so its arrivals
    // are kept until that environment changes, and every round rejoins all
    // kept arrivals in state order.
    let mut arrivals = vec![Vec::new(); states.len()];
    let mut stale = vec![true; states.len()];
    for _ in 0..=states.len() {
        for (index, ((state, (_, environment)), state_seeds)) in
            states.iter().zip(&current).zip(&seeds).enumerate()
        {
            if !stale[index] {
                continue;
            }
            let mut walk = ArrivalWalk {
                program,
                machine,
                frames,
                arrivals: Vec::new(),
                observations: Vec::new(),
                infer_call_bodies: invocation.is_none(),
            };
            let mut environment = environment.clone();
            state_seeds.apply(program, machine, state, &mut environment);
            walk.statements(
                state,
                program.statement_table.statements(state.statement_nodes),
                &mut environment,
            );
            arrivals[index] = walk.arrivals;
        }
        let mut joined: Vec<Option<ValueEnvironment>> = vec![None; states.len()];
        if let Some(external) = &external {
            joined[external_index] = Some(external.clone());
        }
        for (target, environment) in arrivals.iter().flatten() {
            joined[*target] = Some(match joined[*target].take() {
                Some(previous) => previous.join(environment),
                None => environment.clone(),
            });
        }
        let next = states
            .iter()
            .zip(joined)
            .map(|(state, environment)| (state.symbol, environment.unwrap_or_default()))
            .collect::<Vec<_>>();
        if next == current {
            return next;
        }
        for ((stale, (_, next)), (_, current)) in stale.iter_mut().zip(&next).zip(&current) {
            *stale = next != current;
        }
        current = next;
    }
    current
}

/// Infer copied normal-return values with the ordinary state-arrival owner.
/// Every return in the complete graph contributes, including guarded value
/// arms and states reached through named transfers. Captures, writes and
/// argument order use the same walk as source observations. Crash transitions
/// contribute no normal value; incomplete ordinary exits decline inference.
pub(crate) fn inferred_normal_return_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    target_primitive: Option<typed_trees::types::PrimitiveType>,
    actuals: &[(
        SymbolHandle,
        Interval,
        Option<checked_trees::IntegerRangeRecipe>,
    )],
) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
    // This judgment publishes integer intervals. Other result carriers have
    // their own evidence owners and cannot benefit from this body traversal.
    target_primitive.and_then(super::super::primitive_range)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !crate::proof_contracts::contract_entailment::entailment_covers_all_exits(
            program, machine,
        )
    {
        return None;
    }
    let frames = CallFrameResolver::new(program);
    let incoming = incoming_environments(program, machine, frames.as_ref(), Some((entry, actuals)));
    let mut combined: Option<Interval> = None;
    let mut combined_recipe: Option<checked_trees::IntegerRangeRecipe> = None;
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        if program.primitive_type_reference(state.return_type) != target_primitive {
            return None;
        }
        if !matches!(
            statements.last(),
            Some(StatementNode::Expression(_) | StatementNode::Transition(_))
        ) {
            return None;
        }
        let mut returns = Vec::new();
        if let Some(StatementNode::Expression(expression)) = statements.last() {
            returns.push(*expression);
        }
        for statement in statements {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            if !matches!(
                transition.exit,
                typed_trees::statement::TransitionExit::Ordinary
            ) {
                continue;
            }
            for target in transition.targets() {
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Value(expression) => returns.push(*expression),
                    TransitionTargetNode::Named {
                        path,
                        static_machine_parameter,
                        evidence_arguments,
                        ..
                    } => {
                        if static_machine_parameter.is_valid()
                            || !evidence_arguments.is_empty()
                            || typed_trees::state::state_by_symbol(
                                program.machine_states(machine),
                                path.symbol,
                            )
                            .is_none()
                        {
                            return None;
                        }
                    }
                    TransitionTargetNode::Terminal => return None,
                }
            }
        }
        for expression in returns {
            let mut environment = incoming.iter().find_map(|(symbol, environment)| {
                (*symbol == state.symbol).then(|| environment.clone())
            })?;
            seed_state_requirements(program, machine, state, frames.as_ref(), &mut environment);
            let mut walk = ArrivalWalk {
                program,
                machine,
                frames: frames.as_ref(),
                arrivals: Vec::new(),
                observations: vec![ExpressionObservation {
                    include_bounds: true,
                    recipe: None,
                    expression,
                    interval: None,
                    environment: None,
                }],
                infer_call_bodies: false,
            };
            walk.statements(state, statements, &mut environment);
            let observed = walk.observations.pop()?;
            let interval = observed.interval?;
            combined_recipe = if combined.is_none() {
                observed.recipe
            } else {
                combined_recipe
                    .zip(observed.recipe)
                    .map(|(left, right)| left.union(&right))
            };
            combined = Some(combined.map_or(interval, |previous| previous.union(interval)));
        }
    }
    let combined = combined?;
    (combined_recipe.is_some() || (combined.low.is_some() && combined.high.is_some()))
        .then_some((combined, combined_recipe))
}

struct ExpressionObservation {
    include_bounds: bool,
    recipe: Option<checked_trees::IntegerRangeRecipe>,
    expression: ExpressionHandle,
    interval: Option<Interval>,
    environment: Option<ValueEnvironment>,
}

impl ExpressionObservation {
    fn environment(expression: ExpressionHandle) -> Self {
        Self {
            include_bounds: false,
            recipe: None,
            expression,
            interval: None,
            environment: None,
        }
    }
}

struct ArrivalWalk<'program, 'frames> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    frames: Option<&'frames CallFrameResolver<'program>>,
    /// Each arrival this walk delivers, by target state position, in order.
    arrivals: Vec<(usize, ValueEnvironment)>,
    observations: Vec<ExpressionObservation>,
    infer_call_bodies: bool,
}

impl ArrivalWalk<'_, '_> {
    fn join(&mut self, symbol: SymbolHandle, environment: ValueEnvironment) {
        if !symbol.is_valid() {
            return;
        }
        let Some(index) =
            typed_trees::state::state_position(self.program.machine_states(self.machine), symbol)
        else {
            return;
        };
        self.arrivals.push((index, environment));
    }

    fn evaluated_value(
        &self,
        source: &State,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
    ) -> EvaluatedValue {
        match self.checked_value(source, expression, environment) {
            Some(analysis) => EvaluatedValue {
                interval: analysis.value.interval,
                copied: CopiedIntegerValue::capture(
                    self.program,
                    expression,
                    environment,
                    &analysis.decisions.expression_values,
                ),
            },
            None => EvaluatedValue {
                interval: Interval::UNBOUNDED,
                copied: CopiedIntegerValue::default(),
            },
        }
    }

    fn checked_value(
        &self,
        source: &State,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
    ) -> Option<CheckedAnalysis> {
        let mut diagnostics = Vec::new();
        let analysis = analyze(
            self.program,
            self.machine,
            Some(source),
            expression,
            environment,
            None,
            ArithmeticDomain::Exact,
            "state arrival",
            self.infer_call_bodies,
            &mut diagnostics,
        );
        diagnostics.is_empty().then_some(analysis)
    }

    fn cross_writes(
        &self,
        source: &State,
        environment: &mut ValueEnvironment,
        written: Option<Vec<String>>,
    ) {
        if let Some(written) = written {
            environment.invalidate_written_paths(&written);
        } else {
            environment.invalidate_unknown_writes(self.program, Some(source));
        }
    }

    fn expression_effects(
        &self,
        source: &State,
        expression: ExpressionHandle,
        environment: &mut ValueEnvironment,
    ) {
        self.cross_writes(
            source,
            environment,
            self.frames
                .and_then(|frames| frames.expression_may_write_paths(self.machine, expression)),
        );
    }

    fn arguments(
        &mut self,
        source: &State,
        arguments: &[ExpressionHandle],
        environment: &mut ValueEnvironment,
    ) -> Vec<EvaluatedValue> {
        arguments
            .iter()
            .map(|argument| {
                self.expression(source, *argument, environment);
                self.evaluated_value(source, *argument, environment)
            })
            .collect()
    }

    fn arrive(
        &mut self,
        source: &State,
        symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        values: &[EvaluatedValue],
        environment: &ValueEnvironment,
    ) {
        let Some(target) = self
            .program
            .machine_states(self.machine)
            .iter()
            .find(|state| state.symbol == symbol && symbol.is_valid())
        else {
            return;
        };
        let parameters = self.program.state_parameters(target);
        if parameters.len() != arguments.len() {
            self.join(symbol, ValueEnvironment::new());
            return;
        }
        let mut bindings = Vec::new();
        let mut exact_bindings = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let Some(path) = self.bound_place(source, *argument) {
                bindings.push((path, parameter.name.as_str().to_owned()));
            }
            if (!environment.ordered_values.is_empty() || !environment.boolean_values.is_empty())
                && let Some(value) = ordered_values::operand(
                    self.program,
                    self.machine,
                    source,
                    *argument,
                    self.frames,
                )
            {
                exact_bindings.push((value, ordered_values::Operand::parameter(parameter)));
            }
        }
        let mut rebound = environment.rebind(&bindings);
        for value in &environment.boolean_values {
            rebound
                .boolean_values
                .extend(value.rebound(&exact_bindings));
        }
        rebound.ordered_values =
            ordered_values::rebind_relations(&environment.ordered_values, &exact_bindings);
        for (parameter, value) in parameters.iter().zip(values) {
            // A scalar value is a snapshot taken at its own argument position.
            // Reference parameters retain only facts surviving later arguments.
            if !is_reference(self.program, parameter.type_reference) {
                // The delivered argument is range-proved against the
                // parameter's declared range at the transition site, so the
                // recorded interval is intersected with that enforced range
                // before seeding -- the same clamp `record_assignment` applies
                // to local stores. Without it a cyclic delivery (`x + 1` under
                // `x <= self.max`) widens the joined arrival past the declared
                // bound on every fixpoint round, and later reads then trust a
                // value the parameter can never hold.
                let interval = match enforced_declared_range(self.program, parameter.type_reference)
                {
                    Some(declared) => value.interval.intersect(declared),
                    None => value.interval,
                };
                rebound.set(parameter.name.as_str().to_owned(), interval);
                value.copied.record(
                    &mut rebound,
                    Some(parameter.name.as_str().to_owned()),
                    Some(crate::value_custody::places::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(parameter.symbol),
                        segments: Vec::new(),
                    }),
                );
            }
        }
        self.join(symbol, rebound);
    }

    fn bound_place(&self, source: &State, expression: ExpressionHandle) -> Option<String> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol.is_valid() => {
                let root = self
                    .program
                    .state_parameter_by_symbol(source, path.head_symbol)
                    .is_some()
                    || self
                        .program
                        .state_local_declaration(source, path.head_symbol)
                        .is_some();
                root.then(|| place_path(self.program, expression)).flatten()
            }
            ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                self.bound_place(source, member.receiver)?;
                place_path(self.program, expression)
            }
            ExpressionNode::Borrow(borrow) => self.bound_place(source, borrow.target),
            _ => None,
        }
    }

    fn target(
        &mut self,
        source: &State,
        target: typed_trees::statement::TransitionTargetHandle,
        environment: &mut ValueEnvironment,
    ) {
        if !target.is_valid() {
            return;
        }
        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let arguments = self.program.statement_table.expression_handles(*arguments);
                let intervals = self.arguments(source, arguments, environment);
                self.arrive(source, path.symbol, arguments, &intervals, environment);
            }
            TransitionTargetNode::Value(expression) => {
                self.expression(source, *expression, environment)
            }
            TransitionTargetNode::Terminal => {}
        }
    }

    fn statements(
        &mut self,
        source: &State,
        statements: &[StatementNode],
        environment: &mut ValueEnvironment,
    ) -> bool {
        for statement in statements {
            match statement {
                StatementNode::Transition(transition) => {
                    let mut selected = environment.clone();
                    let mut fallback = environment.clone();
                    if let TransitionGuardNode::When(condition) = transition.guard {
                        selected = super::evaluated_condition_environment(
                            self.program,
                            self.machine,
                            Some(source),
                            condition,
                            true,
                            environment,
                        );
                        fallback = super::evaluated_condition_environment(
                            self.program,
                            self.machine,
                            Some(source),
                            condition,
                            false,
                            environment,
                        );
                        // Discover call occurrences separately from the exact
                        // selected-path facts supplied to state arrivals.
                        self.expression(source, condition, environment);
                    }
                    self.target(source, transition.target, &mut selected);
                    self.target(source, transition.continuation, &mut fallback);
                    if transition.continuation.is_valid()
                        || transition.guard == TransitionGuardNode::Always
                    {
                        return false;
                    }
                    *environment = fallback;
                }
                StatementNode::LocalData(local) => {
                    self.expression(source, local.initial_value, environment);
                    let copied_float = super::super::capture_float_assignment(
                        self.program,
                        self.machine,
                        Some(source),
                        environment,
                        self.program.primitive_type_reference(local.type_reference),
                        local.initial_value,
                    );
                    let mut value = self.evaluated_value(source, local.initial_value, environment);
                    let path = local.name.as_str().to_owned();
                    value.copied.capture_local_relations(
                        self.program,
                        self.machine,
                        Some(source),
                        local,
                        environment,
                    );
                    environment.invalidate_written_paths(std::slice::from_ref(&path));
                    environment.set(path.clone(), value.interval);
                    value.copied.record(
                        environment,
                        Some(path.clone()),
                        Some(crate::value_custody::places::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(local.symbol),
                            segments: Vec::new(),
                        }),
                    );
                    super::super::record_float_assignment(
                        environment,
                        Some(path.clone()),
                        copied_float,
                    );
                    super::boolean_values::record_boolean_local(
                        self.program,
                        self.machine,
                        Some(source),
                        environment,
                        local,
                    );
                    record_unsigned_literal_assignment(
                        self.program,
                        environment,
                        Some(path),
                        self.program.primitive_type_reference(local.type_reference),
                        local.initial_value,
                    );
                }
                StatementNode::Assignment(assignment) => {
                    self.expression(source, assignment.target, environment);
                    self.expression(source, assignment.value, environment);
                    let copied_float = super::super::capture_float_assignment(
                        self.program,
                        self.machine,
                        Some(source),
                        environment,
                        declared_place_type_raw(
                            self.program,
                            self.machine,
                            Some(source),
                            assignment.target,
                        )
                        .map(|reference| {
                            crate::value_custody::places::assignment_value_type(
                                self.program,
                                reference,
                            )
                        })
                        .and_then(|reference| self.program.primitive_type_reference(reference)),
                        assignment.value,
                    );
                    let mut value = self.evaluated_value(source, assignment.value, environment);
                    let written = self.frames.and_then(|frames| {
                        frames
                            .assignment_write_frame(self.machine, statement)
                            .into_complete_paths()
                    });
                    let complete = written.is_some();
                    if let Some(written) = written {
                        value.copied.capture_assignment_relations(
                            self.program,
                            self.machine,
                            source,
                            assignment,
                            &written,
                            environment,
                        );
                        environment.invalidate_assignment_paths(
                            self.program,
                            self.machine,
                            Some(source),
                            statement,
                            self.frames,
                            &written,
                        );
                    } else {
                        environment.invalidate_unknown_writes(self.program, Some(source));
                    }
                    // A completed store establishes the evaluated value, not
                    // merely the special case of a direct u64 literal. Retire
                    // aliases first, then let ordinary edge rebinding carry
                    // this snapshot to every successor that retains the place.
                    if complete {
                        let path = self.bound_place(source, assignment.target).or_else(|| {
                            crate::exact_self_field(self.program, self.machine, assignment.target)
                                .and_then(|_| place_path(self.program, assignment.target))
                        });
                        let reference = declared_place_type_raw(
                            self.program,
                            self.machine,
                            Some(source),
                            assignment.target,
                        )
                        .map(|reference| {
                            crate::value_custody::places::assignment_value_type(
                                self.program,
                                reference,
                            )
                        });
                        record_assignment(
                            environment,
                            path.clone(),
                            value.interval,
                            reference.and_then(|reference| {
                                enforced_declared_range(self.program, reference)
                            }),
                        );
                        value.copied.record(
                            environment,
                            path.clone(),
                            crate::value_custody::places::canonical_place_from_expression(
                                self.program,
                                assignment.target,
                            ),
                        );
                        super::super::record_float_assignment(
                            environment,
                            path.clone(),
                            copied_float,
                        );
                        let primitive = reference
                            .and_then(|reference| self.program.primitive_type_reference(reference));
                        super::boolean_values::record_boolean_assignment(
                            self.program,
                            self.machine,
                            Some(source),
                            environment,
                            assignment.target,
                            assignment.value,
                        );
                        record_unsigned_literal_assignment(
                            self.program,
                            environment,
                            path,
                            primitive,
                            assignment.value,
                        );
                    }
                }
                StatementNode::Call(call) => {
                    let operand_written = self.frames.and_then(|frames| {
                        frames.statement_value_may_write_paths(self.machine, statement)
                    });
                    let arguments = self
                        .program
                        .statement_table
                        .expression_handles(call.arguments);
                    let intervals = self.arguments(source, arguments, environment);
                    self.arrive(
                        source,
                        call.target_symbol,
                        arguments,
                        &intervals,
                        environment,
                    );
                    self.cross_writes(
                        source,
                        environment,
                        self.frames
                            .and_then(|frames| frames.may_write_paths(self.machine, call)),
                    );
                    if let Some(signature) = crate::machine_calls::calls::boundary_trait_signature(
                        self.program,
                        self.machine,
                        call,
                    ) {
                        super::seed_out_param_ensures(
                            self.program,
                            self.machine,
                            Some(source),
                            call,
                            signature,
                            environment,
                        );
                    } else {
                        // Formation and exact return observations consume the
                        // same completed citation. Scalar selectors denote
                        // their argument snapshots only while the complete
                        // statement frame preserves them, including siblings.
                        super::super::seed_cited_element_bounds(
                            self.program,
                            self.machine,
                            Some(source),
                            call,
                            operand_written.as_deref(),
                            environment,
                        );
                    }
                }
                StatementNode::Expression(expression) => {
                    self.expression(source, *expression, environment)
                }
                StatementNode::RootBinding(binding) => {
                    self.expression(source, binding.receiver, environment);
                    self.expression(source, binding.implementation_operand, environment);
                }
                StatementNode::AssemblyFact(_) => {}
            }
        }
        true
    }

    fn expression(
        &mut self,
        source: &State,
        expression: ExpressionHandle,
        environment: &mut ValueEnvironment,
    ) {
        if !expression.is_valid() {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                self.expression(source, dispatch.subject, environment);
                let mut remaining = environment.clone();
                let mut joined: Option<ValueEnvironment> = None;
                let mut boolean_values = [false; 2];
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        self.expression(source, pattern, &mut remaining);
                        if let Some(value) = self.program.expression_table.boolean_node(pattern) {
                            boolean_values[usize::from(*value)] = true;
                        }
                    }
                    let mut selected = remaining.clone();
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                        && let ExpressionNode::Boolean(positive) =
                            self.program.expression_table.expression(pattern)
                    {
                        narrow_environment_by_condition(
                            self.program,
                            self.machine,
                            Some(source),
                            &mut selected,
                            dispatch.subject,
                            *positive,
                        );
                        narrow_environment_by_condition(
                            self.program,
                            self.machine,
                            Some(source),
                            &mut remaining,
                            dispatch.subject,
                            !*positive,
                        );
                        self.expression_effects(source, dispatch.subject, &mut selected);
                        self.expression_effects(source, dispatch.subject, &mut remaining);
                    }
                    self.expression(source, arm.value, &mut selected);
                    joined = Some(
                        joined.map_or_else(|| selected.clone(), |joined| joined.join(&selected)),
                    );
                    if matches!(arm.pattern, typed_trees::expression::MatchPattern::Wildcard)
                        || boolean_values == [true, true]
                    {
                        break;
                    }
                }
                if let Some(joined) = joined {
                    *environment = joined;
                }
            }
            ExpressionNode::Call(call) => {
                self.expression(source, call.receiver, environment);
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                let intervals = self.arguments(source, arguments, environment);
                self.arrive(
                    source,
                    call.target_symbol,
                    arguments,
                    &intervals,
                    environment,
                );
                self.expression_effects(source, expression, environment);
            }
            ExpressionNode::Binary(binary) => {
                self.expression(source, binary.left, environment);
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let skipped = environment.clone();
                    self.expression(source, binary.right, environment);
                    *environment = skipped.join(environment);
                } else {
                    self.expression(source, binary.right, environment);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(source, unary.operand, environment),
            ExpressionNode::Member(member) => self.expression(source, member.receiver, environment),
            ExpressionNode::Indexed(indexed) => {
                self.expression(source, indexed.collection, environment);
                self.expression(source, indexed.index, environment);
            }
            ExpressionNode::Atomic(atomic) => self.expression(source, atomic.value, environment),
            ExpressionNode::Cast(cast) => self.expression(source, cast.value, environment),
            ExpressionNode::Borrow(borrow) => self.expression(source, borrow.target, environment),
            ExpressionNode::Range(range) => {
                self.expression(source, range.start, environment);
                self.expression(source, range.end, environment);
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.expression(source, *value, environment);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.expression(source, field.value, environment);
                }
            }
            typed_trees::literal_expression_pattern!() => {}
        }
        if let Ok(position) = self.observations.binary_search_by_key(
            &(expression.arena_index(), expression.generation()),
            |observation| {
                (
                    observation.expression.arena_index(),
                    observation.expression.generation(),
                )
            },
        ) {
            let mut observed = environment.clone();
            self.expression_effects(source, expression, &mut observed);
            // Environment queries need the observation's effects, not its
            // discarded result bounds. Value analysis used for local updates
            // and state arrivals remains in evaluated_value above.
            let include_bounds = self.observations[position].include_bounds;
            let analyzed = include_bounds
                .then(|| self.checked_value(source, expression, &observed))
                .flatten();
            let interval = analyzed
                .as_ref()
                .map_or(Interval::UNBOUNDED, |value| value.value.interval);
            // Concrete and deferred returns participate in the same union.
            // Dropping a literal arm's bounds here would erase a sibling's
            // layout observation when the machine returns through states.
            let recipe = analyzed.and_then(|value| {
                value
                    .value
                    .range_recipe(self.program, self.machine.symbol, expression)
            });
            let observation = &mut self.observations[position];
            if include_bounds {
                observation.recipe = if observation.interval.is_none() {
                    recipe
                } else {
                    observation
                        .recipe
                        .take()
                        .zip(recipe)
                        .map(|(left, right)| left.union(&right))
                };
                observation.interval = Some(
                    observation
                        .interval
                        .map_or(interval, |prior| prior.union(interval)),
                );
            }
            observation.environment = Some(match observation.environment.take() {
                Some(prior) => prior.join(&observed),
                None => observed,
            });
        }
    }
}

fn is_reference(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => is_reference(program, *base_type),
        _ => false,
    }
}
