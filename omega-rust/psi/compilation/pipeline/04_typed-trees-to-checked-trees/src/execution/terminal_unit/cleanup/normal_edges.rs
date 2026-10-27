//! Selected normal-edge custody, expressed in the ordinary state value namespace.
//! The shared ownership selection supplies order and exact outgoing coordinates;
//! hook requirements remain obligations at that edge, never new unconditional facts.
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedUnitCallableEntry,
    CheckedUnitStructuralArgumentSourcePlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

/// Mutable construction scratch; only the shared cleanup arena is published.
struct NormalExitConstruction {
    exit: checked_trees::FlowClaimJoinExit,
    selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
    actions: Vec<checked_trees::CheckedNormalCleanupAction>,
}

fn retain_normal_exits(
    shapes: &mut ShapeCollector<'_>,
    rows: Vec<NormalExitConstruction>,
) -> arena::HandleSpan<checked_trees::CheckedNormalExitCleanupPlan> {
    let owner = &mut shapes.cleanup_control;
    let retained = rows
        .into_iter()
        .map(|row| checked_trees::CheckedNormalExitCleanupPlan {
            exit: row.exit,
            selected_regions: owner.normal_exit_regions.insert_many(row.selected_regions),
            actions: owner.actions.insert_many(row.actions),
        })
        .collect::<Vec<_>>();
    owner.normal_exit_rows.insert_many(retained)
}

pub(in crate::execution::terminal_unit) fn state_cleanups(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    state_plan: &CheckedComposedUnitControlStatePlan,
) -> Result<arena::HandleSpan<checked_trees::CheckedNormalExitCleanupPlan>, &'static str> {
    if state_plan
        .terminator
        .has_normal_continuation(&state_plan.operations, &facts.flow.guarded_exits)
        == Some(false)
    {
        return Ok(Default::default());
    }
    let (machine, state) = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .state_in_machine(machine, state_plan.state)
                .map(|state| (machine, state))
        })
        .ok_or("normal cleanup state has no owning machine")?;
    let rows = cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        machine.symbol,
        state,
        |root| owner_source(program, facts, state, state_plan, root).map(Some),
    )?;
    Ok(retain_normal_exits(shapes, rows))
}

/// Sequence completion disposes current named storage through the same checked
/// exits as state graphs. Anonymous evaluation results keep their own producer
/// disposition; named locals must never revive an overwritten birth result.
pub(in crate::execution::terminal_unit) fn sequence_cleanups(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    plan: &checked_trees::CheckedUnitEffectMachinePlan,
) -> Option<arena::HandleSpan<checked_trees::CheckedNormalExitCleanupPlan>> {
    if plan
        .operations
        .iter()
        .any(|operation| !operation.has_normal_continuation())
    {
        return Some(Default::default());
    }
    let machine = program.machine_by_symbol(plan.machine)?;
    let state = program.state_in_machine(machine, plan.state)?;
    let rows = cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        plan.machine,
        state,
        |root| {
            let operations = &plan.operations;
            if root.as_expression().is_some() {
                return expression_source(facts, state.symbol, operations, root).map(Some);
            }
            let position = program
                .state_parameters(state)
                .iter()
                .position(|parameter| root == PlaceRoot::Symbol(parameter.symbol));
            let Some(position) = position else {
                return Some(super::super::local_storage::source(
                    program,
                    state,
                    operations,
                    root.as_symbol()?,
                ));
            };
            let Some(index) = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position as usize == position)
            else {
                return Some(None);
            };
            Some(Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }))
        },
    )
    .ok()?;
    Some(retain_normal_exits(shapes, rows))
}

fn cleanup_rows(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    mut source_for_owner: impl FnMut(
        PlaceRoot,
    ) -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
) -> Result<Vec<NormalExitConstruction>, &'static str> {
    let flow_state = facts
        .flow
        .control
        .state(machine, state.symbol)
        .ok_or("normal cleanup state has no retained control flow")?;
    let mut rows = Vec::<NormalExitConstruction>::new();
    for outgoing in facts
        .flow
        .control
        .outgoing
        .span_or_empty(flow_state.outgoing)
    {
        let kind = if !outgoing.transition_target.is_valid() {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        } else {
            let transition = program
                .statement_table
                .statement_at(state, outgoing.statement_index)
                .ok_or("normal cleanup edge has no source statement")?
                .as_transition()
                .ok_or("normal cleanup edge is not a direct transition")?;
            if transition.target == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
            } else if transition.continuation == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            } else {
                return Err("normal cleanup edge does not select the source transition");
            }
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: state.symbol,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !rows.iter().any(|row| row.exit == exit) {
            rows.push(NormalExitConstruction {
                exit,
                selected_regions: Vec::new(),
                actions: Vec::new(),
            });
        }
    }
    let mut retirements = Vec::new();
    let mut establishment_ordered = Vec::new();
    for disposition_handle in facts
        .contextual_cleanup
        .state_disposition_handles(machine, state.symbol)
    {
        let disposition = facts
            .contextual_cleanup
            .dispositions
            .get(disposition_handle);
        if disposition.source != PermissionEventSource::StateExit {
            continue;
        }
        if disposition.exit.state_symbol != state.symbol {
            return Err("normal cleanup disposition belongs to another state");
        }
        // Capture authority belongs to the selected receipt, not a mutable
        // declaration namespace or a re-resolved prefix of its operation list.
        let captured = if disposition.selection.is_valid() {
            let receipt = facts
                .flow
                .ownership
                .owned_selections
                .get(disposition.selection);
            if receipt.state != state.symbol || receipt.machine != machine {
                return Err("normal cleanup receipt belongs to another state or machine");
            }
            let mut sources = facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .enumerate()
                .filter(|(_, source)| Some(source.symbol) == disposition.root.as_symbol());
            let (ordinal, _) = sources
                .next()
                .ok_or("normal cleanup receipt has no source for its root")?;
            if sources.next().is_some() {
                return Err("normal cleanup receipt has multiple sources for its root");
            }
            Some(CheckedUnitStructuralArgumentSourcePlan::CapturedStorage {
                selection: disposition.selection,
                source_ordinal: u32::try_from(ordinal)
                    .map_err(|_| "normal cleanup source ordinal exceeds u32")?,
            })
        } else {
            None
        };
        let source = match captured {
            Some(source) => Some(source),
            None => source_for_owner(disposition.root)
                .ok_or("normal cleanup owner has no retained structural storage")?,
        };
        let Some(source) = source else {
            continue;
        };
        if disposition.establishment_ordered
            && !establishment_ordered.contains(&(disposition.exit, disposition.root))
        {
            establishment_ordered.push((disposition.exit, disposition.root));
        }
        if !retirements.iter().any(
            |(exit, retirement, _): &(_, checked_trees::CheckedNormalCleanupAction, _)| {
                *exit == disposition.exit && retirement.source() == &source
            },
        ) {
            retirements.push((
                disposition.exit,
                super::actions::home_retirement(program, shapes, disposition, source.clone())
                    .ok_or("normal cleanup cannot realize the retiring home")?,
                disposition.root,
            ));
        }
        let actions = super::actions::disposition_actions(
            program,
            facts,
            targets,
            shapes,
            disposition_handle,
            source.clone(),
            None,
        )?;
        if let Some(row) = rows.iter_mut().find(|row| row.exit == disposition.exit) {
            if facts
                .contextual_cleanup
                .is_selected_disposition(disposition_handle)
            {
                let region = super::actions::selected_region(
                    program,
                    facts,
                    shapes,
                    disposition_handle,
                    source.clone(),
                    None,
                )
                .ok_or("normal cleanup cannot realize its selected region")?;
                crate::collections::push_unique(&mut row.selected_regions, region);
            }
            row.actions.extend(actions);
        } else {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE normal-cleanup-unmatched machine={machine:?} state={:?} disposition={disposition:?} exits={:?} outgoing={:?}",
                    state.symbol,
                    rows.iter().take(8).map(|row| row.exit).collect::<Vec<_>>(),
                    facts
                        .flow
                        .control
                        .outgoing
                        .span_or_empty(flow_state.outgoing)
                        .iter()
                        .take(8)
                        .collect::<Vec<_>>(),
                );
            }
            return Err("normal cleanup disposition has no matching exit");
        }
    }
    for (exit, retirement, root) in retirements {
        let row = rows
            .iter_mut()
            .find(|row| row.exit == exit)
            .ok_or("normal cleanup retirement has no matching exit")?;
        // A root still under construction keeps the producer's reverse
        // establishment order; the declared order below is for a completed
        // aggregate whose candidate regions join its residual partition.
        if !establishment_ordered.contains(&(exit, root))
            && row
                .selected_regions
                .iter()
                .any(|region| &region.source == retirement.source() && !region.path.is_empty())
        {
            super::order::merge(
                program,
                state.symbol,
                exit.statement_index,
                root,
                &mut row.actions,
                retirement.source(),
            )
            .ok_or("normal cleanup cannot order the selected region actions")?;
        }
        super::actions::finish_home(&mut row.actions, retirement);
    }
    Ok(rows)
}

fn owner_source(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    plan: &CheckedComposedUnitControlStatePlan,
    root: PlaceRoot,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    let operations = &plan.operations;
    if root.as_expression().is_some() {
        return expression_source(facts, state.symbol, operations, root);
    }
    let symbol = root.as_symbol()?;
    if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), symbol)
    {
        let index = plan
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        return Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(index).ok()?,
        });
    }
    super::super::local_storage::source(program, state, operations, symbol)
}

pub(in crate::execution::terminal_unit) fn expression_source(
    facts: &CheckFacts,
    state: SymbolHandle,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    root: PlaceRoot,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    let mut matching = operations.iter().filter_map(|operation| {
        let result = operation.structural_result_binding()?;
        (producer_root(facts, state, operation) == Some(root)).then_some(result)
    });
    let Some(result) = matching.next() else {
        // Computation-owned values live in the expression namespace rather
        // than the flat operation-result roster. Cleanup names that retained
        // owner; emission must find its already-established storage and must
        // never evaluate the expression to obtain a cleanup operand.
        let expression = root.as_expression()?;
        return facts
            .values
            .structural_values
            .nodes
            .iter()
            .any(|(_, value)| value.state == state && value.expression == expression)
            .then_some(
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression },
            );
    };
    if matching.next().is_some() {
        return None;
    }
    Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
        binding_ordinal: result.binding_ordinal,
    })
}

/// Join a retained result to the source occurrence that established it. The
/// call coordinate is authoritative; equal expressions or return types cannot
/// identify a second evaluation as the owner of the first one's cleanup.
pub(in crate::execution::terminal_unit) fn producer_root(
    facts: &CheckFacts,
    state: SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Option<PlaceRoot> {
    use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitLocalBindingPlan};
    let result = operation.structural_result_binding()?;
    if result.local.is_named() {
        return Some(PlaceRoot::Symbol(result.local.symbol));
    }
    let expression = match operation {
        CheckedUnitEffectOperationPlan::EstablishLocal(
            CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
        ) => facts.values.structural_values.nodes.get(*value).expression,
        CheckedUnitEffectOperationPlan::Call(call) => {
            let flow = facts.flow.control.state_by_symbol(state)?;
            let mut calls = facts
                .flow
                .control
                .calls
                .span(flow.calls)?
                .iter()
                .filter(|row| {
                    row.statement_index == call.coordinate.statement_index as usize
                        && row.call_ordinal == call.coordinate.call_ordinal as usize
                });
            let expression = calls.next()?.authored_expression;
            if calls.next().is_some() {
                return None;
            }
            expression
        }
        _ => return None,
    };
    expression
        .is_valid()
        .then_some(PlaceRoot::Expression(expression))
}
