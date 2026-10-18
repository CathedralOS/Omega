//! Structural expression consumers share ordinary call operation schedules.
//! A catalog describes each value's calls; only the value evaluator executes
//! them, at their exact DAG positions. Preceding statement results supply the
//! same source namespace as an ordinary construction or call argument.
//! Result bindings retain their named home; anonymous producers retain their
//! authored occurrence. Internal displacement slots still reserve ordinals and
//! stay in the preamble without pretending to bind an authored source. An
//! unrelated preceding producer must not erase a later value's call schedule.

use super::control::{LocalConstructionTrace, structural_operands};
use super::types::ShapeCollector;
use super::{CheckFacts, ScalarCalleeClosure};
use checked_trees::{CheckedStructuralValueRoot, CheckedUnitEffectOperationPlan};

pub(crate) fn retain(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), typed_trees::type_identity::TypeIdentityId> {
    let mut shapes = ShapeCollector::new(program);
    let mut schedules = Vec::new();
    for (_, root) in facts.values.structural_values.roots.iter() {
        if schedules
            .iter()
            .any(|(_, _, value, _, _)| *value == root.root)
        {
            continue;
        }
        if let Some((preceding_bindings, calls)) = schedule(program, facts, &mut shapes, root) {
            schedules.push((
                root.machine,
                root.state,
                root.root,
                preceding_bindings,
                calls,
            ));
        }
    }
    let mut completed = Vec::new();
    for (machine, state, _, _, _) in &schedules {
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
            .filter(|(owner, source, _, _, _)| owner == machine && source == state);
        if consume_caller_results(
            program,
            *machine,
            *state,
            &mut updated,
            state_schedules.map(|(_, _, _, _, calls)| calls.as_slice()),
        )
        .is_none()
        {
            // A schedule without coherent source custody is not executable.
            // Do not publish partial transfers or the calls that would use them.
            continue;
        }
        *operations = updated;
        for (_, _, value, preceding_bindings, calls) in schedules
            .iter()
            .filter(|(owner, source, _, _, _)| owner == machine && source == state)
        {
            facts.values.structural_values.retain_calls_for_root(
                *value,
                *preceding_bindings,
                calls.clone(),
            );
        }
    }
    // A temporary's result type need not occur in its caller's signature.
    // Retain the ordinary call builder's type declarations under the same
    // namespace as all other structural operation results.
    facts
        .flow
        .terminal_unit_effects
        .adopt_structural_types(shapes.types.into_values())?;
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
    Ok(())
}

/// Catalog calls execute in scalar/value continuations, while the preceding
/// producer's disposal debt belongs to its enclosing checked operation body.
/// Rejoin that shared namespace at publication using ordinary transfer rules.
fn consume_caller_results<'a>(
    program: &dyn ::validation::CheckingInput,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
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
                let retires_caller = arguments.iter().any(|argument| {
                    argument.access == checked_trees::CheckedStructuralAccess::Owned
                        && argument.path.is_empty()
                        && earlier.iter().any(|operation| {
                            operation.structural_result_binding().is_some_and(|result| {
                                Some(result.binding_ordinal)
                                    == argument.source_structural_result_binding_ordinal()
                                    && result.multiplicity
                                        == language_semantics::Multiplicity::Affine
                            })
                        })
                });
                if retires_caller {
                    validation::call_is_unconditional(
                        program,
                        machine,
                        state,
                        call.coordinate.statement_index as usize,
                        call.coordinate.call_ordinal as usize,
                    )?;
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
        .find_map(|(owner, plan)| plan.state_position(state).map(|index| (owner, index)))
    {
        return Some(&mut plans.composed_machines[owner].states[index].operations);
    }
    None
}

fn schedule(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    root: &CheckedStructuralValueRoot,
) -> Option<(u32, Vec<checked_trees::CheckedStructuralValueCall>)> {
    let (machine, state) = validation::find_state(program, root.state)?;
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
        } else if let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) = operation
        {
            Some(facts.values.structural_values.nodes.get(*value).expression)
        } else {
            match operation {
                CheckedUnitEffectOperationPlan::EstablishReference {
                    formation_expression,
                    ..
                } => Some(*formation_expression),
                CheckedUnitEffectOperationPlan::MoveStructuralField { expression, .. }
                    if expression.is_valid() =>
                {
                    Some(*expression)
                }
                _ => None,
            }
        };
        // The binding owner was paired at production, not inferred from an
        // enclosing statement shared by several anonymous child producers.
        let source = result
            .local
            .is_named()
            .then_some(facts::PlaceRoot::Symbol(result.local.symbol))
            .or_else(|| expression.map(facts::PlaceRoot::Expression));
        if let Some(source) = source {
            results.push((result.clone(), source));
        }
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
    let preceding_bindings = u32::try_from(count).ok()?;
    let calls = structural_operands::value_calls(
        program,
        facts,
        ScalarCalleeClosure,
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
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    Some((preceding_bindings, calls))
}
