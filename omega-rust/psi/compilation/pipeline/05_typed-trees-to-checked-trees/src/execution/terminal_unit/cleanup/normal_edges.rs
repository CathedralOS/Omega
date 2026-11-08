//! Selected normal-edge custody, expressed in the ordinary state value namespace.
//! The shared ownership selection supplies order and exact outgoing coordinates;
//! hook requirements remain obligations at that edge, never new unconditional facts.
mod exits;
pub(in crate::execution::terminal_unit) use exits::{state_exit_storage, state_exits};

use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedUnitCallableEntry,
    CheckedUnitStructuralArgumentSourcePlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

/// The producing layer joins source invocation reachability with its completed
/// physical control. Unknown source outcomes remain potentially normal.
pub(in crate::execution::terminal_unit) fn completed_body_outcome(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    physical_continuation: bool,
) -> checked_trees::CheckedNormalCompletion {
    if physical_continuation
        && validation::normal_completion_possible(program, state) != Some(false)
    {
        checked_trees::CheckedNormalCompletion::Possible
    } else {
        checked_trees::CheckedNormalCompletion::NoNormal
    }
}

/// Mutable construction scratch; only the shared cleanup arena is published.
struct NormalExitConstruction {
    exit: checked_trees::FlowClaimJoinExit,
    selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
    actions: Vec<checked_trees::CheckedNormalCleanupAction>,
    result_completion: arena::Handle<checked_trees::CheckedNormalResultCompletion>,
}

fn retain_normal_exits(
    shapes: &mut ShapeCollector<'_>,
    rows: Vec<NormalExitConstruction>,
    completion: checked_trees::CheckedNormalCompletion,
) -> checked_trees::CheckedNormalExitCleanups {
    let owner = &mut shapes.cleanup_control;
    let retained = rows
        .into_iter()
        .map(|row| checked_trees::CheckedNormalExitCleanupPlan {
            exit: row.exit,
            selected_regions: owner.normal_exit_regions.insert_many(row.selected_regions),
            actions: owner.actions.insert_many(row.actions),
            result_completion: row.result_completion,
        })
        .collect::<Vec<_>>();
    checked_trees::CheckedNormalExitCleanups {
        completion,
        rows: owner.normal_exit_rows.insert_many(retained),
    }
}

pub(in crate::execution::terminal_unit) fn state_cleanups(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    state_plan: &CheckedComposedUnitControlStatePlan,
) -> Result<checked_trees::CheckedNormalExitCleanups, &'static str> {
    let (machine, state) = program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .state_in_machine(machine, state_plan.state)
                .map(|state| (machine, state))
        })
        .ok_or("normal cleanup state has no owning machine")?;
    let completion = state_plan.normal_exit_cleanups.completion;
    let exits = state_exits(program, facts, machine.symbol, state)?;
    // No normal return does not erase an authored transfer's cleanup. Only
    // the absent source exit roster permits an unreachable physical edge to
    // carry no transaction.
    if completion == checked_trees::CheckedNormalCompletion::NoNormal && exits.is_empty() {
        return Ok(checked_trees::CheckedNormalExitCleanups {
            completion,
            ..Default::default()
        });
    }
    // A statically selected abnormal completion has no normal cleanup edge.
    // Source flow remains complete; this grants no normal-return disposition.
    if state_plan.selected_exit.is_some()
        && matches!(
            state_plan.terminator,
            checked_trees::CheckedComposedUnitControlTerminatorPlan::Crash { .. }
                | checked_trees::CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal { .. }
                | checked_trees::CheckedComposedUnitControlTerminatorPlan::EvaluateNoNormal
        )
    {
        return Ok(checked_trees::CheckedNormalExitCleanups {
            completion: checked_trees::CheckedNormalCompletion::NoNormal,
            ..Default::default()
        });
    }
    let rows = cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        machine.symbol,
        state,
        exits,
        state_plan.selected_exit,
        matches!(
            state_plan.terminator,
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Exits(_)
            }
        ),
        |root| owner_source(program, facts, state, state_plan, root).map(Some),
    )?;
    Ok(retain_normal_exits(shapes, rows, completion))
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
) -> Option<checked_trees::CheckedNormalExitCleanups> {
    let machine = program.machine_by_symbol(plan.machine)?;
    let state = program.state_in_machine(machine, plan.state)?;
    let completion = completed_body_outcome(
        program,
        state,
        plan.operations
            .iter()
            .all(|operation| operation.has_normal_continuation()),
    );
    let exits = state_exits(program, facts, plan.machine, state).ok()?;
    if completion == checked_trees::CheckedNormalCompletion::NoNormal && exits.is_empty() {
        return Some(checked_trees::CheckedNormalExitCleanups {
            completion,
            ..Default::default()
        });
    }
    let rows = cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        plan.machine,
        state,
        exits,
        None,
        plan.scalar_control.is_some(),
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
    Some(retain_normal_exits(shapes, rows, completion))
}

/// The normal-result producer retains selected arm structure. Only actual
/// result alternatives reach this tree; operand calls keep ordinary completion.
fn cleanup_rows(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    source_exits: Vec<checked_trees::FlowClaimJoinExit>,
    selected_exit: Option<checked_trees::FlowClaimJoinExit>,
    scalar_return: bool,
    mut source_for_owner: impl FnMut(
        PlaceRoot,
    ) -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
) -> Result<Vec<NormalExitConstruction>, &'static str> {
    if !scalar_return {
        return cleanup_rows_at(
            program,
            facts,
            targets,
            shapes,
            machine,
            state,
            &source_exits,
            selected_exit,
            None,
            source_for_owner,
        );
    }
    fn key(exit: checked_trees::FlowClaimJoinExit) -> Result<(usize, u8), &'static str> {
        use checked_trees::FlowClaimJoinExitKind::*;
        Ok((
            exit.statement_index,
            match exit.kind {
                FinalExpression => 0,
                TransitionTarget => 1,
                TransitionContinuation => 2,
                _ => return Err("normal result has no authored outgoing coordinate"),
            },
        ))
    }
    let mut values = std::collections::BTreeMap::<_, Vec<_>>::new();
    for row in exits::state_exit_rows(program, facts, machine, state)? {
        values
            .entry(key(row.exit)?)
            .or_default()
            .push(row.returned_value);
    }
    let cases = crate::flow::CleanupCaseSelections::new(facts, machine, state.symbol)
        .ok_or("normal result has no complete case selection roster")?;
    let mut rows = Vec::new();
    for exit in source_exits.iter().copied() {
        if selected_exit.is_some_and(|selected| selected != exit) {
            continue;
        }
        let mut captures = values
            .remove(&key(exit)?)
            .ok_or("normal result has no completed capture")?;
        captures.sort_unstable_by_key(|value| (value.arena_index(), value.generation()));
        captures.dedup();
        let completed =
            crate::flow::cleanup_result_selections(program, facts, exit, captures, cases.clone())
                .ok_or("normal result has no complete selected capture graph")?;
        let role = if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation {
            checked_trees::CheckedScalarExpressionRole::ContinuationReturn
        } else {
            checked_trees::CheckedScalarExpressionRole::Return
        };
        let mut roots = facts.values.scalar_computations.roots_for_statement_role(
            state.symbol,
            u32::try_from(exit.statement_index)
                .map_err(|_| "normal result statement exceeds u32")?,
            role,
        );
        let root = roots.next();
        if completed.nodes[0].branches.is_empty() || root.is_none() {
            rows.extend(cleanup_rows_at(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                &source_exits,
                Some(exit),
                None,
                &mut source_for_owner,
            )?);
            continue;
        }
        let root = root.ok_or("normal result has no owning computation")?;
        if roots.next().is_some() || root.machine != machine {
            return Err("normal result has no unique owning computation");
        }
        let mut regions = None;
        let result_completion = normal_result_tree(
            program,
            facts,
            targets,
            shapes,
            machine,
            state,
            &source_exits,
            exit,
            &completed,
            0,
            root.root,
            &mut source_for_owner,
            &mut regions,
            &mut std::collections::BTreeSet::new(),
        )?;
        // These leaves add no action or owned-region specialization. Keep the
        // existing complete continuation owner, including reference endings.
        let result_completion = if regions.as_ref().is_some_and(Vec::is_empty)
            && shapes
                .cleanup_control
                .result_actions(result_completion)
                .next()
                .is_none()
        {
            arena::Handle::invalid()
        } else {
            result_completion
        };
        rows.push(NormalExitConstruction {
            exit,
            // Every returning value leaf installs this roster. Its absence
            // means every retained leaf was declaration-proven NoNormal.
            selected_regions: regions.unwrap_or_default(),
            actions: Vec::new(),
            result_completion,
        });
    }
    Ok(rows)
}

/// Publish the exact checked result structure with each leaf transaction.
/// Correspondence and complete coverage are decided here, before lowering.
#[allow(clippy::too_many_arguments)]
fn normal_result_tree(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    source_exits: &[checked_trees::FlowClaimJoinExit],
    exit: checked_trees::FlowClaimJoinExit,
    completed: &crate::flow::CompletedCleanupResults<'_>,
    cut: usize,
    computation: checked_trees::CheckedScalarComputationHandle,
    source_for_owner: &mut impl FnMut(
        PlaceRoot,
    ) -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
    regions: &mut Option<Vec<checked_trees::CheckedUnitStructuralArgumentPlan>>,
    active: &mut std::collections::BTreeSet<(u32, u32)>,
) -> Result<arena::Handle<checked_trees::CheckedNormalResultCompletion>, &'static str> {
    let plans = &facts.values.scalar_computations;
    let key = (computation.arena_index(), computation.generation());
    if !plans.nodes.is_valid(computation) || !active.insert(key) {
        return Err("normal result has a stale or cyclic computation");
    }
    let source = completed
        .nodes
        .get(cut)
        .ok_or("normal result lost its completed cut")?;
    let node = plans.nodes.get(computation);
    let result = if validation::expression_normal_completion_possible(program, source.expression)
        == Some(false)
    {
        checked_trees::CheckedNormalResultCompletion::NoNormal { computation }
    } else if let checked_trees::CheckedScalarComputationKind::Dispatch {
        source_expression,
        arms,
        ..
    } = &node.kind
        && !source.branches.is_empty()
    {
        if *source_expression != source.expression {
            return Err("normal result selection changed its source expression");
        }
        let arm_span = *arms;
        let arms = plans
            .dispatch_arms
            .span(arm_span)
            .filter(|arms| !arms.is_empty())
            .ok_or("normal result selection lost its arm roster")?;
        let mut retained = Vec::with_capacity(arms.len());
        let mut seen = std::collections::BTreeSet::new();
        for (arm_handle, arm) in arm_span.handles().zip(arms) {
            let key = (arm.source_arm.arena_index(), arm.source_arm.generation());
            if !seen.insert(key) {
                return Err("normal result selection repeats a source arm");
            }
            let child = *source
                .branches
                .get(&key)
                .ok_or("normal result arm has no completed source capture")?;
            let result = normal_result_tree(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                source_exits,
                exit,
                completed,
                child,
                arm.value,
                source_for_owner,
                regions,
                active,
            )?;
            retained.push(checked_trees::CheckedNormalResultArm {
                arm: arm_handle,
                result,
            });
        }
        if seen.len() != source.branches.len() {
            return Err("normal result selection omits a completed source arm");
        }
        checked_trees::CheckedNormalResultCompletion::Dispatch {
            computation,
            arms: shapes
                .cleanup_control
                .normal_result_arms
                .insert_many(retained),
        }
    } else {
        let mut rows = cleanup_rows_at(
            program,
            facts,
            targets,
            shapes,
            machine,
            state,
            source_exits,
            Some(exit),
            Some(&source.result),
            &mut *source_for_owner,
        )?;
        if rows.len() != 1 {
            return Err("normal result leaf has no unique transaction");
        }
        let row = rows
            .pop()
            .ok_or("normal result leaf has no unique transaction")?;
        if let Some(expected) = regions.as_ref() {
            if expected != &row.selected_regions {
                return Err("normal result leaves disagree on their selected owned homes");
            }
        } else {
            *regions = Some(row.selected_regions);
        }
        checked_trees::CheckedNormalResultCompletion::Value {
            computation,
            actions: shapes.cleanup_control.actions.insert_many(row.actions),
        }
    };
    active.remove(&key);
    Ok(shapes
        .cleanup_control
        .normal_result_completions
        .append(result))
}

fn cleanup_rows_at(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    source_exits: &[checked_trees::FlowClaimJoinExit],
    selected_exit: Option<checked_trees::FlowClaimJoinExit>,
    completed_result: Option<&crate::flow::CompletedCleanupResult<'_>>,
    mut source_for_owner: impl FnMut(
        PlaceRoot,
    ) -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
) -> Result<Vec<NormalExitConstruction>, &'static str> {
    let flow_state = facts
        .flow
        .control
        .state(machine, state.symbol)
        .ok_or("normal cleanup state has no retained control flow")?;
    if selected_exit.is_some_and(|selected| !source_exits.contains(&selected)) {
        return Err("selected normal cleanup has no exact source exit");
    }
    let mut rows = source_exits
        .iter()
        .copied()
        .filter(|exit| selected_exit.is_none_or(|selected| selected == *exit))
        .map(|exit| NormalExitConstruction {
            exit,
            selected_regions: Vec::new(),
            actions: Vec::new(),
            result_completion: Default::default(),
        })
        .collect::<Vec<_>>();
    let mut retirements = Vec::new();
    let mut establishment_ordered = Vec::new();
    for disposition_handle in facts.contextual_cleanup.state_disposition_handles(
        super::owning_cleanup_machine(program, state.symbol),
        state.symbol,
    ) {
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
        if selected_exit.is_some_and(|selected| disposition.exit != selected) {
            // Preserve the full authored receipt roster. Only a different,
            // validated source exit is absent from this executable completion;
            // malformed or missing coordinates must not disappear here.
            if !source_exits.contains(&disposition.exit) {
                return Err("normal cleanup disposition has no matching exit");
            }
            continue;
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
        let actions = super::actions::disposition_actions_at(
            program,
            facts,
            targets,
            shapes,
            disposition_handle,
            source.clone(),
            None,
            completed_result,
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
                crate::shared::collections::push_unique(&mut row.selected_regions, region);
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
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    logical_root_type_identity: None,
                },
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
