//! Exact local reference identity, separate from conservative write footprints.
use super::place_paths::{push_unique_origin, single_place_origin};
use super::{
    ExpressionHandle, ExpressionNode, FrameInference, FramePathPrecision, FramePlaceOrigin,
    FrameSourcePlace, Machine, MachineSymbols, State, StatementNode, StoredLocalOrigins,
    SymbolHandle, TopLevelSymbols, TypeReferenceHandle, TypeReferenceNode, caller_aliases,
    frame_place_path, local_aliases, reference_origins, stored_origins,
    type_is_caller_isolated_local, type_may_carry_write, type_reference_is_reference,
};
use crate::machine_calls::calls::write_frames::alias_origins::stable_alias_initializer_origins;
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
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let unconstrained = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if let TypeReferenceNode::Reference { access, .. } =
        program.type_reference_table.type_reference(unconstrained)
        && !access.is_readable()
    {
        return None;
    }
    carrier_storage_declaration(program, reference)
}

/// Inventory declared descriptor slots for write effects without reading them.
/// A write-only formal retains its incoming slot identities so a later store
/// can replace their loans; these rows grant no observation or pointer access.
pub(super) fn carrier_storage_declaration(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let mut reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    (reference.is_valid() && !type_reference_is_reference(program, reference)).then_some(reference)
}

/// Retain an unresolved read-only binding without claiming a storage identity.
/// Only the exact-reference prefix query uses this marker, after checking RHS
/// effects and binding exposure. Write-capable carriers must remain opaque to
/// the whole query because their unknown origin can hide a caller write.
pub(super) fn unknown_readonly_origin(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    let reference = source_root_type(program, state, index, target.root)?;
    Some(reference_value_replaces_binding(
        program,
        machine,
        state,
        reference,
        assignment.value,
    ))
}

/// Reference-valued assignment replaces the selected carrier; value-shaped
/// assignment writes through it. This query classifies the actual typed place,
/// including a projected slot, without changing bare-local alias transfer.
pub(super) fn replaces_reference_binding(
    program: &dyn crate::CheckingInput,
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
    Some(reference_value_replaces_binding(
        program,
        machine,
        state,
        reference,
        assignment.value,
    ))
}

/// The installed value is the reference descriptor for a rebinding and the
/// referent for a write through it. Loan formation and stored-origin transfer
/// must use this same choice: a reference-typed destination does not imply
/// that an owned replacement borrows its source carrier.
pub fn assignment_stored_value_type(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<TypeReferenceHandle> {
    if assignment.receiving_type.is_valid() {
        return Some(assignment.receiving_type);
    }
    let reference = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.target,
    )?;
    if reference_value_replaces_binding(program, machine, state, reference, assignment.value) {
        Some(reference)
    } else {
        Some(crate::reference_shell(program, reference).map_or(reference, |shell| shell.referee))
    }
}

fn reference_value_replaces_binding(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    reference: TypeReferenceHandle,
    value: ExpressionHandle,
) -> bool {
    crate::reference_shell(program, reference).is_some_and(|shell| {
        // Shared permission cannot write its referent. Assignment to that
        // reference-valued slot replaces its descriptor, including implicit
        // static views (string literals) with no borrowed backing to infer.
        shell.access == language_semantics::ReferenceAccess::Shared
            || local_aliases::expression_may_rebind_mutable_alias(program, machine, state, value)
    })
}

pub(super) fn expression_origins_before_statement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    before: &StatementNode,
    expression: ExpressionHandle,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<Vec<FrameSourcePlace>> {
    let (state, _, index) = caller_aliases::caller_statement_owner(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(before),
    )?;
    let prefix = collected_prefix_at(collections, program, machine, state, symbols, index, true)?;
    let origins = value_origins(
        program,
        machine,
        expression,
        symbols,
        &mut FrameInference::default(),
        &prefix.aliases,
        &prefix.stored,
        true,
    )?;
    origins
        .into_iter()
        .map(|origin| (origin.precision == FramePathPrecision::Exact).then_some(origin.source))
        .collect()
}

pub(super) fn local_origin(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    before: &StatementNode,
    local_symbol: SymbolHandle,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<FrameSourcePlace> {
    let origins = local_origins(program, machine, symbols, before, local_symbol, collections)?;
    let [origin] = origins.as_slice() else {
        return None;
    };
    Some(origin.clone())
}

pub(super) fn local_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    before: &StatementNode,
    local_symbol: SymbolHandle,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<Vec<FrameSourcePlace>> {
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
    let candidates = if let Some((_, origin)) = origins.next() {
        if origins.next().is_some() {
            return None;
        }
        std::slice::from_ref(origin)
    } else {
        prefix
            .divergent
            .iter()
            .find(|(name, _)| name == local.name.as_str())?
            .1
            .as_slice()
    };
    if candidates.is_empty() {
        return None;
    }
    candidates
        .iter()
        .map(|origin| {
            if origin.precision != FramePathPrecision::Exact {
                return None;
            }
            let source = origin.source.clone();
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
                return None;
            }
            Some(source)
        })
        .collect()
}

/// Every exact captured referent of one reference value. Singleton consumers
/// collapse this set only when its complete candidates identify the same place.
pub(super) fn value_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    implicit_borrow: bool,
) -> Option<Vec<FramePlaceOrigin>> {
    let declared = declared_initializer_origins(
        program,
        machine,
        expression,
        symbols,
        inference,
        implicit_borrow,
        aliases,
        stored,
    )?;
    if results::call_expression(program, expression).is_some() {
        // Declaration resolution already crossed the actual's stored boundary.
        // Replaying capture here would load the resulting backing slot again.
        return Some(declared);
    }
    let (origins, state, index) = caller_isolated_initializer_origins(
        program, machine, expression, inference, aliases, symbols, stored,
    )?;
    let mut captured = Vec::new();
    for origin in origins {
        for origin in captured_value_origins(program, expression, &origin, aliases, stored) {
            let origin = origin?;
            if origin.precision != FramePathPrecision::Exact {
                return None;
            }
            validate_source_projection(program, machine, state, index, &origin.source, stored)?;
            push_unique_origin(&mut captured, origin);
        }
    }
    (!captured.is_empty()).then_some(captured)
}

/// Storage provenance for a reference-valued producer. Unlike an exact
/// subject, a range may retain a conservative collection region. The ordinary
/// alias walk authenticates the producer and captured binding; builtin selector
/// custody and exact nominal prefixes remain mandatory. This relation carries
/// lifetime/mutation obligations, never descriptor equality or predicates.
pub(super) fn backing_origin(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    single_place_origin(backing_origins(
        program, machine, expression, symbols, inference, aliases, stored,
    )?)
}

/// Preserve a reference producer's complete backing set, including coarse
/// collection coordinates. Exact-value consumers use `value_origins` instead.
/// The caller's isolated locals joined through `MachineSymbols` and the
/// stable alias-initializer origins for an expression write site — the
/// shared walk every captured-value scan starts from.
fn caller_isolated_initializer_origins<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
) -> Option<(Vec<FramePlaceOrigin>, &'program State, usize)> {
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
    stable_alias_initializer_origins(
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
    )
    .map(|origins| (origins, state, index))
}

pub(super) fn backing_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    let (origins, state, index) = caller_isolated_initializer_origins(
        program, machine, expression, inference, aliases, symbols, stored,
    )?;
    let mut captured = Vec::new();
    for origin in origins {
        for origin in captured_value_origins(program, expression, &origin, aliases, stored) {
            let origin = origin?;
            if (!origin.source.root.is_valid() && !origin.source.is_static())
                || !origin.source.builtin_coordinates
            {
                return None;
            }
            if !origin.source.is_static() {
                // Exact aliases retain the declared-value checks used by
                // singleton consumers. Captured alternatives never weaken
                // loaded-reference or nominal-selector authentication.
                if origin.precision == FramePathPrecision::Exact {
                    declared_initializer_origins(
                        program, machine, expression, symbols, inference, false, aliases, stored,
                    )?;
                }
                validate_backing_projection(
                    program,
                    machine,
                    state,
                    index,
                    &origin.source,
                    stored,
                )?;
            }
            push_unique_origin(&mut captured, origin);
        }
    }
    (!captured.is_empty()).then_some(captured)
}

/// Authenticate the supplied collection's declaration and frozen reference
/// boundary. The remaining selectors describe possible backing, not an exact
/// predicate subject or a new permission; executable formation owns bounds.
pub(super) fn validate_backing_projection(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    stored: &[StoredLocalOrigins],
) -> Option<()> {
    if !source.root.is_valid() || !source.builtin_coordinates {
        return None;
    }
    let mut prefix = source.clone();
    if let Some(position) = prefix.segments.iter().position(|segment| {
        matches!(
            segment,
            PlaceSegment::Index { .. }
                | PlaceSegment::FixedIndex { .. }
                | PlaceSegment::FixedRange { .. }
        )
    }) {
        prefix.segments.truncate(position);
    }
    validate_source_projection(program, machine, state, index, &prefix, stored)
}

/// Alias rows are captured reference values. Resolve fresh field loads only
/// beyond the reference boundary already crossed by the original alias.
pub(super) fn captured_value_origins(
    program: &dyn crate::CheckingInput,
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

fn declared_initializer_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    mut expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    implicit_borrow: bool,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    // Every reference formation lends its source, including a recast without
    // an outer Borrow node. Validate that source's selectors, not the syntax
    // wrapper or the new observation declaration.
    let formation = crate::reference_formation_source(program, expression);
    let explicit_borrow = formation.is_some();
    if let Some(formation) = formation {
        expression = formation.source;
    }
    expression = program
        .expression_table
        .peeled_expression_handle(expression);
    if let Some(_) = program.expression_table.call_node(expression) {
        let (state, _, index) = caller_aliases::caller_statement_owner(
            program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        let returned = results::call_origins(
            program, machine, expression, symbols, inference, aliases, stored,
        )?;
        // The returned leaf names the caller-visible storage only after the
        // stored evidence resolves it: a leaf loaded inside the callee's
        // borrowed carrier lands on the caller's leaf slot, whose referent the
        // slot's own row supplies.
        let mut captured = Vec::new();
        for origin in returned {
            for origin in
                stored_origins::canonical_reference_origins(program, &Some(origin), aliases, stored)
            {
                let origin = origin?;
                if origin.precision != FramePathPrecision::Exact {
                    return None;
                }
                validate_source_projection(program, machine, state, index, &origin.source, stored)?;
                push_unique_origin(&mut captured, origin);
            }
        }
        return (!captured.is_empty()).then_some(captured);
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
    let reference = source_root_type(program, state, index, source.root)?;
    if projections::validate_owned_source(program, machine, reference, &source).is_none() {
        return frozen_reference_origin(
            program, machine, state, index, expression, aliases, stored,
        )
        .map(|origin| vec![origin]);
    }
    frame_place_path(program, expression).map(|origin| vec![origin])
}

/// A loaded slot has identity only through a matching declaration-time leaf.
/// Type-derived overlap and a similarly spelled reference field are insufficient.
fn frozen_reference_origin(
    program: &dyn crate::CheckingInput,
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
        source_root_type(program, state, index, source.root)?,
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
            .iter()
            .filter(|leaf| leaf.local_segments.is_empty())
            .count()
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    stored: &[StoredLocalOrigins],
) -> Option<()> {
    let Some(first) = source.recasts.first() else {
        if source.representation.is_some() {
            return None;
        }
        return validate_declaration_source_projection(
            program, machine, state, index, source, stored,
        );
    };
    // Prove the original declaration prefix once. Each later selector belongs
    // to the preceding interpretation, not to that original field namespace.
    // These coordinates still grant no loan or representation validity proof.
    let mut original = source.clone();
    original.segments = first.segments.clone();
    original.representation = None;
    original.recasts.clear();
    validate_declaration_source_projection(program, machine, state, index, &original, stored)?;
    let mut presentation = None;
    for recast in &source.recasts {
        if recast.root != source.root {
            return None;
        }
        match (presentation, &recast.representation) {
            (None, None) => {}
            (Some(previous), Some((reference, selectors)))
                if program.normalized_type_identity(previous)
                    == program.normalized_type_identity(*reference) =>
            {
                validate_owned_projection(program, *reference, selectors)?;
            }
            _ => return None,
        }
        let formation = crate::reference_formation_source(program, recast.formation_expression)?;
        if !matches!(
            formation.interpretation,
            crate::ReferenceFormationInterpretation::RepresentationView
                | crate::ReferenceFormationInterpretation::Placed
        ) || !formation.view_type.is_valid()
            || program.normalized_type_identity(formation.view_type)
                != program.normalized_type_identity(recast.target_type)
        {
            return None;
        }
        if formation.interpretation == crate::ReferenceFormationInterpretation::Placed {
            // Placement keeps a logical accessor namespace beside the exact
            // original schema. It does not obtain ordinary recast permission.
            let view = crate::value_custody::places::data_definition_for_type(
                program,
                formation.view_type,
            )?;
            let mut plans = program
                .placed_view_plans
                .iter()
                .filter(|plan| plan.data_symbol == view.symbol);
            let (Some(plan), None) = (plans.next(), plans.next()) else {
                return None;
            };
            let root = source_root_type(program, state, index, recast.root)?;
            let source_type = crate::project_type_reference_from_segments(
                program,
                carrier_storage_type(program, root)?,
                &recast.segments,
            )?;
            let schema = crate::value_custody::places::data_definition_for_type(
                program,
                carrier_storage_type(program, source_type)?,
            )?;
            if schema.symbol != plan.schema_symbol {
                return None;
            }
        }
        presentation = Some(recast.target_type);
    }
    let (reference, selectors) = source.representation.as_ref()?;
    if presentation.map(|reference| program.normalized_type_identity(reference))
        != Some(program.normalized_type_identity(*reference))
    {
        return None;
    }
    validate_owned_projection(program, *reference, selectors)
}

fn validate_declaration_source_projection(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    index: usize,
    source: &FrameSourcePlace,
    stored: &[StoredLocalOrigins],
) -> Option<()> {
    let reference = source_root_type(program, state, index, source.root)?;
    if projections::validate_owned_source(program, machine, reference, source).is_some() {
        return Some(());
    }
    let reference = carrier_storage_type(program, reference)?;
    if !program
        .state_parameter_by_symbol(state, source.root)
        .is_some()
        || source.segments.iter().any(|segment| {
            !matches!(
                segment,
                PlaceSegment::Field { .. }
                    | PlaceSegment::Case { .. }
                    | PlaceSegment::FixedIndex { .. }
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
            .iter()
            .filter(|leaf| leaf.local_segments.is_empty())
            .count()
            != 1
    {
        return None;
    }
    validate_owned_projection(program, slot, &source.segments[leaf.local_segments.len()..])
}

pub(super) fn source_root_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    index: usize,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !root.is_valid() {
        return None;
    }
    let mut parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == root);
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
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<()> {
    reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    // Effect footprints deliberately coarsen even literal array selections.
    // The retained source coordinate still identifies an exact fixed element;
    // the shared type walk checks its declared extent alongside Field/Case
    // ownership. A runtime Index or collection wildcard cannot establish that
    // exact source, even though the type walker can identify its element type.
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
        )
    }) {
        return None;
    }
    stored_origins::projected_type(program, reference, segments).map(|_| ())
}
