//! Project the shared frame engine's local storage origins into flow places.
//!
//! Alias transfer belongs to validation. This adapter resolves its canonical
//! names to existing typed symbols. Complete runtime-selected paths retain
//! their descendant fields for overlap, but do not supply exact identity.
//! The inventory includes unknown reference sources. Demand a source only
//! after selecting overlapping local storage; unrelated unknown leaves cannot
//! erase a known effect. Alias invalidation conservatively retires unknown
//! leaves on every write, without assigning them a referent. An exact-origin
//! query must not discard unknown alternatives.

use validation::CanonicalPlace;
use validation::canonical_place_from_expression_in_state;
use validation::push_field_place_segments;
use validation::resolve_member_symbol_from_type_symbol;
use validation::symbol_type_symbol;

use crate::flow::canonical_place_from_symbol;
use crate::flow::canonical_place_segments_equal;
use crate::flow::canonical_place_segments_may_overlap;
use crate::flow::mutation::normalize_write_only_range_place;

use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;
mod coordinates;
pub(crate) use coordinates::{OriginPrecision, origin_place, sole_local_referent_origin};

pub(crate) fn close_storage_places_over_aliases(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    places: Vec<CanonicalPlace>,
    resolver: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    if places.is_empty() {
        return Some(places);
    }
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let resolver = resolver?;
    let origins = resolver.local_write_origins_before_statement(machine, statement)?;
    close_storage_places_over_origins(program, state, statement_index, places, &origins)
}

/// Project physical writes into the names under which content facts live.
/// The caller supplies the inventory at its observation frontier; a binding
/// without preservation evidence is an unknown alias, never a disjoint one.
pub(crate) fn close_storage_places_over_origins(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    mut places: Vec<CanonicalPlace>,
    origins: &[validation::LocalWriteOrigin],
) -> Option<Vec<CanonicalPlace>> {
    let storage = places.clone();
    let mut seen: std::collections::HashSet<CanonicalPlace> = storage.iter().cloned().collect();
    for origin in origins {
        // Image backing is distinct from unknown declaration storage. Its
        // descriptor may be replaced (already in the physical footprint),
        // but a declaration write cannot mutate the literal's backing bytes.
        if origin
            .source
            .as_ref()
            .is_some_and(|source| source.is_image_storage)
        {
            continue;
        }
        let mut alias = canonical_place_from_symbol(origin.local_symbol)?;
        alias.segments.extend_from_slice(&origin.local_segments);
        if origin.source.is_none() {
            // This leaf can refer to any written storage. Retire its entire
            // local footprint rather than dropping all alias expansion and
            // losing even the current writer's known access route. This is
            // may-write invalidation, never evidence of a particular referent.
            if seen.insert(alias.clone()) {
                places.push(alias);
            }
            continue;
        }
        let (source, precision) = origin_place(program, state, statement_index, origin)?;
        for place in &storage {
            if source.root != place.root {
                continue;
            }
            let mut alias = alias.clone();
            if !canonical_place_segments_may_overlap(program, &place.segments, &source.segments) {
                continue;
            }
            if place.segments.len() >= source.segments.len()
                && precision != OriginPrecision::Enclosing
            {
                alias.segments.extend(coordinates::alias_projection(
                    &source.segments,
                    &place.segments,
                )?);
            }
            if seen.insert(alias.clone()) {
                places.push(alias);
            }
        }
    }
    Some(places)
}

pub(super) fn assignment_storage_places(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let assignment = statement.as_assignment()?;
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    let mut projection = assignment.target;
    loop {
        projection = match program.expression_table.expression(projection) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed)
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
            {
                indexed.collection
            }
            ExpressionNode::Indexed(_)
                if !validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    projection,
                ) =>
            {
                return None;
            }
            _ => break,
        };
    }
    let mut direct = canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        assignment.target,
    );
    if let Some(place) = &mut direct {
        normalize_write_only_range_place(program, state_symbol, place);
        if !place_requires_local_write_origin(
            program,
            state_symbol,
            statement_index,
            place,
            call_frames,
        ) {
            return direct.map(|place| vec![place]);
        }
    }
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let target = resolver.assignment_write_target(machine, statement);
    let Some(target) = target else {
        // The resolver cannot classify a write through a reference local it
        // has no origin for; a local of finite candidate origins still names
        // exactly those places (`rebase_local_write_places`), anything else
        // stays unknown.
        return rebase_local_write_places(
            program,
            state_symbol,
            statement_index,
            direct?,
            call_frames,
        );
    };
    match target {
        validation::AssignmentWriteTarget::LocalBindingReplacement { .. } => {
            direct.map(|place| vec![place])
        }
        validation::AssignmentWriteTarget::Storage { paths } => {
            if let Some(place) = direct {
                rebase_local_write_places(
                    program,
                    state_symbol,
                    statement_index,
                    place,
                    call_frames,
                )
            } else {
                paths
                    .iter()
                    .map(|path| place_from_origin_path(program, state, statement_index, path))
                    .collect()
            }
        }
    }
}

fn place_requires_local_write_origin(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return false;
    };
    let Some((_, local)) = program
        .state_local_declaration(state, root)
        .filter(|&(ordinal, _)| ordinal < statement_index)
    else {
        return false;
    };
    let mut owned_frames = None;
    crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
        .is_none_or(|resolver| resolver.local_requires_write_origin(local.type_reference))
}

/// Equality evidence needs one complete origin, not the conservative union
/// used to retire facts after a possible write.
pub(crate) fn rebase_exact_local_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CanonicalPlace> {
    if !place_requires_local_write_origin(
        program,
        state_symbol,
        statement_index,
        &place,
        call_frames,
    ) {
        return Some(place);
    }
    let root = place.root.as_symbol()?;
    let (machine, state) = validation::find_state(program, state_symbol)?;
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let origins = resolver.local_write_origins_before_statement(machine, statement)?;
    let mut candidates = origins.iter().filter(|origin| {
        origin.local_symbol == root
            && canonical_place_segments_may_overlap(
                program,
                &place.segments,
                &origin.local_segments,
            )
    });
    let origin = candidates.next()?;
    if candidates.next().is_some()
        || place.segments.len() < origin.local_segments.len()
        || !place
            .segments
            .iter()
            .zip(&origin.local_segments)
            .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
    {
        return None;
    }
    let canonical = coordinates::exact_origin_place(program, state, statement_index, origin)?;
    // The origin may be a captured view window. Its first child projection
    // uses that window's coordinates, while subsequent array dimensions
    // belong to the selected value. Share the captured-operand address join
    // so returned-reference candidates and call handback name actual storage.
    crate::flow::project_captured_view_place(
        canonical,
        &place.segments[origin.local_segments.len()..],
    )
}

pub(super) fn rebase_local_write_places(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    if !place_requires_local_write_origin(
        program,
        state_symbol,
        statement_index,
        &place,
        call_frames,
    ) {
        return Some(vec![place]);
    }
    let root = place.root.as_symbol()?;
    let (machine, state) = validation::find_state(program, state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let Some(origins) =
        resolver.local_write_origins_before_statement(machine, statements.get(statement_index)?)
    else {
        return reference_result_candidate_places(
            program,
            state_symbol,
            statement_index,
            root,
            &place,
            call_frames,
        );
    };
    // An origin identifies the captured referent of one descriptor, not the
    // transitive storage footprint of that referent. Walk the same immutable
    // inventory through every writable descriptor before deciding a local
    // holder is disjoint from caller storage. Do not flatten this inventory:
    // exact-reference consumers still need the original descriptor coordinates.
    let mut pending = vec![(place, Vec::<usize>::new())];
    let mut projected = Vec::new();
    while let Some((place, crossed)) = pending.pop() {
        let root = place.root.as_symbol()?;
        let mut retains_private_storage = true;
        for (origin_index, origin) in origins.iter().enumerate() {
            if origin.local_symbol != root
                || !canonical_place_segments_may_overlap(
                    program,
                    &place.segments,
                    &origin.local_segments,
                )
            {
                continue;
            }
            let descriptor = CanonicalPlace {
                root: place.root,
                segments: origin.local_segments.clone(),
            };
            let reference = validation::canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &descriptor,
            )?;
            if validation::declared_reference_access(program, reference)
                .is_some_and(|access| !access.is_exclusive())
                || origin
                    .source
                    .as_ref()
                    .is_some_and(|source| source.is_image_storage)
            {
                continue;
            }
            let (mut canonical, precision) = origin_place(program, state, statement_index, origin)?;
            if place.segments.len() >= origin.local_segments.len() {
                retains_private_storage = false;
                if precision != OriginPrecision::Enclosing {
                    canonical.segments = facts::project_captured_view_segments(
                        &canonical.segments,
                        &place.segments[origin.local_segments.len()..],
                    )?;
                }
            }
            // A captured self-origin already names its storage. Other cycles
            // cannot establish a finite footprint, including cycles which
            // grow a selector suffix on each crossing. Refuse rather than
            // returning an incomplete may-write set or looping indefinitely.
            if canonical == place {
                // Another possible origin may fully cover this descriptor.
                // Its replacement cannot erase the retained incoming target.
                if !projected.contains(&place) {
                    projected.push(place.clone());
                }
                continue;
            }
            if crossed.contains(&origin_index) {
                return None;
            }
            let mut next_crossed = crossed.clone();
            next_crossed.push(origin_index);
            pending.push((canonical, next_crossed));
        }
        // A disjoint owned field, or an ancestor also containing owned bytes,
        // still has local facts to invalidate. Caller summaries filter it only
        // after the reachable external storage has been collected.
        if retains_private_storage && !projected.contains(&place) {
            projected.push(place);
        }
    }
    Some(projected)
}

/// A reference local bound from a checked reference result names one of
/// finitely many candidate origins (flow/reference_places); a write through
/// it lands on every candidate's corresponding place.
fn reference_result_candidate_places(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    root: SymbolHandle,
    place: &CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let candidates = crate::flow::reference_result_candidates_before_statement(
        program,
        state_symbol,
        statement_index,
        root,
        call_frames,
    )?;
    Some(
        candidates
            .into_iter()
            .map(|mut candidate| {
                candidate.segments.extend_from_slice(&place.segments);
                candidate
            })
            .collect(),
    )
}

pub(crate) fn place_from_origin_path(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    path: &str,
) -> Option<CanonicalPlace> {
    let mut members = path.split('.');
    let root_name = members.next()?;
    let root = program
        .state_parameters(state)
        .iter()
        .find_map(|parameter| (parameter.name.as_str() == root_name).then_some(parameter.symbol))
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .find_map(|statement| {
                    let local = statement.as_local_data()?;
                    (local.name.as_str() == root_name).then_some(local.symbol)
                })
        })?;
    let mut place = canonical_place_from_symbol(root)?;
    let mut symbol = root;
    for member in members {
        let type_symbol = symbol_type_symbol(program, symbol)?;
        symbol = resolve_member_symbol_from_type_symbol(program, type_symbol, member)?;
        push_field_place_segments(program, &mut place.segments, symbol);
    }
    Some(place)
}
