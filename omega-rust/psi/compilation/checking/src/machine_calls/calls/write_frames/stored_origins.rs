//! Declaration-frozen reference leaves in by-value local aggregates.
//!
//! The literal walker is shared with call instantiation. State transfer stores
//! canonical origins now, not expressions to replay after an alias is rebound.
//!
//! A reference slot and knowledge of its referent are separate facts. Keep an
//! unresolved slot through projection, moves, calls and joins rather than
//! rejecting every use of its enclosing record or omitting the slot. Only an
//! overlapping write demands its source; that demand fails closed before
//! private-local filtering. Exact-origin consumers likewise reject unknown
//! sources. This replaces the local pure-tail-return exception with ordinary
//! transfer, without treating incomplete effect summaries as complete.

use super::path_instantiation::aggregate_arguments::reference_leaves_with_origins;
use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root,
};
use super::{Machine, SymbolHandle, TopLevelSymbols, TypedTrees};
use crate::machine_calls::calls::write_frames::FrameInference;
use facts::PlaceSegment;
use typed_trees::statement::TableLocalData;
use typed_trees::types::TypeReferenceNode;

mod assignments;
mod frozen_bindings;
mod projections;
mod reference_values;
mod type_origins;

/// Case evidence also matters for payloads with no exclusive references. It
/// cannot be discovered by asking whether a local contributes writes.
pub(super) fn has_aggregate_case_shape(
    program: &TypedTrees,
    reference: super::TypeReferenceHandle,
) -> bool {
    type_origins::declares_cases(program, reference)
}

pub(super) use assignments::assignment_origins;
pub(super) use frozen_bindings::assignment_replaces_case_binding;
pub(super) use frozen_bindings::assignment_replaces_reference_ancestor;
pub(super) use frozen_bindings::binding_source;
pub(super) use frozen_bindings::statement_exposes_frozen_binding;
pub(super) use frozen_bindings::{call_exposes_frozen_binding, expression_exposes_frozen_binding};

pub(super) use projections::reference_leaves_before_statement_for_query;
pub(super) use projections::symbolic_reference_leaves;
pub(super) use projections::{project_stored_origins, projected_storage_type, projected_type};
pub(super) use reference_values::source_reaches_leaf;
pub(super) use reference_values::{
    canonical_reference_origins, captured_reference_projection_origins,
};
pub(super) use type_origins::declared_origins;
pub(super) use type_origins::declared_origins_for_query;
pub(super) use type_origins::demand_is_declared;

#[derive(Debug, Clone)]
pub(super) struct StoredLocalOrigins {
    pub local_symbol: SymbolHandle,
    pub references: Vec<StoredWriteOrigin>,
    pub cases: Vec<Vec<PlaceSegment>>,
    pub moves: Vec<super::path_instantiation::aggregate_arguments::AggregateMove>,
    /// Type-derived frontier rows are a symbolic may-set: a selected element
    /// can specialize an unknown index. Rows observed from an assigned or
    /// initialized value already name their source selectors.
    pub symbolic: bool,
}

/// An exclusive call can replace a private value's cases without introducing
/// caller-visible references. Its old case and move snapshots cannot survive
/// that call. Retain an empty row and append the same inference snapshot so a
/// later projection cannot recover the overwritten initializer's evidence.
/// Retiring before call traversal is conservative: it supplies no new origin,
/// and the ordinary exposure fence and complete call frame are still required.
pub(super) fn retire_private_call_argument_cases(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &mut [StoredLocalOrigins],
    inference: &mut FrameInference,
) {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::StatementNode;

    for argument in program.statement_table.expression_handles(call.arguments) {
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(*argument) else {
            continue;
        };
        if !borrow.access.is_exclusive() {
            continue;
        }
        let ExpressionNode::Name(name) = program.expression_table.expression(borrow.target) else {
            continue;
        };
        let Some(source) = frozen_bindings::binding_source(program, borrow.target, aliases) else {
            continue;
        };
        if !source.root.is_valid()
            || source.root != name.symbol
            || !source.segments.is_empty()
            || program
                .expression_table
                .name_path_members(name.members)
                .len()
                != 1
        {
            continue;
        }
        let declaration = program.symbols.get(source.root);
        if declaration.kind != symbols::SymbolKind::Local || declaration.parent != state.symbol {
            continue;
        }
        let Some(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == source.root => Some(local),
                _ => None,
            })
        else {
            continue;
        };
        if !super::type_is_caller_isolated_local(program, local.type_reference)
            || super::type_may_carry_write(program, local.type_reference)
        {
            continue;
        }
        let Some(established) = stored
            .iter_mut()
            .find(|local| local.local_symbol == source.root)
        else {
            continue;
        };
        if !established.references.is_empty()
            || (established.cases.is_empty() && established.moves.is_empty())
        {
            continue;
        }
        established.cases.clear();
        established.moves.clear();
        established.symbolic = false;
        inference.record_local(established);
    }
}

#[derive(Debug, Clone)]
pub(super) struct StoredWriteOrigin {
    pub local_symbol: SymbolHandle,
    pub local_path: String,
    pub local_segments: Vec<PlaceSegment>,
    /// A reference boundary, or opaque region which may contain one, remains
    /// present even when its captured storage is unknown. Such a region must
    /// survive transport and prevent overlapping writes from being discarded
    /// as private local storage; it proves no actual referent identity.
    pub origin: Option<FramePlaceOrigin>,
}

/// The type supplies a reference frontier even when value transfer cannot
/// establish its sources. This is unknown evidence, never an empty footprint.
pub(super) fn unresolved_origins(
    program: &TypedTrees,
    machine: &Machine,
    local: &TableLocalData,
    include_shared: bool,
) -> Option<StoredLocalOrigins> {
    local_declaration_state(program, machine, local)?;
    let mut origins = type_origins::declared_origins_for_query(
        program,
        local.symbol,
        local.name.as_str(),
        local.type_reference,
        include_shared,
    )?;
    for leaf in &mut origins.references {
        leaf.origin = None;
    }
    // Unknown transport cannot establish conserved input identity.
    origins.moves.clear();
    Some(origins)
}

/// Unknown referents do not permit unknown local identities. Both exact
/// transfer and type-only inventory must name this actual live declaration,
/// not a same-spelled local in another state or a stale symbol generation.
fn local_declaration_state<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    local: &TableLocalData,
) -> Option<&'program typed_trees::state::State> {
    if !local.symbol.is_valid() {
        return None;
    }
    let declaration = program.symbols.get(local.symbol);
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == declaration.parent)?;
    (declaration.kind == symbols::SymbolKind::Local
        && program.symbols.name(local.symbol) == local.name.as_str()
        && program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, typed_trees::statement::StatementNode::LocalData(candidate)
                if std::ptr::eq(candidate, local))
            }))
    .then_some(state)
}

pub(super) fn declaration_origins(
    program: &TypedTrees,
    machine: &Machine,
    local: &TableLocalData,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<StoredLocalOrigins> {
    declaration_origins_for_query(
        program, machine, local, aliases, stored, symbols, inference, false,
    )
}

pub(super) fn declaration_origins_for_query(
    program: &TypedTrees,
    machine: &Machine,
    local: &TableLocalData,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<StoredLocalOrigins> {
    if super::type_reference_is_reference(program, local.type_reference) {
        return None;
    }
    let state = local_declaration_state(program, machine, local)?;
    // A declared-zeroed local (`let room: Room;`) has no initializer to walk.
    // Its declaration frontier is the type-derived symbolic may-set — the same
    // seed a by-value state parameter receives — so a later field store to its
    // own storage still enumerates exact places instead of going opaque.
    if !local.initial_value.is_valid() {
        let mut origins = type_origins::declared_origins_for_query(
            program,
            local.symbol,
            local.name.as_str(),
            local.type_reference,
            include_shared,
        )?;
        // This declaration establishes fresh storage, not a transported input
        // subtree. Keep its symbolic case/reference inventory, but do not let
        // the parameter-style move row prohibit initializing an interior field.
        origins.moves.clear();
        return Some(origins);
    }
    let leaves = reference_leaves_with_origins(
        program,
        machine,
        local.initial_value,
        local.type_reference,
        "",
        symbols,
        inference,
        include_shared,
        &|expression, reference, _implicit_borrow, inference| {
            if include_shared {
                super::reference_subjects::backing_origin(
                    program, machine, expression, symbols, inference, aliases, stored,
                )
                .or_else(|| {
                    super::reference_subjects::unknown_readonly_origin(program, reference, "")
                })
                .map(|origin| vec![origin])
            } else {
                super::reference_origins::exclusive_reference_origins(
                    program, machine, expression, symbols, inference,
                )
            }
        },
        &|expression, reference, _| {
            projections::moved_reference_leaves(
                program,
                state,
                local,
                expression,
                reference,
                stored,
                include_shared,
            )
        },
    )?;
    let mut origins = Vec::new();
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
            origins.push(StoredWriteOrigin {
                local_symbol: local.symbol,
                local_path: append_place_suffix(local.name.as_str(), &leaf.local_suffix),
                local_segments: leaf.local_segments.clone(),
                origin,
            });
        }
    }
    Some(StoredLocalOrigins {
        local_symbol: local.symbol,
        references: origins,
        cases: leaves.cases,
        moves: leaves.moves,
        symbolic: false,
    })
}

/// Classify one exclusive borrow's replacement ceiling. Its enclosing call
/// separately narrows that ceiling using its complete may-write frame.
pub(super) fn borrow_target_exposes_carrier_binding(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    // An exclusive borrow of an exclusively-referenced place is a
    // reborrow: it reaches the referent the binding already names and
    // cannot write the slot itself. When the borrowed place is exactly a
    // stored-local reference leaf (a field-only member chain), the leaf's
    // declared origin is the load evidence and the borrow does not expose
    // the binding for replacement. Shared leaves, parameter roots,
    // indexed members, and non-reference places keep their exposure
    // fence: `&mut` on a shared leaf can only borrow the slot.
    let reborrows_proven_leaf = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        target,
    )
    .is_some_and(
        |reference| match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { access, .. } => access.is_exclusive(),
            _ => false,
        },
    ) && frozen_bindings::binding_source(program, target, aliases)
        .is_some_and(|source| {
            source
                .segments
                .iter()
                .all(|segment| matches!(segment, PlaceSegment::Field { .. }))
                && stored.iter().any(|local| {
                    local.local_symbol == source.root
                        && program.symbols.get(local.local_symbol).kind
                            == symbols::SymbolKind::Local
                        && local
                            .references
                            .iter()
                            .any(|leaf| leaf.local_segments == source.segments)
                })
        });
    if reborrows_proven_leaf {
        return false;
    }
    if frozen_bindings::target_replaces_case_binding(program, target, stored, aliases)
        || frozen_bindings::target_replaces_reference_binding(
            program, target, stored, aliases, true,
        )
    {
        return true;
    }
    let Some(path) = super::frame_place_path(program, target) else {
        return false;
    };
    let (root, _) = split_place_root(&path.path);
    let established_local = stored
        .iter()
        .any(|local| program.symbols.name(local.local_symbol) == root);
    // An incoming by-value carrier also anchors frozen leaf origins.
    // Replacing its reference slots would change what its symbolic
    // parameter-relative paths denote at the caller boundary.
    let incoming_carrier = program.state_parameters(state).iter().any(|parameter| {
        parameter.name.as_str() == root
            && !super::type_reference_is_reference(program, parameter.type_reference)
            && super::type_may_carry_write(program, parameter.type_reference)
            && !super::type_is_caller_isolated_local(program, parameter.type_reference)
    });
    (established_local || incoming_carrier)
        && crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            target,
        )
        .is_none_or(|reference| {
            super::type_may_carry_write(program, reference)
                && !super::type_is_caller_isolated_local(program, reference)
        })
}

pub(super) fn canonical_origins(
    origin: &FramePlaceOrigin,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    let (root, suffix) = split_place_root(&origin.path);
    let origin = aliases.iter().find(|(name, _)| name == root).map_or_else(
        || origin.clone(),
        |(_, prior)| compose_origin(prior, suffix, origin.precision),
    );
    let mut origins = Vec::new();
    let mut includes_private = true;
    for leaf in stored.iter().flat_map(|local| &local.references) {
        let source = if let Some(suffix) = place_suffix(&leaf.local_path, &origin.path) {
            includes_private = false;
            let precision = if leaf.local_segments.iter().any(|segment| {
                matches!(
                    segment,
                    PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
                )
            }) {
                FramePathPrecision::CollectionCoarse
            } else {
                origin.precision
            };
            compose_origin(leaf.origin.as_ref()?, suffix, precision)
        } else if place_suffix(&origin.path, &leaf.local_path).is_some() {
            compose_origin(leaf.origin.as_ref()?, "", origin.precision)
        } else {
            continue;
        };
        if !origins.iter().any(|prior: &FramePlaceOrigin| {
            prior.path == source.path && prior.precision == source.precision
        }) {
            origins.push(source);
        }
    }
    if includes_private {
        origins.push(origin);
    }
    Some(origins)
}

fn compose_origin(
    origin: &FramePlaceOrigin,
    suffix: &str,
    precision: FramePathPrecision,
) -> FramePlaceOrigin {
    FramePlaceOrigin {
        source: Default::default(),
        path: match origin.precision {
            FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
            FramePathPrecision::CollectionCoarse => origin.path.clone(),
        },
        precision: if origin.precision == FramePathPrecision::CollectionCoarse {
            origin.precision
        } else {
            precision
        },
    }
}

pub(super) fn place_suffix<'path>(root: &str, path: &'path str) -> Option<&'path str> {
    let suffix = path.strip_prefix(root)?;
    (suffix.is_empty() || suffix.starts_with('.') || suffix.starts_with('[')).then_some(suffix)
}

/// Keep the local spelling for fact invalidation. Caller summaries remove it
/// only after adding every overlapping external leaf.
pub(super) fn expand_write_path(
    path: &str,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<String>> {
    let mut paths = vec![super::rebase_local_alias_path(path, aliases)];
    for origin in canonical_origins(
        &FramePlaceOrigin {
            path: path.to_owned(),
            precision: FramePathPrecision::Exact,
            source: Default::default(),
        },
        aliases,
        stored,
    )? {
        if !paths.contains(&origin.path) {
            paths.push(origin.path);
        }
    }
    Some(paths)
}

/// Resolve every effect before any caller filters private local storage. An
/// unresolved overlapping reference makes the entire effect set opaque.
pub(super) fn expand_write_paths(
    paths: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<String>> {
    let mut expanded = Vec::new();
    for path in paths {
        expanded.extend(expand_write_path(path, aliases, stored)?);
    }
    Some(expanded)
}
