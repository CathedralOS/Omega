//! Structural expression consumers share ordinary call operation schedules.
//! A catalog describes each value's calls; only the value evaluator executes
//! them, at their exact DAG positions. Preceding statement results supply the
//! same source namespace as an ordinary construction or call argument.
//! Result bindings retain their named home; anonymous producers retain their
//! authored occurrence. Internal displacement slots still reserve ordinals and
//! stay in the preamble without pretending to bind an authored source. An
//! unrelated preceding producer must not erase a later value's call schedule.
//!
//! Prepare schedules against candidate prefixes before custody or dependency
//! admission needs them. Prefix queries and completed-state preparation share
//! one catalogue and one root builder. Rejected bodies discard their schedules;
//! final retention publishes surviving schedules without rebuilding calls.

use super::control::{LocalConstructionTrace, structural_operands};
use super::types::ShapeCollector;
use super::{CheckFacts, ScalarCalleeClosure};
use checked_trees::{CheckedStructuralValueRoot, CheckedUnitEffectOperationPlan};

/// Calls prepared against one candidate body's real operation prefix. This
/// scratch belongs to the construction attempt; only its winning body moves
/// the rows into the retained value arenas. Queries borrow the same executable
/// schedules that lowering will later consume, never reconstructing a call.
#[derive(Default)]
pub(crate) struct PreparedCalls {
    rows: Vec<PreparedRoot>,
}

struct PreparedRoot {
    owner: symbols::SymbolHandle,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    value: checked_trees::CheckedStructuralValueHandle,
    preceding_bindings: u32,
    calls: Vec<checked_trees::CheckedStructuralValueCall>,
}

impl PreparedRoot {
    fn operation_dependencies(&self) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        self.calls.iter().flat_map(|call| {
            call.operands()
                .iter()
                .chain(std::iter::once(call.operation()))
                .chain(call.completions().iter())
        })
    }
}

impl PreparedCalls {
    pub(super) fn append(&mut self, owner: symbols::SymbolHandle, mut completed: Self) {
        for row in &mut completed.rows {
            row.owner = owner;
        }
        self.rows.append(&mut completed.rows);
    }

    pub(super) fn retain_bodies(
        &mut self,
        mut survives: impl FnMut(symbols::SymbolHandle, symbols::SymbolHandle) -> bool,
    ) {
        self.rows.retain(|row| survives(row.owner, row.state));
    }

    pub(super) fn state_schedules(
        &self,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &[checked_trees::CheckedStructuralValueCall]> {
        self.rows
            .iter()
            .filter(move |row| row.machine == machine && row.state == state)
            .map(|row| row.calls.as_slice())
    }

    pub(super) fn operations_for(
        &self,
        owner: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        self.rows
            .iter()
            .filter(move |row| row.owner == owner && row.state == state)
            .flat_map(PreparedRoot::operation_dependencies)
    }

    pub(super) fn operation_dependencies(
        &self,
    ) -> impl Iterator<Item = &CheckedUnitEffectOperationPlan> {
        self.rows
            .iter()
            .flat_map(PreparedRoot::operation_dependencies)
    }

    pub(super) const EMPTY: Self = Self { rows: Vec::new() };

    pub(super) fn schedule(
        &self,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Option<&[checked_trees::CheckedStructuralValueCall]> {
        let mut rows = self.rows.iter().filter(|row| row.value == value);
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        Some(&row.calls)
    }

    /// Complete only roots whose authored statement has executed. A future
    /// selected root remains unavailable to the conservative future-use fence.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_before(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        trivial_locals: &[(
            checked_trees::CheckedTrivialAffineStructuralLocalPlan,
            symbols::SymbolHandle,
        )],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        operations: &[CheckedUnitEffectOperationPlan],
        boundary: usize,
    ) {
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        for root in facts.values.structural_values.roots_for_state(state.symbol) {
            if root.machine != machine.symbol
                || root.state != state.symbol
                || !order.precedes(root.statement_ordinal as usize, boundary)
                || self.rows.iter().any(|row| row.value == root.root)
            {
                continue;
            }
            if let Some((preceding_bindings, calls)) = prepare_root(
                program,
                facts,
                shapes,
                machine,
                state,
                parameters,
                trivial_locals,
                entry_claims,
                operations,
                root,
            ) {
                self.rows.push(PreparedRoot {
                    owner: machine.symbol,
                    machine: root.machine,
                    state: root.state,
                    value: root.root,
                    preceding_bindings,
                    calls,
                });
            }
        }
    }
}

/// Keep nested operand preparations in their owning root's namespace. A
/// separately catalogued inner root can start with different private ordinals.
/// Existing operation traversal visits each embedded preparation and normal
/// completion; catalog enumeration still does not select an execution path.
pub(super) fn dependencies(
    calls: &[checked_trees::CheckedStructuralValueCall],
) -> impl Iterator<Item = &checked_trees::CheckedStructuralValueCall> {
    calls.iter().chain(calls.iter().flat_map(|call| {
        call.operands()
            .iter()
            .chain(call.completions().iter())
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
                ) => Some(calls.as_slice()),
                _ => None,
            })
            .flatten()
    }))
}

pub(crate) fn retain(facts: &mut CheckFacts, prepared: PreparedCalls) -> Result<(), String> {
    // Only surviving candidates reach publication. Fused dispatch owners may
    // share an authored root, but must agree on its exact executable namespace.
    for row in prepared.rows {
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
    let authored_state = program.state_by_symbol(state)?;
    let order = program
        .statement_table
        .admitted_execution_order(authored_state.statement_nodes);
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
                            order.precedes(
                                result.statement_index as usize,
                                call.coordinate.statement_index as usize,
                            )
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

pub(super) struct StateCallInputs<'state> {
    pub parameters: &'state [checked_trees::CheckedUnitStructuralParameterPlan],
    pub entry_claims: &'state [checked_trees::CheckedUnitEntryClaimPlan],
    pub operations: &'state [CheckedUnitEffectOperationPlan],
    pub trivial_locals: &'state [checked_trees::CheckedTrivialAffineStructuralLocalPlan],
}

/// Complete the same catalogue after the full candidate prefix is known.
/// Earlier query boundaries keep their schedules and private ordinals; only
/// missing roots are prepared here, before custody and dependency admission.
pub(super) fn prepare_state(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    prepared: &mut PreparedCalls,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    inputs: &StateCallInputs<'_>,
) -> Option<()> {
    let (machine, state) =
        validation::find_state(program, state).filter(|(owner, _)| owner.symbol == machine)?;
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
    for root in facts.values.structural_values.roots_for_state(state.symbol) {
        if root.machine != machine.symbol
            || root.state != state.symbol
            || prepared.rows.iter().any(|row| row.value == root.root)
        {
            continue;
        }
        if let Some((preceding_bindings, calls)) = prepare_root(
            program,
            facts,
            shapes,
            machine,
            state,
            inputs.parameters,
            &trivial_locals,
            inputs.entry_claims,
            inputs.operations,
            root,
        ) {
            prepared.rows.push(PreparedRoot {
                owner: machine.symbol,
                machine: root.machine,
                state: root.state,
                value: root.root,
                preceding_bindings,
                calls,
            });
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_root(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    trivial_locals: &[(
        checked_trees::CheckedTrivialAffineStructuralLocalPlan,
        symbols::SymbolHandle,
    )],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    root: &CheckedStructuralValueRoot,
) -> Option<(u32, Vec<checked_trees::CheckedStructuralValueCall>)> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut preceding = Vec::new();
    let mut results = Vec::new();
    let mut count = 0;
    let source_flow = super::types::state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span(source_flow.calls)?;
    for operation in operations {
        let Some(result) = operation.structural_result_binding() else {
            let statement = operation.source_statement_index();
            if statement.is_some_and(|statement| {
                order.precedes(statement as usize, root.statement_ordinal as usize)
            }) {
                preceding.push(operation.clone());
            }
            continue;
        };
        if !order.precedes(
            result.statement_index as usize,
            root.statement_ordinal as usize,
        ) {
            continue;
        }
        count = count.max(result.binding_ordinal as usize + 1);
        if let Some(owner) = super::calls::structural_result_owner(
            program,
            facts,
            state.symbol,
            source_calls,
            operation,
        ) {
            results.push((result.clone(), owner));
        }
        preceding.push(operation.clone());
    }
    let preceding_bindings = u32::try_from(count).ok()?;
    let calls = structural_operands::value_calls(
        program,
        facts,
        ScalarCalleeClosure,
        shapes,
        machine,
        state,
        parameters,
        trivial_locals,
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
