//! Original reference subjects outlive executable state bindings, not their
//! contents. Qualifiers still pass through the ordinary all-incoming meet and
//! mutation invalidation. Project observation writes into this same namespace;
//! otherwise a renamed writer could leave an obsolete entry claim alive.
use super::{entry_storage_origins, reference_type, root_storage, stable_parameter};
use crate::flow::FlowBuildContext;
use facts::PlaceRoot;
use symbols::SymbolHandle;
use validation::CanonicalPlace;

/// An owned aggregate is not necessarily private storage: a selected child
/// can cross one of its stored references. Only a reference-free selected
/// value reached without any reference crossing is disjoint from invocation
/// referents. Unknown/opaque carriers cannot supply that proof.
fn private_owned_projection(
    program: &dyn validation::CheckingInput,
    build: &FlowBuildContext,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
) -> bool {
    let Some(frames) = build.call_frames else {
        return false;
    };
    let root = CanonicalPlace {
        root: place.root,
        segments: Vec::new(),
    };
    let Some(root_type) =
        validation::canonical_place_type_reference(program, state.symbol, statement_index, &root)
    else {
        return false;
    };
    let mut selected_type = root_type;
    for length in 0..=place.segments.len() {
        let Some(reference) = validation::project_type_reference_from_segments(
            program,
            root_type,
            &place.segments[..length],
        ) else {
            return false;
        };
        if validation::reference_shell(program, reference).is_some() {
            return false;
        }
        selected_type = reference;
    }
    frames.proof_value_is_caller_isolated(selected_type)
}

pub(in crate::flow) fn invocation_referent(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    parameter: SymbolHandle,
) -> Option<PlaceRoot> {
    let entry = program.machine_states(machine).first()?;
    let parameter = program.state_parameter_by_symbol(entry, parameter)?;
    reference_type(program, parameter.type_reference).then_some(PlaceRoot::InvocationReferent {
        parameter: parameter.symbol,
        type_reference: parameter.type_reference,
    })
}

/// Classify the address space, not the current value, of a transported write.
/// Every incoming route must retain the same authenticated content formation
/// and exact owner. Unknown/mixed routes keep the caller's conservative write.
pub(in crate::flow) fn nonresident_content_region(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<crate::flow::mutation::StorageRegion> {
    let place = crate::flow::rebase_exact_local_place(
        program,
        state.symbol,
        statement_index,
        place.clone(),
        build.call_frames,
    )?;
    let root = place.root.as_symbol()?;
    if program
        .state_parameter_by_symbol(state, root)
        .is_some_and(|parameter| {
            reference_type(program, parameter.type_reference)
                && !stable_parameter(program, machine, state, root)
        })
    {
        return None;
    }
    let solved = build.storage_origins_at(machine.symbol)?;
    let mut source = root_storage(program, root);
    source.segments = place.segments;
    let sources = entry_storage_origins(program, machine, state, &solved, &source)?;
    let mut common = None;
    for source in sources {
        let first = source.recasts.first()?;
        let validation::ReferenceViewFormation::ExtentView(declaration) = first.formation else {
            return None;
        };
        if first.representation.is_some()
            || first.root != source.root
            || first
                .segments
                .iter()
                .any(|segment| !crate::flow::place_segment_has_static_identity(segment))
            || typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(
                program,
                declaration.machine(),
            )
            .ok()??
                != declaration
            || declaration.operation()
                != typed_trees::typed_trees::PlacementLifecycleOperation::View
        {
            return None;
        }
        let owner = CanonicalPlace {
            root: invocation_referent(program, machine, first.root)
                .unwrap_or(facts::PlaceRoot::Symbol(first.root)),
            segments: first.segments.clone(),
        };
        let region = crate::flow::mutation::StorageRegion::NonresidentContent {
            owner,
            formation: declaration,
        };
        if common.as_ref().is_some_and(|common| common != &region) {
            return None;
        }
        common = Some(region);
    }
    common
}

/// Known empty means private storage; unknown cannot prove disjointness. A
/// recast's logical selectors are not declaration fields, so retain only its
/// enclosing owner for may-write observations and decline it for exact proof.
pub(in crate::flow) fn project_invocation_place(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
    exact: bool,
) -> Option<Vec<CanonicalPlace>> {
    if matches!(place.root, PlaceRoot::InvocationReferent { .. }) {
        return Some(vec![place.clone()]);
    }
    let current_place = place;
    let place = crate::flow::rebase_exact_local_place(
        program,
        state.symbol,
        statement_index,
        place.clone(),
        build.call_frames,
    )?;
    let Some(root) = place.root.as_symbol() else {
        return None;
    };
    if program
        .state_parameter_by_symbol(state, root)
        .is_some_and(|parameter| {
            reference_type(program, parameter.type_reference)
                && !stable_parameter(program, machine, state, root)
        })
    {
        return None;
    }
    let solved = build.storage_origins_at(machine.symbol)?;
    let mut source = root_storage(program, root);
    source.segments = place.segments;
    let sources = entry_storage_origins(program, machine, state, &solved, &source)?;
    if exact && (sources.is_empty() || sources.iter().any(|source| source != &sources[0])) {
        return None;
    }
    let mut projected = Vec::new();
    for source in sources {
        let Some(root) = invocation_referent(program, machine, source.root) else {
            if !private_owned_projection(program, build, state, statement_index, current_place) {
                return None;
            }
            continue;
        };
        let coarse = source.collection_coarse
            || !source.recasts.is_empty()
            || source.representation.is_some();
        if exact && coarse {
            return None;
        }
        let place = CanonicalPlace {
            root,
            segments: if coarse { Vec::new() } else { source.segments },
        };
        if !projected.contains(&place) {
            projected.push(place);
        }
    }
    Some(projected)
}

/// Observation aliases only: never publish these as physical descriptor
/// stores or as executable reference origins. Unknown writes retire every
/// original reference subject without inventing a particular alias.
pub(in crate::flow) fn extend_invocation_observations(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    places: &mut Vec<CanonicalPlace>,
) {
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    if !program
        .state_parameters(entry)
        .iter()
        .any(|parameter| invocation_referent(program, machine, parameter.symbol).is_some())
    {
        return;
    }
    let mut projected = Vec::new();
    for place in places.iter() {
        match project_invocation_place(
            program,
            build,
            machine,
            state,
            statement_index,
            place,
            false,
        ) {
            Some(incoming) => {
                // Many physical field writes can observe the same enclosing
                // invocation region. Collapse that projection before searching
                // the original footprint: appending every duplicate here made
                // an aggregate store perform one full footprint scan per leaf.
                for place in incoming {
                    if !projected.contains(&place) {
                        projected.push(place);
                    }
                }
            }
            None => {
                // Projection failed, so the write may still reach an entry
                // referent. A resolved field path reaches that field, not every
                // sibling: replacing the whole referent is the unresolved case.
                let field_path = !place.segments.is_empty()
                    && place.segments.iter().all(|segment| match segment {
                        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                        facts::PlaceSegment::Case { variant } => variant.is_valid(),
                        facts::PlaceSegment::FixedIndex { .. } => true,
                        _ => false,
                    });
                if let Some(entry) = program.machine_states(machine).first() {
                    for parameter in program.state_parameters(entry) {
                        if let Some(root) = invocation_referent(program, machine, parameter.symbol)
                        {
                            let projected_place = CanonicalPlace {
                                root,
                                segments: if field_path {
                                    place.segments.clone()
                                } else {
                                    Vec::new()
                                },
                            };
                            if !projected.contains(&projected_place) {
                                projected.push(projected_place);
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    for place in projected {
        if !places.contains(&place) {
            places.push(place);
        }
    }
}
