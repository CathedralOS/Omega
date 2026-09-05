//! Declaration-frozen reference leaves in by-value local aggregates.
//!
//! The literal walker is shared with call instantiation. State transfer stores
//! canonical origins now, not expressions to replay after an alias is rebound.

use super::path_instantiation::aggregate_arguments::reference_leaves_with_origins;
use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root,
};
use super::{Machine, SymbolHandle, TopLevelSymbols, TypedTrees};
use crate::calls::write_frames::FrameInference;
use psi_facts::PlaceSegment;
use psi_typed_trees::statement::TableLocalData;

mod frozen_cases;
mod projections;
mod reference_values;
mod type_origins;

/// Case evidence also matters for payloads with no exclusive references. It
/// cannot be discovered by asking whether a local contributes writes.
pub(super) fn has_aggregate_case_shape(
    program: &TypedTrees,
    reference: super::TypeReferenceHandle,
) -> bool {
    declared_origins(program, SymbolHandle::invalid(), "", reference)
        .is_some_and(|origins| !origins.cases.is_empty())
}

pub(super) use frozen_cases::assignment_replaces_case_binding;
pub(super) use frozen_cases::statement_exposes_frozen_binding;

pub(super) use projections::reference_leaves_before_statement;
pub(super) use projections::symbolic_reference_leaves;
pub(super) use reference_values::canonical_reference_origins;
pub(super) use reference_values::source_reaches_leaf;
pub(super) use type_origins::declared_origins;
pub(super) use type_origins::demand_is_declared;

#[derive(Debug, Clone)]
pub(super) struct StoredLocalOrigins {
    pub local_symbol: SymbolHandle,
    pub references: Vec<StoredWriteOrigin>,
    pub cases: Vec<Vec<PlaceSegment>>,
}

#[derive(Debug, Clone)]
pub(super) struct StoredWriteOrigin {
    pub local_symbol: SymbolHandle,
    pub local_path: String,
    pub local_segments: Vec<PlaceSegment>,
    pub origin: FramePlaceOrigin,
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
    if super::type_reference_is_reference(program, local.type_reference) || !local.symbol.is_valid()
    {
        return None;
    }
    let declaration = program.symbols.get(local.symbol);
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == declaration.parent)?;
    if declaration.kind != psi_symbols::SymbolKind::Local
        || program.symbols.name(local.symbol) != local.name.as_str()
        || !program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, psi_typed_trees::statement::StatementNode::LocalData(candidate)
                if std::ptr::eq(candidate, local))
            })
    {
        return None;
    }
    let leaves = reference_leaves_with_origins(
        program,
        machine,
        local.initial_value,
        local.type_reference,
        "",
        symbols,
        inference,
        &|expression, reference, _| {
            projections::moved_reference_leaves(
                program, state, local, expression, reference, stored,
            )
        },
    )?;
    let mut origins = Vec::new();
    for leaf in leaves.references {
        for origin in canonical_reference_origins(program, &leaf.origin, aliases, stored) {
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
    })
}

/// A call must not acquire replacement access to an established reference
/// slot or a whole carrier. Borrowing owned storage beneath a leaf is safe for
/// origin identity. Reuse the common expression traversal, including helpers.
pub(super) fn expression_borrows_carrier_binding(
    program: &TypedTrees,
    machine: &Machine,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
    stored: &[StoredLocalOrigins],
) -> bool {
    super::local_aliases::expression_has_exclusive_borrow(program, expression, &|target| {
        if frozen_cases::target_replaces_case_binding(program, target, stored) {
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
            && crate::places::declared_place_type_raw(program, machine, Some(state), target)
                .is_none_or(|reference| {
                    super::type_may_carry_write(program, reference)
                        && !super::type_is_caller_isolated_local(program, reference)
                })
    })
}

pub(super) fn canonical_origins(
    origin: &FramePlaceOrigin,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<FramePlaceOrigin> {
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
            compose_origin(&leaf.origin, suffix, precision)
        } else if place_suffix(&origin.path, &leaf.local_path).is_some() {
            compose_origin(&leaf.origin, "", origin.precision)
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
    origins
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
) -> Vec<String> {
    let mut paths = vec![super::rebase_local_alias_path(path, aliases)];
    for origin in canonical_origins(
        &FramePlaceOrigin {
            path: path.to_owned(),
            precision: FramePathPrecision::Exact,
            source: Default::default(),
        },
        aliases,
        stored,
    ) {
        if !paths.contains(&origin.path) {
            paths.push(origin.path);
        }
    }
    paths
}
