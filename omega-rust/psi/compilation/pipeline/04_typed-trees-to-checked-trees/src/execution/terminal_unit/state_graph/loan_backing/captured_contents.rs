//! Stable-home lifetime retention from the actual completed carrier. Anonymous
//! assemblies carry their children's ingress rather than inventing local loan
//! rows under a new root. This does not prove relocation independence and is
//! deliberately absent from the retired-carrier query.
use super::{Dependencies, ingress_covers_capture_path, shared_leaf_lifetime_roots};
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowOperandRetention};
use facts::PlaceRoot;

pub(super) fn retained(
    dependencies: &Dependencies<'_>,
    operand: Handle<FlowOperandFact>,
    root: PlaceRoot,
    captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
) -> bool {
    let control = &dependencies.facts.flow.control;
    if !control.operands.is_valid(operand) {
        return false;
    }
    let operand = control.operands.get(operand);
    // A complete captured whole-root address binds this assembly to the
    // resource's actual supplying home. Equal types or lifetime roots alone
    // cannot substitute another carrier or cover its unselected siblings.
    let addresses = control.operand_referents.span_or_empty(operand.referents);
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| address.root != root || !address.segments.is_empty())
    {
        return false;
    }
    let Some(reference) = validation::canonical_place_type_reference(
        dependencies.program,
        dependencies.state,
        dependencies.statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )
    .and_then(|reference| {
        validation::reference_result_custody::input_carrier_type(dependencies.program, reference)
    }) else {
        return false;
    };
    let mut selected = Vec::new();
    if !profile_contents(
        dependencies,
        operand.retention,
        root,
        reference,
        &mut Vec::new(),
        &mut selected,
    ) {
        return false;
    }
    // Failed alternative proofs publish no partial parent roster.
    captured.clear();
    captured.extend(selected);
    true
}

fn profile_contents(
    dependencies: &Dependencies<'_>,
    retention: Handle<FlowOperandRetention>,
    root: PlaceRoot,
    reference: typed_trees::types::TypeReferenceHandle,
    active: &mut Vec<Handle<FlowOperandRetention>>,
    captured: &mut Vec<(PlaceRoot, PlaceRoot)>,
) -> bool {
    let control = &dependencies.facts.flow.control;
    if !control.operand_retentions.is_valid(retention) || active.contains(&retention) {
        return false;
    }
    let profile = control.operand_retentions.get(retention);
    active.push(retention);
    let complete = if !profile.alternatives.is_empty() {
        profile.alternatives.handles().all(|alternative| {
            profile_contents(dependencies, alternative, root, reference, active, captured)
        })
    } else if control.operands.is_valid(profile.stored_value) {
        let value = control.operands.get(profile.stored_value);
        let prefix = control
            .operand_retained_paths
            .span_or_empty(profile.stored_projection);
        validation::project_type_reference_from_segments(
            dependencies.program,
            value.type_reference,
            prefix,
        )
        .is_some_and(|actual| {
            validation::storage_origin_types_match(dependencies.program, actual, reference)
        }) && validation::borrow_contracts::borrow_carrying_owner_paths(
            dependencies.program,
            reference,
        )
        .is_some_and(|paths| {
            paths.iter().all(|path| {
                let mut selected = prefix.to_vec();
                selected.extend(crate::borrow::owner_place_segments(path));
                if ingress_covers_capture_path(
                    dependencies.program,
                    dependencies.facts,
                    profile.stored_value,
                    &selected,
                    dependencies.ingress,
                ) {
                    return true;
                }
                let owner_path =
                    crate::borrow::owner_path_from_place_segments(dependencies.program, &selected);
                let Some(roots) = shared_leaf_lifetime_roots(
                    dependencies.program,
                    dependencies.facts,
                    value,
                    &owner_path,
                ) else {
                    return false;
                };
                roots.into_iter().all(|parent| {
                    dependencies.root(parent, value.constraints, &mut vec![root], captured)
                })
            })
        })
    } else {
        false
    };
    active.pop();
    complete
}
