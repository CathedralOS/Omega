//! Integer result facts across complete ordinary and terminal invocations.
//!
//! The arrival walker owns ordered scalar snapshots and effects. This owner
//! joins every complete return and bounds recursive inference with invocation-
//! local active frames; unknown inputs and recurrent calls never prove themselves.

use super::{ArrivalWalk, ExpressionObservation, ValueObservationSite, incoming_environments};
use crate::CallFrameResolver;
use crate::proof_contracts::arithmetic_domains::{Interval, Machine, State};
use std::cell::RefCell;
use symbols::SymbolHandle;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

/// One outer range request owns the active invocation graph. Ordinary and
/// terminal descendants share it; recurrence retains published guarantees
/// rather than using the recursive body as its own proof.
#[derive(Default)]
pub(crate) struct ReturnInference {
    active: RefCell<Vec<(SymbolHandle, SymbolHandle)>>,
}

impl ReturnInference {
    pub(crate) fn infer(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        entry: &State,
        target_primitive: Option<typed_trees::types::PrimitiveType>,
        actuals: &[ActualRange],
    ) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
        inferred_normal_return_interval_in(program, machine, entry, target_primitive, actuals, self)
    }
}

type ActualRange = (
    SymbolHandle,
    Interval,
    Option<checked_trees::IntegerRangeRecipe>,
);

pub(crate) fn inferred_normal_return_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    target_primitive: Option<typed_trees::types::PrimitiveType>,
    actuals: &[ActualRange],
) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
    ReturnInference::default().infer(program, machine, entry, target_primitive, actuals)
}

fn join_return(
    combined: &mut Option<Interval>,
    combined_recipe: &mut Option<checked_trees::IntegerRangeRecipe>,
    interval: Interval,
    recipe: Option<checked_trees::IntegerRangeRecipe>,
) {
    *combined_recipe = if combined.is_none() {
        recipe
    } else {
        combined_recipe
            .take()
            .zip(recipe)
            .map(|(left, right)| left.union(&right))
    };
    *combined = Some(combined.map_or(interval, |previous| previous.union(interval)));
}

fn inferred_normal_return_interval_in(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    target_primitive: Option<typed_trees::types::PrimitiveType>,
    actuals: &[ActualRange],
    inference: &ReturnInference,
) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
    // This judgment publishes integer intervals. Other result carriers have
    // their own evidence owners and cannot benefit from this body traversal.
    let primitive = target_primitive?;
    crate::proof_contracts::arithmetic_domains::integer_ranges::primitive_range(primitive)?;
    let memos = crate::frozen_program::frozen_program_memos(program).filter(|_| {
        // Deferred recipes carry more than concrete interval endpoints. Until
        // they have an exact memo identity, preserve their ordinary inference.
        actuals.iter().all(|(_, _, recipe)| recipe.is_none())
            && program
                .machine(machine.symbol)
                .is_some_and(|stored| std::ptr::eq(stored, machine))
            && program
                .state_in_machine(machine, entry.symbol)
                .is_some_and(|stored| std::ptr::eq(stored, entry))
    });
    let key = (
        machine.symbol,
        entry.symbol,
        primitive as u8,
        actuals
            .iter()
            .map(|(symbol, interval, _)| (*symbol, interval.low, interval.high))
            .collect::<Vec<_>>(),
    );
    if let Some(memos) = memos
        && let Ok(intervals) = memos.normal_return_intervals.lock()
        && let Some(interval) = intervals.get(&key)
    {
        return interval.clone();
    }
    let identity = (machine.symbol, entry.symbol);
    {
        let mut active = inference.active.borrow_mut();
        if active.contains(&identity) {
            return None;
        }
        active.push(identity);
    }
    let interval = discover_normal_return_interval(
        program,
        machine,
        entry,
        target_primitive,
        actuals,
        inference,
    );
    inference.active.borrow_mut().pop();
    if interval.is_none() && crate::proof_contracts::contract_entailment::struct_trace() {
        eprintln!(
            "STRUCT integer-return unknown owner={} entry={:?} primitive={target_primitive:?} supply={:?} all-exits={}",
            machine.name,
            entry.symbol,
            machine.supply_mode,
            crate::proof_contracts::contract_entailment::entailment_covers_all_exits(
                program, machine
            )
        );
        for state in program.machine_states(machine) {
            let last = program
                .statement_table
                .statements(state.statement_nodes)
                .last();
            eprintln!(
                "STRUCT integer-return state={:?} primitive={:?} last={last:?}",
                state.symbol,
                program.primitive_type_reference(state.return_type)
            );
            if let Some(StatementNode::Transition(transition)) = last {
                for target in transition.targets() {
                    eprintln!(
                        "STRUCT integer-return target={:?}",
                        program.statement_table.transition_target(target)
                    );
                }
            }
        }
    }

    if interval.is_some()
        && let Some(memos) = memos
        && let Ok(mut intervals) = memos.normal_return_intervals.lock()
    {
        intervals.insert(key, interval.clone());
    }
    interval
}

fn discover_normal_return_interval(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    target_primitive: Option<typed_trees::types::PrimitiveType>,
    actuals: &[ActualRange],
    inference: &ReturnInference,
) -> Option<(Interval, Option<checked_trees::IntegerRangeRecipe>)> {
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !crate::proof_contracts::contract_entailment::entailment_covers_all_exits(
            program, machine,
        )
    {
        return None;
    }
    let mut state_returns = Vec::new();
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        let order = program
            .statement_table
            .finite_execution_order(state.statement_nodes)
            .ok()??;
        let last = order.last().map(|index| &statements[*index]);
        if program.primitive_type_reference(state.return_type) != target_primitive {
            return None;
        }
        if !matches!(
            last,
            Some(StatementNode::Expression(_) | StatementNode::Transition(_))
        ) {
            return None;
        }
        let mut returns = Vec::new();
        if let Some(StatementNode::Expression(expression)) = last {
            returns.push(ExpressionObservation::bounds(*expression));
        }
        for statement in order.iter().map(|index| &statements[*index]) {
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
                    TransitionTargetNode::Value(expression) => {
                        if !returns
                            .iter()
                            .any(|observed| observed.expression() == Some(*expression))
                        {
                            returns.push(ExpressionObservation::bounds(*expression));
                        }
                    }
                    TransitionTargetNode::Named {
                        path,
                        static_machine_parameter,
                        evidence_arguments,
                        ..
                    } => {
                        if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                            return None;
                        }
                        if typed_trees::state::state_by_symbol(
                            program.machine_states(machine),
                            path.symbol,
                        )
                        .is_none()
                        {
                            returns.push(ExpressionObservation::invocation(target));
                        }
                    }
                    TransitionTargetNode::Terminal => return None,
                }
            }
        }
        if !returns.is_empty() {
            state_returns.push((state, returns));
        }
    }
    if state_returns.is_empty() {
        return None;
    }
    let frames = CallFrameResolver::new(program);
    let incoming = incoming_environments(
        program,
        machine,
        frames.as_ref(),
        Some((entry, actuals)),
        Some(inference),
    );
    let mut combined: Option<Interval> = None;
    let mut combined_recipe: Option<checked_trees::IntegerRangeRecipe> = None;
    for (state, mut returns) in state_returns {
        returns.sort_unstable_by_key(ExpressionObservation::key);
        let mut environment = incoming.iter().find_map(|(symbol, environment)| {
            (*symbol == state.symbol).then(|| environment.clone())
        })?;
        let mut walk = ArrivalWalk {
            program,
            machine,
            frames: frames.as_ref(),
            arrivals: Vec::new(),
            observations: returns,
            completed_children: Default::default(),
            return_ranges: Some(inference),
            collect_arrivals: false,
        };
        walk.state(state, &mut environment);
        for observed in walk.observations {
            match observed.site {
                ValueObservationSite::Expression(_) => {
                    join_return(
                        &mut combined,
                        &mut combined_recipe,
                        observed.interval?,
                        observed.recipe,
                    );
                }
                ValueObservationSite::Invocation { target, arrivals } => {
                    if arrivals.is_empty() {
                        return None;
                    }
                    let TransitionTargetNode::Named { path, .. } =
                        program.statement_table.transition_target(target)
                    else {
                        return None;
                    };
                    let (callee, entry) =
                        crate::machine_calls::calls::machine_state_by_symbol(program, path.symbol)?;
                    let parameters = program.state_parameters(entry);
                    for arrival in arrivals {
                        if arrival.len() != parameters.len() {
                            return None;
                        }
                        let actuals = parameters
                            .iter()
                            .zip(arrival)
                            .filter_map(|(parameter, (interval, recipe))| {
                                (!parameter.is_const
                                    && !parameter.is_mutable
                                    && program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_some())
                                .then_some((parameter.symbol, interval, recipe))
                            })
                            .collect::<Vec<_>>();
                        let (interval, recipe) = inferred_normal_return_interval_in(
                            program,
                            callee,
                            entry,
                            target_primitive,
                            &actuals,
                            inference,
                        )?;
                        join_return(&mut combined, &mut combined_recipe, interval, recipe);
                    }
                }
            }
        }
    }
    let combined = combined?;
    (combined_recipe.is_some() || (combined.low.is_some() && combined.high.is_some()))
        .then_some((combined, combined_recipe))
}
