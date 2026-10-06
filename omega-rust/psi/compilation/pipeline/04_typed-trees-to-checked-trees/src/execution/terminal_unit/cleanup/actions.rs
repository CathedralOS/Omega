//! Executable action selection from an already checked contextual disposition.
//! State exits and temporary call continuations share this geometry and hook
//! selection. Storage namespaces differ; cleanup order and prerequisites do not.
//! A receiver's nominal hook precedes its structural children; nested hook
//! paths are ordered obligations, not overlapping attempts to consume a root.

use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallableEntry,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use typed_trees::TypedTrees;

pub(super) fn disposition_actions(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    disposition: &checked_trees::ContextualCleanupDisposition,
    source: CheckedUnitStructuralArgumentSourcePlan,
    place_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
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
    let reference = if disposition.disposed_type.is_valid() {
        disposition.disposed_type
    } else {
        crate::flow::canonical_place_type_reference(
            program,
            disposition.state,
            statement_index,
            &place,
        )?
    };
    let type_identity = shapes.add_type(reference, &binders, &[])?;
    // A caller that already holds the dying place in its own operand
    // namespace supplies it: a runtime-selected element has no static
    // argument path, but its store names the same place.
    let path = if let Some(place_path) = place_path {
        place_path.to_vec()
    } else if place.segments.is_empty() {
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
        for segments in crate::flow::concrete_cleanup_paths(&selected_path)? {
            let selected_place = crate::flow::CanonicalPlace {
                root: disposition.root,
                segments,
            };
            let (mut reference, selected_path) =
                crate::execution::terminal_unit::calls::projected_argument_path(
                    program,
                    disposition.state,
                    statement_index,
                    &selected_place,
                )?;
            if disposition.disposed_type.is_valid() && selected_place.segments == place.segments {
                reference = disposition.disposed_type;
            }
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
    // Hook receivers are intact disposition boundaries. A parent's hook is
    // followed by structural children, not substituted for them. Refine only
    // the paths that reach hooks, preserving maximal ordinary residual
    // subtrees and the same reverse declaration/index order for every kind.
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

fn selected_nominal(
    program: &TypedTrees,
    targets: &[CheckedUnitCallableEntry<'_>],
    hook: symbols::SymbolHandle,
    source: CheckedUnitStructuralArgumentSourcePlan,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    type_identity: String,
) -> Option<CheckedUnitNominalAffineCleanupPlan> {
    let machine = crate::lookup::machine_by_symbol(program, hook)?;
    let state = program.machine_states(machine).first()?;
    let target = super::callable_target(targets, hook)?;
    if !super::nominal_cleanup_target_matches(program, machine, state, &target, &type_identity) {
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
