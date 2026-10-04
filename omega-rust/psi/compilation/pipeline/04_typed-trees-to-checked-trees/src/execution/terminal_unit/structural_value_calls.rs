//! Structural expression consumers share ordinary call operation schedules.
//! A catalog describes each value's calls; only the value evaluator executes
//! them, at their exact DAG positions. Preceding statement results supply the
//! same source namespace as an ordinary construction or call argument.

use super::control::{LocalConstructionTrace, structural_operands};
use super::types::ShapeCollector;
use super::{CheckFacts, ScalarCalleePlans, TypedTrees};
use checked_trees::{CheckedStructuralValueRoot, CheckedUnitEffectOperationPlan};

pub(crate) fn retain(program: &TypedTrees, facts: &mut CheckFacts) {
    let mut shapes = ShapeCollector::new(program);
    let mut schedules = Vec::new();
    for (_, root) in facts.values.structural_values.roots.iter() {
        if schedules.iter().any(|(_, _, value, _)| *value == root.root) {
            continue;
        }
        if let Some(calls) = schedule(program, facts, &mut shapes, root) {
            schedules.push((root.machine, root.state, root.root, calls));
        }
    }
    let mut completed = Vec::new();
    for (machine, state, _, _) in &schedules {
        if completed.contains(&(*machine, *state)) {
            continue;
        }
        completed.push((*machine, *state));
        let Some(operations) = caller_operations(facts, *machine, *state) else {
            continue;
        };
        let mut updated = operations.clone();
        let state_schedules = schedules
            .iter()
            .filter(|(owner, source, _, _)| owner == machine && source == state);
        if consume_caller_results(
            &mut updated,
            state_schedules.map(|(_, _, _, calls)| calls.as_slice()),
        )
        .is_none()
        {
            // A schedule without coherent source custody is not executable.
            // Do not publish partial transfers or the calls that would use them.
            continue;
        }
        *operations = updated;
        for (_, _, value, calls) in schedules
            .iter()
            .filter(|(owner, source, _, _)| owner == machine && source == state)
        {
            facts
                .values
                .structural_values
                .retain_calls_for_root(*value, calls.clone());
        }
    }
    // A temporary's result type need not occur in its caller's signature.
    // Retain the ordinary call builder's type declarations under the same
    // namespace as all other structural operation results.
    for declaration in shapes.types.into_values() {
        if !facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|existing| existing.identity == declaration.identity)
        {
            facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(declaration);
        }
    }
    for domain in shapes.domains {
        if !facts
            .flow
            .terminal_unit_effects
            .structural_domains
            .iter()
            .any(|existing| existing.domain == domain.domain)
        {
            facts
                .flow
                .terminal_unit_effects
                .structural_domains
                .push(domain);
        }
    }
}

/// Catalog calls execute in scalar/value continuations, while the preceding
/// producer's disposal debt belongs to its enclosing checked operation body.
/// Rejoin that shared namespace at publication using ordinary transfer rules.
fn consume_caller_results<'a>(
    operations: &mut [CheckedUnitEffectOperationPlan],
    schedules: impl Iterator<Item = &'a [checked_trees::CheckedStructuralValueCall]>,
) -> Option<()> {
    let mut consumed = Vec::new();
    for calls in schedules {
        for producer in calls {
            for operation in producer
                .operands()
                .iter()
                .chain(std::iter::once(producer.operation()))
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            {
                let Some(call) = operation.call() else {
                    continue;
                };
                if operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .any(|existing| {
                        existing
                            .call()
                            .is_some_and(|existing| existing.coordinate == call.coordinate)
                    })
                {
                    // Statement-owned invocations already discharged these
                    // sources when their operation sequence was constructed.
                    continue;
                }
                let mut earlier = operations
                    .iter()
                    .filter(|operation| {
                        operation.structural_result_binding().is_some_and(|result| {
                            result.statement_index < call.coordinate.statement_index
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let arguments = call
                    .structural_arguments
                    .iter()
                    .filter(|argument| {
                        argument
                            .source_structural_result_binding_ordinal()
                            .is_some_and(|ordinal| {
                                earlier.iter().any(|operation| {
                                    operation
                                        .structural_result_binding()
                                        .is_some_and(|result| result.binding_ordinal == ordinal)
                                })
                            })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if arguments.is_empty() {
                    continue;
                }
                if let Some((_, target, previous)) = consumed
                    .iter()
                    .find(|(coordinate, _, _)| *coordinate == call.coordinate)
                {
                    if *target != call.target.state || *previous != arguments {
                        return None;
                    }
                    continue;
                }
                super::control::statement_sequence::consume_retained_results(
                    &mut earlier,
                    operation,
                )?;
                for producer in earlier {
                    let binding = producer.structural_result_binding()?;
                    let mut targets = operations.iter_mut().filter(|operation| {
                        operation
                            .structural_result_binding()
                            .is_some_and(|result| result.binding_ordinal == binding.binding_ordinal)
                    });
                    let target = targets.next()?;
                    if targets.next().is_some() {
                        return None;
                    }
                    *target = producer;
                }
                consumed.push((call.coordinate, call.target.state, arguments));
            }
        }
    }
    Some(())
}

fn caller_operations(
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> Option<&mut Vec<CheckedUnitEffectOperationPlan>> {
    let plans = &mut facts.flow.terminal_unit_effects;
    if let Some(index) = plans
        .machines
        .iter()
        .position(|plan| plan.machine == machine && plan.state == state)
    {
        return Some(&mut plans.machines[index].operations);
    }
    if let Some((owner, index)) = plans
        .composed_machines
        .iter()
        .enumerate()
        .filter(|(_, plan)| plan.machine == machine)
        .find_map(|(owner, plan)| {
            plan.states
                .iter()
                .position(|plan| plan.state == state)
                .map(|index| (owner, index))
        })
    {
        return Some(&mut plans.composed_machines[owner].states[index].operations);
    }
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)?
        .states
        .iter_mut()
        .find(|plan| plan.state == state)
        .map(|plan| &mut plan.unit_operations)
}

fn schedule(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    root: &CheckedStructuralValueRoot,
) -> Option<Vec<checked_trees::CheckedStructuralValueCall>> {
    let (machine, state) = crate::semantic::calls::find_state_with_machine(program, root.state)?;
    if machine.symbol != root.machine {
        return None;
    }
    let plans = &facts.flow.terminal_unit_effects;
    let ordinary = plans
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol && plan.state == state.symbol);
    let composed = plans
        .composed_machines
        .iter()
        .filter(|plan| plan.machine == machine.symbol)
        .flat_map(|plan| &plan.states)
        .find(|plan| plan.state == state.symbol);
    let scalar = facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .and_then(|graph| graph.states.iter().find(|plan| plan.state == state.symbol));
    let (parameters, entry_claims, operations) = if let Some(plan) = ordinary {
        (
            &plan.structural_parameters,
            plan.entry_claims.as_slice(),
            &plan.operations,
        )
    } else if let Some(plan) = composed {
        (
            &plan.structural_parameters,
            plan.entry_claims.as_slice(),
            &plan.operations,
        )
    } else if let Some(plan) = scalar {
        (&plan.structural_parameters, &[][..], &plan.unit_operations)
    } else {
        return None;
    };
    let mut preceding = Vec::new();
    let mut results = Vec::new();
    let mut count = 0;
    let source_flow = super::types::state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span(source_flow.calls)?;
    for operation in operations {
        let Some(result) = operation.structural_result_binding() else {
            let statement = operation
                .call()
                .map(|call| call.coordinate.statement_index)
                .or_else(|| operation.store_statement_index());
            if statement.is_some_and(|statement| statement < root.statement_ordinal) {
                preceding.push(operation.clone());
            }
            continue;
        };
        if result.statement_index >= root.statement_ordinal {
            continue;
        }
        count = count.max(result.binding_ordinal as usize + 1);
        let expression = if let Some(call) = operation.call() {
            source_calls
                .iter()
                .find(|source| {
                    source.statement_index == call.coordinate.statement_index as usize
                        && source.call_ordinal == call.coordinate.call_ordinal as usize
                })
                .map(|source| source.authored_expression)
        } else if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
            operation
        {
            Some(facts.values.structural_values.nodes.get(*value).expression)
        } else {
            None
        };
        let local = program
            .statement_table
            .statements(state.statement_nodes)
            .get(result.statement_index as usize)
            .and_then(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if operation.statement_structural_result_binding().is_some()
                        && expression
                            .is_none_or(|expression| expression == local.initial_value) =>
                {
                    Some(local.symbol)
                }
                _ => None,
            });
        let source = local
            .map(facts::PlaceRoot::Symbol)
            .or_else(|| expression.map(facts::PlaceRoot::Expression))?;
        results.push((result.clone(), source));
        preceding.push(operation.clone());
    }
    let trivial_locals = ordinary
        .into_iter()
        .flat_map(|plan| &plan.trivial_affine_locals)
        .filter_map(|local| {
            match program
                .statement_table
                .statements(state.statement_nodes)
                .get(local.declaration_ordinal as usize)?
            {
                typed_trees::statement::StatementNode::LocalData(source) => {
                    Some((local.clone(), source.symbol))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    structural_operands::value_calls(
        program,
        facts,
        ScalarCalleePlans {
            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &facts.flow.terminal_structural_scalar_returns,
        },
        shapes,
        machine,
        state,
        parameters,
        &trivial_locals,
        entry_claims,
        &results,
        &preceding,
        &mut count,
        root.root,
        &LocalConstructionTrace::default(),
    )
}
