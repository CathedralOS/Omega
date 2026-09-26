//! Assigned carriers transfer their exact origins into the binding's stored
//! rows, the same whole-subtree ownership an instantiated move relies on.
//! The target must resolve to one established local or parameter root, or to
//! a plain field path beneath it; an index or case selector cannot retire
//! sibling evidence. A reference-valued target is a rebinding, not carrier
//! replacement, and stays with the alias paths.

use super::super::path_instantiation::aggregate_arguments::reference_leaves_with_origins;
use super::super::place_paths::append_place_suffix;
use super::super::reference_origins::exclusive_reference_origins;
use super::super::reference_subjects;
use super::super::{FrameInference, FramePlaceOrigin, Machine, TopLevelSymbols, TypedTrees};
use super::frozen_bindings::binding_source;
use super::projections::{prefix_matches, reference_leaves_before_statement_for_query};
use super::reference_values::canonical_reference_origins;
use super::{StoredLocalOrigins, StoredWriteOrigin};
use facts::PlaceSegment;
use symbols::SymbolKind;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment};

/// The complete stored record for the assignment target's root binding.
/// Returns the spliced record so the caller can expand the write through the
/// pre-state before installing the post-state. Any failure leaves the walk
/// opaque rather than keeping stale declared rows beside observed ones.
#[allow(clippy::too_many_arguments)]
pub(in crate::machine_calls::calls::write_frames) fn assigned_stored_origins(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    assignment: &TableAssignment,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<StoredLocalOrigins> {
    let source = binding_source(program, assignment.target, aliases)?;
    if !source.root.is_valid() {
        return None;
    }
    let declaration = program.symbols.get(source.root);
    if declaration.parent != state.symbol
        || !matches!(declaration.kind, SymbolKind::Local | SymbolKind::Parameter)
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == source.root && parameter.is_self)
        || source.root == machine.symbol
    {
        return None;
    }
    let target_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.target,
    )?;
    // A reference-typed bare name is a local rebinding that stays with the
    // alias paths; a member path replaces an exclusive stored leaf's slot
    // in place, while a shared leaf rebind retires its record unproven.
    if super::super::type_reference_is_reference(program, target_type)
        && (source.segments.is_empty() || !super::super::type_may_carry_write(program, target_type))
    {
        return None;
    }
    if source.segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
        )
    }) {
        return None;
    }
    let established = stored
        .iter()
        .find(|local| local.local_symbol == source.root);
    // A move row owns its whole subtree. A replacement strictly inside that
    // subtree cannot split the transport it describes.
    if established.is_some_and(|established| {
        established.moves.iter().any(|moved| {
            moved.local_segments.len() < source.segments.len()
                && prefix_matches(&moved.local_segments, &source.segments)
        })
    }) {
        return None;
    }
    let leaves = reference_leaves_with_origins(
        program,
        machine,
        assignment.value,
        target_type,
        "",
        symbols,
        inference,
        include_shared,
        &|expression, reference, implicit_borrow, inference| {
            if include_shared {
                reference_subjects::value_origin(
                    program,
                    machine,
                    expression,
                    symbols,
                    inference,
                    aliases,
                    stored,
                    implicit_borrow,
                )
                .or_else(|| reference_subjects::unknown_readonly_origin(program, reference, ""))
                .map(|origin| vec![origin])
            } else {
                exclusive_reference_origins(program, machine, expression, symbols, inference)
            }
        },
        &|expression, reference, _| {
            reference_leaves_before_statement_for_query(
                program,
                state,
                statement,
                expression,
                reference,
                Some(stored),
                None,
                include_shared,
            )
        },
    )?;
    let mut replaced = established.cloned().unwrap_or_else(|| StoredLocalOrigins {
        local_symbol: source.root,
        references: Vec::new(),
        cases: Vec::new(),
        moves: Vec::new(),
        symbolic: false,
    });
    // The replacement owns its destination subtree. Declared alternatives
    // beneath it describe the overwritten value, never the new one.
    replaced
        .references
        .retain(|leaf| !prefix_matches(&source.segments, &leaf.local_segments));
    replaced
        .cases
        .retain(|case| !prefix_matches(&source.segments, case));
    replaced
        .moves
        .retain(|moved| !prefix_matches(&source.segments, &moved.local_segments));
    replaced.symbolic = false;
    let mut target_path = program.symbols.name(source.root).to_owned();
    for segment in &source.segments {
        // Selected case arms own no name segment in the local spelling; only
        // fields contribute, matching the declared leaf-path convention.
        if let PlaceSegment::Field { symbol } = segment {
            target_path.push('.');
            target_path.push_str(program.symbols.name(*symbol));
        }
    }
    for leaf in leaves.references {
        for origin in canonical_reference_origins(program, &leaf.origin, aliases, stored) {
            let mut local_segments = source.segments.clone();
            local_segments.extend(leaf.local_segments.iter().copied());
            replaced.references.push(StoredWriteOrigin {
                local_symbol: source.root,
                local_path: append_place_suffix(&target_path, &leaf.local_suffix),
                local_segments,
                origin,
            });
        }
    }
    for case in leaves.cases {
        let mut selected = source.segments.clone();
        selected.extend(case);
        replaced.cases.push(selected);
    }
    for mut moved in leaves.moves {
        let mut segments = source.segments.clone();
        segments.extend(moved.local_segments);
        moved.local_segments = segments;
        replaced.moves.push(moved);
    }
    Some(replaced)
}
