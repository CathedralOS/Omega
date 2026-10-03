//! Assigned carriers transfer their exact origins into the binding's stored
//! rows, the same whole-subtree ownership an instantiated move relies on.
//! Exact subtrees replace their prior rows. Indexed storage writes retain the
//! collection footprint and forget overlapping reference identities: a coarse
//! write cannot establish which element was replaced. Neither operation grants
//! access permission, and later reference demands still need proven origins.

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

/// Ordinary and cyclic effect walks use the same assignment-origin transfer.
/// No replacement means the existing frontier survives; a failed required
/// replacement must make the effect walk opaque.
#[allow(clippy::too_many_arguments)]
pub(in crate::machine_calls::calls::write_frames) fn assignment_origins(
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
    replaces_alias: bool,
) -> Option<Option<StoredLocalOrigins>> {
    let replaces_stored = (!replaces_alias
        && (super::assignment_replaces_case_binding(program, assignment, stored, aliases)
            || (include_shared
                && super::assignment_replaces_reference_ancestor(
                    program, assignment, stored, aliases,
                ))))
        || super::super::alias_bindings::assignment_replaces_untracked_reference(
            program, machine, state, assignment, aliases,
        );
    if !replaces_stored {
        return Some(None);
    }
    assigned_stored_origins(
        program,
        machine,
        state,
        statement,
        assignment,
        aliases,
        stored,
        symbols,
        inference,
        include_shared,
    )
    .map(Some)
}

/// The complete stored record for the assignment target's root binding.
/// Returns the spliced record so the caller can expand the write through the
/// pre-state before installing the post-state. Any failure leaves the walk
/// opaque rather than keeping stale declared rows beside observed ones.
#[allow(clippy::too_many_arguments)]
fn assigned_stored_origins(
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
    // alias paths. A member path instead writes inside its root's storage:
    // an exclusive stored leaf's slot is replaced in place, while a shared
    // leaf's rebind retires only that leaf's frozen origin — its spliced
    // provenance is the unknown marker, never the value's claimed subject —
    // and the rest of the record survives instead of going opaque.
    let shared_leaf_rebind = super::super::type_reference_is_reference(program, target_type)
        && !source.segments.is_empty()
        && !super::super::type_may_carry_write(program, target_type);
    if super::super::type_reference_is_reference(program, target_type) && source.segments.is_empty()
    {
        return None;
    }
    if source.segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
                | PlaceSegment::Index { .. }
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
    if let Some(collection_depth) = source.segments.iter().position(|segment| {
        matches!(
            segment,
            PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
        )
    }) {
        let root_type = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == source.root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|statement| match statement {
                        StatementNode::LocalData(local) if local.symbol == source.root => {
                            Some(local.type_reference)
                        }
                        _ => None,
                    })
            })?;
        let storage_type = reference_subjects::carrier_storage_type(program, root_type)?;
        let inventory = super::declared_origins_for_query(
            program,
            source.root,
            program.symbols.name(source.root),
            storage_type,
            include_shared,
        )?;
        let mut replaced = established.cloned().unwrap_or_else(|| inventory.clone());
        // Frame paths deliberately forget element identity. Forget evidence for
        // every element the same path may name. The full type inventory is
        // necessary: the new value may introduce a reference-bearing case
        // absent from the overwritten value's selected-case rows.
        let collection = &source.segments[..collection_depth];
        replaced
            .references
            .retain(|leaf| !prefix_matches(collection, &leaf.local_segments));
        for mut leaf in inventory
            .references
            .into_iter()
            .filter(|leaf| prefix_matches(collection, &leaf.local_segments))
        {
            leaf.origin = None;
            replaced.references.push(leaf);
        }
        replaced
            .cases
            .retain(|case| !prefix_matches(collection, case));
        replaced.moves.retain(|moved| {
            !prefix_matches(collection, &moved.local_segments)
                && !prefix_matches(&moved.local_segments, collection)
        });
        replaced.symbolic = false;
        return Some(replaced);
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
            if shared_leaf_rebind {
                reference_subjects::unknown_readonly_origin(program, reference, "")
                    .map(|origin| vec![origin])
            } else if include_shared {
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

#[cfg(test)]
mod tests {
    #[test]
    fn indexed_record_replacement_retains_its_storage_write_frame() {
        let program = crate::front_end::typed_program(
            r#"
            domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
            boundary data Authority;
            data CellId { case Empty; case A1; case A2; }
            data Exit { command: &[u8] in Utf8; destination: CellId; }
            data Room { cell: CellId; label: &[u8] in Utf8; description: &[u8] in Utf8; exits: [Exit; 2]; }
            data Main { authority: Authority; room: Room; }
            machine print(value: &[u8] in Utf8) {}
            machine fill(context: &mut Main) -> i32 {
                context.room.cell = CellId::A1;
                context.room.label = "A1";
                context.room.description = "A1: a stone entry.";
                context.room.exits[0] = Exit { command: "north", destination: CellId::A2 };
                context.room.exits[1] = Exit { command: "wait", destination: CellId::Empty };
                print(context.room.label);
                print(context.room.description);
                print(context.room.exits[0].command);
                0
            }
            "#,
        );
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "fill")
            .expect("fill machine");
        let resolver = crate::CallFrameResolver::new(&program).expect("frame resolver");
        let frame =
            resolver.inferred_state_write_frame(machine, &program.machine_states(machine)[0]);
        assert_eq!(
            frame
                .complete_paths()
                .map(|paths| paths.iter().map(String::as_str).collect::<Vec<_>>()),
            Some(vec![
                "$P0.room.cell",
                "$P0.room.description",
                "$P0.room.exits",
                "$P0.room.label"
            ]),
            "indexed record writes must retain their exact conservative frame: {frame:?}",
        );
    }
}
