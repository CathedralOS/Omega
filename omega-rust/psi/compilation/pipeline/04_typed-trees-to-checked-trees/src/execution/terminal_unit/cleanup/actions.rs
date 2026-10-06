//! Executable action selection from an already checked contextual disposition.
//! State exits and temporary call continuations share this geometry and hook
//! selection. Storage namespaces differ; cleanup order and prerequisites do not.

use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitEffectMachinePlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use typed_trees::TypedTrees;

pub(super) fn disposition_actions(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitEffectMachinePlan],
    shapes: &mut ShapeCollector<'_>,
    disposition: &checked_trees::ContextualCleanupDisposition,
    source: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let machine = crate::lookup::machine_by_symbol(program, disposition.machine)?;
    let binders = machine_binders(program, machine);
    let statement_index = match disposition.source {
        language_semantics::PermissionEventSource::Call {
            statement_index, ..
        }
        | language_semantics::PermissionEventSource::Statement { statement_index } => {
            statement_index
        }
        _ => disposition.exit.statement_index,
    };
    let selected = crate::flow::selected_cleanup_leaves(program, facts, disposition)?;
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
        disposition.state,
        statement_index,
        &place,
    )?;
    let type_identity = shapes.add_type(reference, &binders, &[])?;
    let path = if place.segments.is_empty() {
        Vec::new()
    } else {
        crate::execution::terminal_unit::calls::projected_argument_path(
            program,
            disposition.state,
            statement_index,
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
                disposition.state,
                statement_index,
                &excluded,
            )?;
        // The contextual row remembers earlier transfers under the original
        // owner. A selected residual ignores siblings outside its subtree;
        // an already removed ancestor cannot authorize another disposition.
        let Some(relative_path) = removed_path.strip_prefix(path.as_slice()) else {
            if path.starts_with(&removed_path) {
                return None;
            }
            continue;
        };
        moved.push((
            relative_path.to_vec(),
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
                    disposition.state,
                    statement_index,
                    &selected_place,
                )?;
            let selected_type = shapes.add_type(reference, &binders, &[])?;
            hooks.push(selected_nominal(
                program,
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
    Some(actions)
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

fn selected_nominal(
    program: &TypedTrees,
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
    })
}
