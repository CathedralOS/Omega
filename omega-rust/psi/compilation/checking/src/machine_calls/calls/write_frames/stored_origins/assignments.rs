//! Assigned carriers transfer their exact origins into the binding's stored
//! rows, the same whole-subtree ownership an instantiated move relies on.
//! Exact field and fixed-element subtrees replace their prior rows. Slice
//! inventory remains a default beside concrete replaced elements; a
//! may-overlap relation cannot erase the untouched remainder of that inventory.
//! Dynamic indexed writes retain old contents and add one possible residence
//! for each replacement leaf: an unknown selector cannot identify a killed
//! loan, but does not make the replacement backing unknown. Coarse write footprints do not
//! erase independently established structural slot coordinates. Neither operation grants
//! access permission, and later reference demands still need proven origins.

use super::super::path_instantiation::aggregate_arguments::reference_leaves;
use super::super::place_paths::{FramePathPrecision, append_place_suffix, coordinate_storage_path};
use super::super::reference_origins::exclusive_reference_origins;
use super::super::reference_subjects;
use super::super::{FrameInference, FramePlaceOrigin, Machine, TopLevelSymbols};
use super::frozen_bindings::binding_source;
use super::projections::{
    path_covers, prefix_matches, reference_leaves_before_statement_for_query,
};
use super::reference_values::canonical_reference_origins;
use super::{StoredLocalOrigins, StoredWriteOrigin};
use facts::PlaceSegment;
use symbols::SymbolKind;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment};

/// Captured replacement of one storage subtree. Unselected siblings belong to
/// the inventory at installation, not the inventory before RHS evaluation.
pub(in crate::machine_calls::calls::write_frames) struct AssignmentOrigins {
    local: StoredLocalOrigins,
    segments: Vec<PlaceSegment>,
    may_select: bool,
    /// An untracked indexed carrier still has declared sibling inventory.
    /// Missing runtime origins cannot turn those siblings into absent leaves.
    declared_inventory: Option<StoredLocalOrigins>,
}

impl AssignmentOrigins {
    pub(in crate::machine_calls::calls::write_frames) fn install(
        self,
        inference: &mut FrameInference,
        stored: &mut Vec<StoredLocalOrigins>,
    ) {
        let mut replacement = self.local;
        if let Some(previous) = stored
            .iter()
            .find(|local| local.local_symbol == replacement.local_symbol)
            .or(self.declared_inventory.as_ref())
        {
            replacement.references.extend(
                previous
                    .references
                    .iter()
                    .filter(|leaf| {
                        self.may_select || !path_covers(&self.segments, &leaf.local_segments)
                    })
                    .map(|leaf| {
                        let mut retained = leaf.clone();
                        if self.may_select
                            && self.segments.len() < leaf.local_segments.len()
                            && prefix_matches(&self.segments, &leaf.local_segments)
                        {
                            // The old descendant was reached through the old
                            // pointer. It cannot identify contents of a newly
                            // installed pointer on the selected alternative.
                            retained.origin = None;
                        }
                        retained
                    }),
            );
            replacement.cases.extend(
                previous
                    .cases
                    .iter()
                    .filter(|case| self.may_select || !path_covers(&self.segments, case))
                    .cloned(),
            );
            replacement.moves.extend(
                previous
                    .moves
                    .iter()
                    .filter(|moved| {
                        !path_covers(&self.segments, &moved.local_segments)
                            && !(self.may_select
                                && prefix_matches(&moved.local_segments, &self.segments))
                    })
                    .cloned(),
            );
        }
        super::record_replacement_local(inference, stored, replacement);
    }
}

/// Ordinary and cyclic effect walks use the same assignment-origin transfer.
/// No replacement means the existing frontier survives; a failed required
/// replacement must make the effect walk opaque.
#[allow(clippy::too_many_arguments)]
pub(in crate::machine_calls::calls::write_frames) fn assignment_origins(
    program: &dyn crate::CheckingInput,
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
) -> Option<Option<AssignmentOrigins>> {
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

/// Capture the destination subtree and its new origins against the entry
/// inventory. Installation splices only that subtree after RHS effects, so an
/// RHS write to a sibling survives and cannot redirect the captured store.
#[allow(clippy::too_many_arguments)]
fn assigned_stored_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    assignment: &TableAssignment,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<AssignmentOrigins> {
    let mut source = binding_source(program, assignment.target, aliases)?;
    if !source.root.is_valid() || !source.builtin_coordinates {
        return None;
    }
    let declaration = program.symbols.get(source.root);
    if (declaration.parent != state.symbol
        || !matches!(declaration.kind, SymbolKind::Local | SymbolKind::Parameter))
        || source.root == machine.symbol
    {
        return None;
    }
    let target_type =
        reference_subjects::assignment_stored_value_type(program, machine, state, assignment)?;
    // Bare descriptor replacement belongs to alias transport. A value-shaped
    // assignment through a reference replaces the referent's stored subtree.
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
    let may_select = source
        .segments
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Index { .. }));
    let declared_inventory = if may_select && established.is_none() {
        let root_type = program
            .state_parameter_by_symbol(state, source.root)
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
        Some(inventory)
    } else {
        None
    };
    for segment in &mut source.segments {
        if let PlaceSegment::Index { expression } = segment {
            // No runtime selector identity crosses a statement or call. The
            // may-select row records possible residence, not an executable index.
            *expression = Default::default();
        }
    }
    // A move row owns its whole subtree. A replacement strictly inside that
    // subtree cannot split the transport it describes. Neither an exact nor
    // a possible member write can preserve a moved ancestor's whole-value
    // identity while silently changing some of its children.
    if established.is_some_and(|established| {
        established.moves.iter().any(|moved| {
            moved.local_segments.len() < source.segments.len()
                && prefix_matches(&moved.local_segments, &source.segments)
        })
    }) {
        return None;
    }
    let leaves = reference_leaves(
        program,
        machine,
        assignment.value,
        target_type,
        "",
        symbols,
        inference,
        include_shared,
        &|expression, reference, _implicit_borrow, inference| {
            if include_shared {
                reference_subjects::backing_origin(
                    program, machine, expression, symbols, inference, aliases, stored,
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
    let mut replaced = StoredLocalOrigins {
        local_symbol: source.root,
        references: Vec::new(),
        cases: Vec::new(),
        moves: Vec::new(),
        symbolic: false,
    };
    let (target_path, precision) =
        coordinate_storage_path(program, program.symbols.name(source.root), &source.segments);
    for leaf in leaves.references {
        // Shared-value producers already captured their reference boundary.
        // Resolving that value through the current slot again would redirect
        // aliases after replacement.
        let captured = if include_shared {
            vec![leaf.origin.clone()]
        } else {
            canonical_reference_origins(program, &leaf.origin, aliases, stored)
        };
        for origin in captured {
            let mut local_segments = source.segments.clone();
            local_segments.extend(leaf.local_segments.iter().copied());
            replaced.references.push(StoredWriteOrigin {
                local_symbol: source.root,
                local_path: match precision {
                    FramePathPrecision::Exact => {
                        append_place_suffix(&target_path, &leaf.local_suffix)
                    }
                    FramePathPrecision::CollectionCoarse => target_path.clone(),
                },
                local_segments,
                may_select: may_select || leaf.may_select,
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
    Some(AssignmentOrigins {
        local: replaced,
        segments: source.segments,
        may_select,
        declared_inventory,
    })
}
