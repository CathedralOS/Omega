//! Selected normal-edge custody, expressed in the ordinary state value namespace.
//! The shared ownership selection supplies order and exact outgoing coordinates;
//! hook requirements remain obligations at that edge, never new unconditional facts.
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedNormalCleanupAction, CheckedNormalExitCleanupPlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use typed_trees::{TypedTrees, statement::StatementNode};

pub(in crate::execution::terminal_unit) fn retain_planned(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitEffectMachinePlan],
    shapes: &mut ShapeCollector<'_>,
    machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    machines.retain_mut(|plan| {
        for state in &mut plan.states {
            let Some(rows) = state_cleanups(program, facts, targets, shapes, state) else {
                return false;
            };
            state.normal_exit_cleanups = rows;
        }
        true
    });
}

fn state_cleanups(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitEffectMachinePlan],
    shapes: &mut ShapeCollector<'_>,
    state_plan: &CheckedComposedUnitControlStatePlan,
) -> Option<Vec<CheckedNormalExitCleanupPlan>> {
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_plan.state)
            .map(|state| (machine, state))
    })?;
    let flow_state = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, row)| (row.state_symbol == state.symbol).then_some(row))?;
    let mut rows = Vec::<CheckedNormalExitCleanupPlan>::new();
    for outgoing in facts
        .flow
        .control
        .outgoing
        .span_or_empty(flow_state.outgoing)
    {
        let kind = if !outgoing.transition_target.is_valid() {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        } else {
            let StatementNode::Transition(transition) = program
                .statement_table
                .statements(state.statement_nodes)
                .get(outgoing.statement_index)?
            else {
                return None;
            };
            if transition.target == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
            } else if transition.continuation == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            } else {
                return None;
            }
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: state.symbol,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !rows.iter().any(|row| row.exit == exit) {
            rows.push(CheckedNormalExitCleanupPlan {
                exit,
                actions: Vec::new(),
            });
        }
    }
    let binders = machine_binders(program, machine);
    for (_, disposition) in facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| {
            row.machine == machine.symbol
                && row.state == state.symbol
                && row.source == PermissionEventSource::StateExit
        })
    {
        if disposition.exit.state_symbol != state.symbol {
            return None;
        }
        let selected = crate::flow::selected_cleanup_leaves(program, facts, disposition)?;
        let source = owner_source(program, state, state_plan, disposition.root)?;
        let place = crate::flow::CanonicalPlace {
            root: disposition.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(disposition.path)?
                .to_vec(),
        };
        let reference = crate::flow::canonical_place_type_reference(
            program,
            state.symbol,
            disposition.exit.statement_index,
            &place,
        )?;
        let type_identity = shapes.add_type(reference, &binders, &[])?;
        let path = if place.segments.is_empty() {
            Vec::new()
        } else {
            crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                state.symbol,
                disposition.exit.statement_index,
                &place,
            )?
            .1
        };
        let mut moved = Vec::new();
        for removed in facts
            .contextual_cleanup
            .exclusions
            .span(disposition.exclusions)?
        {
            let excluded = crate::flow::CanonicalPlace {
                root: disposition.root,
                segments: facts
                    .contextual_cleanup
                    .segments
                    .span(removed.segments)?
                    .to_vec(),
            };
            let (reference, removed_path) =
                crate::execution::terminal_unit::calls::projected_argument_path(
                    program,
                    state.symbol,
                    disposition.exit.statement_index,
                    &excluded,
                )?;
            moved.push((
                removed_path.strip_prefix(path.as_slice())?.to_vec(),
                shapes.add_type(reference, &binders, &[])?,
            ));
        }
        let mut hooks = Vec::new();
        for leaf in selected {
            let crate::flow::CleanupLeaf::Nominal {
                machine: hook,
                path: selected_path,
            } = leaf
            else {
                return None;
            };
            for segments in concrete_paths(&selected_path)? {
                let selected_place = crate::flow::CanonicalPlace {
                    root: disposition.root,
                    segments,
                };
                let (reference, selected_path) =
                    crate::execution::terminal_unit::calls::projected_argument_path(
                        program,
                        state.symbol,
                        disposition.exit.statement_index,
                        &selected_place,
                    )?;
                let selected_type = shapes.add_type(reference, &binders, &[])?;
                hooks.push(selected_nominal(
                    program,
                    facts,
                    targets,
                    hook,
                    source.clone(),
                    selected_path,
                    selected_type,
                )?);
            }
        }
        // Geometry chooses the live complement, not whether it is cleanup-free.
        // Hook receivers are intact disposition boundaries. Refine only the
        // paths that reach them, preserving maximal ordinary residual subtrees
        // and the same reverse declaration/index order for every action kind.
        let residuals = if moved.is_empty() && hooks.is_empty() {
            vec![CheckedUnitPartialAffineDiscardPlan {
                source: source.clone(),
                path: Vec::new(),
                type_identity: type_identity.clone(),
            }]
        } else {
            let moved = moved
                .iter()
                .map(|(path, identity)| (path.as_slice(), identity.as_str()))
                .collect::<Vec<_>>();
            let boundaries = hooks
                .iter()
                .map(|hook| {
                    Some((
                        hook.path.strip_prefix(path.as_slice())?,
                        hook.type_identity.as_str(),
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            super::residuals::reconstruct_with_boundaries(
                &shapes.types,
                &source,
                &type_identity,
                &moved,
                &boundaries,
                usize::MAX,
            )?
        };
        // The effect traversal groups repeated array leaves by field. Index
        // their exact paths once; only residual geometry chooses action order.
        hooks.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        let mut selected_hooks = 0;
        let actions = residuals
            .into_iter()
            .map(|mut residual| {
                let mut full_path = path.clone();
                full_path.append(&mut residual.path);
                residual.path = full_path;
                if let Ok(position) = hooks.binary_search_by(|hook| hook.path.cmp(&residual.path)) {
                    selected_hooks += 1;
                    CheckedNormalCleanupAction::InvokeNominal(hooks[position].clone())
                } else {
                    CheckedNormalCleanupAction::Discard(residual)
                }
            })
            .collect::<Vec<_>>();
        if selected_hooks != hooks.len() {
            return None;
        }
        if let Some(row) = rows.iter_mut().find(|row| row.exit == disposition.exit) {
            row.actions.extend(actions);
        } else {
            return None;
        }
    }
    Some(rows)
}

/// Effect discovery retains array windows once; executable cleanup needs every
/// concrete selected receiver. Reserve the complete expansion before writing it.
fn concrete_paths(path: &[facts::PlaceSegment]) -> Option<Vec<Vec<facts::PlaceSegment>>> {
    let mut paths = vec![Vec::new()];
    for segment in path {
        if let facts::PlaceSegment::FixedRange { start, end } = *segment {
            let count = paths.len().checked_mul(end.checked_sub(start)?)?;
            let mut expanded = Vec::new();
            expanded.try_reserve_exact(count).ok()?;
            for prefix in paths {
                for index in start..end {
                    let mut selected = prefix.clone();
                    selected.push(facts::PlaceSegment::FixedIndex { index });
                    expanded.push(selected);
                }
            }
            paths = expanded;
        } else {
            for prefix in &mut paths {
                prefix.push(*segment);
            }
        }
    }
    Some(paths)
}

fn owner_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    plan: &CheckedComposedUnitControlStatePlan,
    root: PlaceRoot,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    let PlaceRoot::Symbol(symbol) = root else {
        return None;
    };
    if let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    {
        let index = plan
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        return Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(index).ok()?,
        });
    }
    for operation in &plan.operations {
        if matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralLocal { symbol: local, .. } if *local == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
        }
        if let Some(result) = operation.statement_structural_result_binding()
            && matches!(program.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
                Some(StatementNode::LocalData(local)) if local.symbol == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            });
        }
    }
    None
}

fn selected_nominal(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitEffectMachinePlan],
    hook: symbols::SymbolHandle,
    source: CheckedUnitStructuralArgumentSourcePlan,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    type_identity: String,
) -> Option<CheckedUnitNominalAffineCleanupPlan> {
    let machine = crate::lookup::machine_by_symbol(program, hook)?;
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [receiver] = program.state_parameters(state) else {
        return None;
    };
    let target = targets.iter().find(|target| target.machine == hook)?;
    if !super::nominal_cleanup_target_matches(program, machine, state, target, &type_identity) {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupPlan {
        source,
        path,
        type_identity,
        cleanup_machine: hook,
        cleanup_state: state.symbol,
        cleanup_contract_report_fingerprint: target.contract_report_fingerprint,
        requirements: super::nominal_cleanup_boolean_requirements(
            program, facts, machine, state, receiver,
        )?,
    })
}
