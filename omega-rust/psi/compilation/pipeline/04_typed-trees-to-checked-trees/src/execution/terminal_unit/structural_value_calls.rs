//! Structural expression consumers share ordinary call operation schedules.
//! A catalog describes each value's calls; only the value evaluator executes
//! them, at their exact DAG positions. Preceding statement results supply the
//! same source namespace as an ordinary construction or call argument.
//! Result bindings retain their named home; anonymous producers retain their
//! authored occurrence. Internal displacement slots still reserve ordinals and
//! stay in the preamble without pretending to bind an authored source. An
//! unrelated preceding producer must not erase a later value's call schedule.
//!
//! Prepare schedules beside each candidate's operation prefix, before result
//! custody and dependency closure need them. The same batch supplies ownership
//! queries, callable dependencies and temporary types. Failed candidates take
//! their schedules with them; final retention only publishes surviving batches.
//! Preparing calls after body publication would make nested transfers invisible
//! while deciding whether that body can be published in the first place.

use super::control::{LocalConstructionTrace, structural_operands};
use super::types::ShapeCollector;
use super::{CheckFacts, ScalarCalleeClosure};
use checked_trees::{CheckedStructuralValueRoot, CheckedUnitEffectOperationPlan};

/// Construction scratch, committed only with the body that supplied its exact
/// predecessor namespace. Custody queries borrow it before body publication.
pub(crate) struct PreparedValueCalls {
    pub owner: symbols::SymbolHandle,
    pub state: symbols::SymbolHandle,
    pub value: checked_trees::CheckedStructuralValueHandle,
    pub preceding_bindings: u32,
    pub calls: Vec<checked_trees::CheckedStructuralValueCall>,
}

impl PreparedValueCalls {
    pub(super) fn operation_dependencies(
        &self,
    ) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        self.calls.iter().flat_map(|call| {
            call.operands()
                .iter()
                .chain(std::iter::once(call.operation()))
                .chain(call.completions().iter())
        })
    }
}

pub(super) struct StateCallInputs<'state> {
    pub parameters: &'state [checked_trees::CheckedUnitStructuralParameterPlan],
    pub entry_claims: &'state [checked_trees::CheckedUnitEntryClaimPlan],
    pub operations: &'state [CheckedUnitEffectOperationPlan],
    pub trivial_locals: &'state [checked_trees::CheckedTrivialAffineStructuralLocalPlan],
}

pub(super) fn prepare_state(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    owner: symbols::SymbolHandle,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    inputs: &StateCallInputs<'_>,
) -> Vec<PreparedValueCalls> {
    let mut prepared = Vec::new();
    for root in facts.values.structural_values.roots_for_state(state) {
        if root.machine != machine
            || root.state != state
            || prepared
                .iter()
                .any(|row: &PreparedValueCalls| row.value == root.root)
        {
            continue;
        }
        if let Some((preceding_bindings, calls)) = schedule(program, facts, shapes, root, inputs) {
            prepared.push(PreparedValueCalls {
                owner,
                state,
                value: root.root,
                preceding_bindings,
                calls,
            });
        }
    }
    prepared
}

pub(crate) fn retain(
    facts: &mut CheckFacts,
    prepared: Vec<PreparedValueCalls>,
) -> Result<(), String> {
    // Only surviving candidates reach here. Fused dispatch owners can share
    // an authored root, but they must agree on its exact call namespace.
    for row in prepared {
        if let Some((preceding, calls)) = facts.values.structural_values.call_schedule(row.value) {
            if preceding != row.preceding_bindings || calls != row.calls {
                return Err("surviving bodies disagree on a structural value call schedule".into());
            }
            continue;
        }
        facts.values.structural_values.retain_calls_for_root(
            row.value,
            row.preceding_bindings,
            row.calls,
        );
    }
    Ok(())
}

/// Catalog calls execute in scalar/value continuations, while the preceding
/// producer's disposal debt belongs to its enclosing checked operation body.
/// Rejoin that shared namespace inside the candidate using ordinary transfer rules.
pub(super) fn consume_caller_results<'a>(
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
                // Named homes have exact selected normal-exit cleanup rows.
                // Their initializer's provisional discard is not an ownership
                // bit, and a conditional move does not erase sibling cleanup.
                // Anonymous results still rely on their producer's disposal
                // flag, so clearing that flag requires an unconditional move.
                let retires_caller = arguments.iter().any(|argument| {
                    argument.access == checked_trees::CheckedStructuralAccess::Owned
                        && argument.path.is_empty()
                        && earlier.iter().any(|operation| {
                            operation.discardable_structural_result().is_some_and(
                                |(result, discard)| {
                                    Some(result.binding_ordinal)
                                        == argument.source_structural_result_binding_ordinal()
                                        && discard
                                        && !result.local.is_named()
                                        && result.multiplicity
                                            == language_semantics::Multiplicity::Affine
                                },
                            )
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

fn schedule(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    root: &CheckedStructuralValueRoot,
    inputs: &StateCallInputs<'_>,
) -> Option<(u32, Vec<checked_trees::CheckedStructuralValueCall>)> {
    let (machine, state) = validation::find_state(program, root.state)?;
    if machine.symbol != root.machine {
        return None;
    }
    let mut preceding = Vec::new();
    let mut results = Vec::new();
    let mut count = 0;
    let source_flow = super::types::state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span(source_flow.calls)?;
    for operation in inputs.operations {
        let Some(result) = operation.structural_result_binding() else {
            let statement = operation.source_statement_index();
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
    let trivial_locals = inputs
        .trivial_locals
        .iter()
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
        inputs.parameters,
        &trivial_locals,
        inputs.entry_claims,
        &results,
        &preceding,
        &mut count,
        root.root,
        &LocalConstructionTrace::default(),
        &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
    )?;
    Some((preceding_bindings, calls))
}
