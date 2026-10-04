//! Exact local reference identity, separate from conservative write footprints.
use super::{
    ExpressionHandle, ExpressionNode, FrameInference, FramePathPrecision, FramePlaceOrigin,
    FrameSourcePlace, Machine, MachineSymbols, State, StatementNode, StoredLocalOrigins,
    SymbolHandle, TopLevelSymbols, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    caller_aliases, frame_place_path, local_aliases, reference_origins, stored_origins,
    type_is_caller_isolated_local, type_may_carry_write, type_reference_is_reference,
};
use crate::machine_calls::calls::write_frames::alias_origins::stable_alias_initializer_origin;
use crate::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, collected_prefix_at,
};
use facts::PlaceSegment;
use std::sync::Mutex;
use symbols::SymbolKeyMap as HashMap;

pub(super) mod bindings;
mod projections;
mod results;

/// The input's own readable reference can expose carrier storage. This is a
/// type walk only: selecting a reference field still requires a seeded leaf
/// whose binding survives every statement before the query.
pub(super) fn carrier_storage_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    if let TypeReferenceNode::Reference {
        access, referee, ..
    } = program.type_reference_table.type_reference(reference)
    {
        if !access.is_readable() {
            return None;
        }
        reference = *referee;
    }
    (reference.is_valid() && !type_reference_is_reference(program, reference)).then_some(reference)
}

/// Retain an unresolved read-only binding without claiming a storage identity.
/// Only the exact-reference prefix query uses this marker, after checking RHS
/// effects and binding exposure. Write-capable carriers must remain opaque to
/// the whole query because their unknown origin can hide a caller write.
pub(super) fn unknown_readonly_origin(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
) -> Option<FramePlaceOrigin> {
    (type_reference_is_reference(program, reference) && !type_may_carry_write(program, reference))
        .then(|| FramePlaceOrigin {
            path: name.to_owned(),
            precision: FramePathPrecision::CollectionCoarse,
            source: FrameSourcePlace::default(),
        })
}

pub(super) fn replaces_binding(
    program: &TypedTrees,
    machine: &Machine,
    statement: &StatementNode,
) -> Option<bool> {
    let StatementNode::Assignment(assignment) = statement else {
        return Some(false);
    };
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(statement),
    )?;
    let target = FrameSourcePlace::from_expression(program, assignment.target);
    if !target.segments.is_empty()
        || program.symbols.get(target.root).kind != symbols::SymbolKind::Local
    {
        return Some(false);
    }
    let reference = source_root_type(program, machine, state, index, target.root)?;
    Some(
        type_reference_is_reference(program, reference)
            && local_aliases::expression_may_rebind_mutable_alias(
                program,
                machine,
                state,
                assignment.value,
            ),
    )
}

/// Reference-valued assignment replaces the selected carrier; value-shaped
/// assignment writes through it. This query classifies the actual typed place,
/// including a projected slot, without changing bare-local alias transfer.
pub(super) fn replaces_reference_binding(
    program: &TypedTrees,
    machine: &Machine,
    statement: &StatementNode,
) -> Option<bool> {
    let StatementNode::Assignment(assignment) = statement else {
        return Some(false);
    };
    let (state, _, _) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(statement),
    )?;
    let reference = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.target,
    )?;
    Some(
        type_reference_is_reference(program, reference)
            && local_aliases::expression_may_rebind_mutable_alias(
                program,
                machine,
                state,
                assignment.value,
            ),
    )
}

pub(super) fn local_origin(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    before: &StatementNode,
    local_symbol: SymbolHandle,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<FrameSourcePlace> {
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(before),
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let (_, local) = program
        .state_local_declaration(state, local_symbol)
        .filter(|&(ordinal, _)| ordinal < index)?;
    if !type_reference_is_reference(program, local.type_reference)
        || program.symbols.get(local_symbol).parent != state.symbol
        || program.symbols.get(local_symbol).kind != symbols::SymbolKind::Local
        || program.symbols.name(local_symbol) != local.name.as_str()
    {
        return None;
    }
    let prefix = collected_prefix_at(collections, program, machine, state, symbols, index, true)?;
    let mut origins = prefix
        .aliases
        .iter()
        .filter(|(name, _)| name == local.name.as_str());
    let (_, origin) = origins.next()?;
    if origins.next().is_some() || origin.precision != FramePathPrecision::Exact {
        return None;
    }
    let mut source = origin.source.clone();
    validate_source_projection(program, machine, state, index, &source, &prefix.stored)?;
    // A reference local left in the result has not been canonically resolved.
    // It is not evidence that the similarly named slot kept its old referent.
    if statements[..index].iter().any(|statement| {
        matches!(statement,
        StatementNode::LocalData(local) if local.symbol == source.root
            && type_reference_is_reference(program, local.type_reference))
    }) {
        return None;
    }
    if source.root == machine.symbol {
        source.root = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)?
            .symbol;
    }
    Some(source)
}

pub(super) fn value_origin(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    implicit_borrow: bool,
) -> Option<FramePlaceOrigin> {
    let declared = declared_initializer_origin(
        program,
        machine,
        expression,
        symbols,
        inference,
        implicit_borrow,
        aliases,
        stored,
    )?;
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    let origin = if results::call_expression(program, expression).is_some() {
        declared
    } else {
        let isolated = program.statement_table.statements(state.statement_nodes)[..index]
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local)
                    if type_is_caller_isolated_local(program, local.type_reference)
                        && !type_reference_is_reference(program, local.type_reference) =>
                {
                    Some(local.name.as_str().to_owned())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return None;
        }
        stable_alias_initializer_origin(
            program,
            machine,
            &machine_symbols,
            inference,
            expression,
            program.state_parameters(state),
            &isolated,
            aliases,
            &[],
            symbols,
            true,
            stored,
        )?
    };
    let mut origins =
        captured_value_origins(program, expression, &origin, aliases, stored).into_iter();
    let origin = origins.next()??;
    if origins.next().is_some() || origin.precision != FramePathPrecision::Exact {
        return None;
    }
    validate_source_projection(program, machine, state, index, &origin.source, stored)?;
    Some(origin)
}

/// Storage provenance for a reference-valued producer. Unlike an exact
/// subject, a range may retain a conservative collection region. The ordinary
/// alias walk authenticates the producer and captured binding; builtin selector
/// custody and exact nominal prefixes remain mandatory. This relation carries
/// lifetime/mutation obligations, never descriptor equality or predicates.
pub(super) fn backing_origin(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    let isolated = program.statement_table.statements(state.statement_nodes)[..index]
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local)
                if type_is_caller_isolated_local(program, local.type_reference)
                    && !type_reference_is_reference(program, local.type_reference) =>
            {
                Some(local.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return None;
    }
    let origin = stable_alias_initializer_origin(
        program,
        machine,
        &machine_symbols,
        inference,
        expression,
        program.state_parameters(state),
        &isolated,
        aliases,
        &[],
        symbols,
        true,
        stored,
    )?;
    let mut candidates =
        captured_value_origins(program, expression, &origin, aliases, stored).into_iter();
    let origin = candidates.next()??;
    if candidates.next().is_some()
        || (!origin.source.root.is_valid() && !origin.source.is_static())
        || !origin.source.builtin_coordinates
    {
        return None;
    }
    if origin.source.is_static() {
        return Some(origin);
    }
    // An exact alias may later serve exact-reference consumers. Preserve their
    // existing source-declaration/selector authentication rather than allowing
    // the conservative backing query to manufacture exact identity.
    if origin.precision == FramePathPrecision::Exact {
        declared_initializer_origin(
            program, machine, expression, symbols, inference, false, aliases, stored,
        )?;
    }
    let mut prefix = origin.source.clone();
    if let Some(index) = prefix.segments.iter().position(|segment| {
        matches!(
            segment,
            PlaceSegment::Index { .. }
                | PlaceSegment::FixedIndex { .. }
                | PlaceSegment::FixedRange { .. }
        )
    }) {
        prefix.segments.truncate(index);
    }
    validate_source_projection(program, machine, state, index, &prefix, stored)?;
    Some(origin)
}

/// Alias rows are captured reference values. Resolve fresh field loads only
/// beyond the reference boundary already crossed by the original alias.
fn captured_value_origins(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: &FramePlaceOrigin,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Vec<Option<FramePlaceOrigin>> {
    let raw = FrameSourcePlace::from_expression(program, expression);
    if raw.root.is_valid()
        && let Some((_, captured)) = aliases
            .iter()
            .find(|(name, _)| program.symbols.name(raw.root) == name)
    {
        return stored_origins::captured_reference_projection_origins(
            program, captured, origin, stored,
        );
    }
    stored_origins::canonical_reference_origins(program, &Some(origin.clone()), &[], stored)
}

fn declared_initializer_origin(
    program: &TypedTrees,
    machine: &Machine,
    mut expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    implicit_borrow: bool,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    let explicit_borrow = matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Borrow(_)
    );
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        expression = borrow.target;
    }
    if let ExpressionNode::Call(_) = program.expression_table.expression(expression) {
        let (state, _, index) = caller_aliases::caller_statement_owner(
            program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        let origin = results::call_origin(
            program, machine, expression, symbols, inference, aliases, stored,
        )?;
        // The returned leaf names the caller-visible storage only after the
        // stored evidence resolves it: a leaf loaded inside the callee's
        // borrowed carrier lands on the caller's leaf slot, whose referent the
        // slot's own row supplies.
        let mut origins =
            stored_origins::canonical_reference_origins(program, &Some(origin), aliases, stored)
                .into_iter();
        let origin = origins.next()??;
        if origins.next().is_some() {
            return None;
        }
        validate_source_projection(program, machine, state, index, &origin.source, stored)?;
        return (origin.precision == FramePathPrecision::Exact).then_some(origin);
    }
    reference_origins::declared_origin_root(program, machine, expression)?;
    projections::validate_selectors(program, machine, expression)?;
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    if !explicit_borrow
        && !implicit_borrow
        && !type_reference_is_reference(
            program,
            crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            )?,
        )
    {
        return None;
    }
    let source = FrameSourcePlace::from_expression(program, expression);
    let reference = source_root_type(program, machine, state, index, source.root)?;
    if projections::validate_owned_source(program, machine, reference, &source).is_none() {
        return frozen_reference_origin(
            program, machine, state, index, expression, aliases, stored,
        );
    }
    frame_place_path(program, expression)
}

/// A loaded slot has identity only through a matching declaration-time leaf.
/// Type-derived overlap and a similarly spelled reference field are insufficient.
fn frozen_reference_origin(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    index: usize,
    expression: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    let raw = frame_place_path(program, expression)?;
    if raw.source.segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
        )
    }) {
        return None;
    }
    // Resolve the carrier alias before selecting its stored leaf, but retain
    // the carrier's selected-case evidence before substituting that leaf's
    // referent. A possible payload is not an established one.
    let source = stored_origins::binding_source(program, expression, aliases)?;
    let reference = carrier_storage_type(
        program,
        source_root_type(program, machine, state, index, source.root)?,
    )?;
    let carrier = stored
        .iter()
        .find(|carrier| carrier.local_symbol == source.root)?;
    let mut leaves = carrier
        .references
        .iter()
        .filter(|leaf| source.segments.starts_with(&leaf.local_segments));
    let leaf = leaves.next()?;
    if leaves.next().is_some() {
        return None;
    }
    let slot = stored_origins::projected_storage_type(program, reference, &leaf.local_segments)?;
    if !type_reference_is_reference(program, slot)
        || stored_origins::project_stored_origins(program, carrier, &leaf.local_segments, false)?
            .references
            .len()
            != 1
    {
        return None;
    }
    let mut origins =
        stored_origins::canonical_reference_origins(program, &Some(raw), aliases, stored)
            .into_iter();
    let origin = origins.next()??;
    if origins.next().is_some() || origin.precision != FramePathPrecision::Exact {
        return None;
    }
    validate_source_projection(program, machine, state, index, &origin.source, stored)?;
    Some(origin)
}

/// Reference queries seed owned or readable borrowed input leaves and validate
/// their stability through the queried prefix. A helper result checks its whole
/// body before exporting that boundary for caller substitution. Neither query
/// supplies a qualification merely from the input's declared type.
pub(super) fn validate_source_projection(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    stored: &[StoredLocalOrigins],
) -> Option<()> {
    let reference = source_root_type(program, machine, state, index, source.root)?;
    if projections::validate_owned_source(program, machine, reference, source).is_some() {
        return Some(());
    }
    let reference = carrier_storage_type(program, reference)?;
    if !program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == source.root && !parameter.is_self)
        || source.segments.iter().any(|segment| {
            !matches!(
                segment,
                PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
            )
        })
    {
        return None;
    }
    let input = stored
        .iter()
        .find(|input| input.local_symbol == source.root)?;
    let mut leaves = input
        .references
        .iter()
        .filter(|leaf| source.segments.starts_with(&leaf.local_segments));
    let leaf = leaves.next()?;
    if leaves.next().is_some() {
        return None;
    }
    let slot = stored_origins::projected_storage_type(program, reference, &leaf.local_segments)?;
    if !type_reference_is_reference(program, slot)
        || stored_origins::project_stored_origins(program, input, &leaf.local_segments, false)?
            .references
            .len()
            != 1
    {
        return None;
    }
    validate_owned_projection(program, slot, &source.segments[leaf.local_segments.len()..])
}

fn source_root_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    index: usize,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !root.is_valid() {
        return None;
    }
    let mut parameters = program.state_parameters(state).iter().filter(|parameter| {
        parameter.symbol == root || (root == machine.symbol && parameter.is_self)
    });
    if let Some(parameter) = parameters.next() {
        return parameters
            .next()
            .is_none()
            .then_some(parameter.type_reference);
    }
    if program.symbols.get(root).parent != state.symbol
        || program.symbols.get(root).kind != symbols::SymbolKind::Local
    {
        return None;
    }
    program
        .state_local_declaration(state, root)
        .filter(|&(ordinal, _)| ordinal < index)
        .map(|(_, local)| local.type_reference)
}

pub(super) fn validate_owned_projection(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<()> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    // No collection-footprint wildcard or runtime expression establishes an
    // exact source. The shared type walk validates nominal Field/Case owners.
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
        )
    }) {
        return None;
    }
    stored_origins::projected_type(program, reference, segments).map(|_| ())
}
