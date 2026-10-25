//! Project local or incoming parameter evidence without replaying an initializer.
//! FactPlan supplies selectors; exact declarations and selector-owned types
//! establish whether those selectors can transport the retained leaves.

use super::super::path_instantiation::aggregate_arguments::{
    AggregateMove, AggregateOrigins, ReferenceLeaf, ReferenceSource,
};
use super::super::type_instantiation::DeclarationScopes;
use super::StoredLocalOrigins;
use facts::{FactPlan, PlaceSegment};
use symbols::SymbolKind;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn moved_reference_leaves(
    program: &dyn crate::CheckingInput,
    state: &State,
    destination: &TableLocalData,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
) -> Option<AggregateOrigins> {
    let before = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find(|statement| {
            matches!(statement, StatementNode::LocalData(local)
            if std::ptr::eq(local, destination))
        })?;
    reference_leaves_before_statement_for_query(
        program,
        state,
        before,
        expression,
        expected,
        Some(stored),
        None,
        include_shared,
    )
}

/// Raw call instantiation retains symbolic binding paths. The existing caller
/// prefix transfer must expand those paths before filtering local storage.
pub(in crate::machine_calls::calls::write_frames) fn symbolic_reference_leaves(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    inference: &super::super::FrameInference,
) -> Option<AggregateOrigins> {
    let (state, before, _) = super::super::caller_aliases::caller_statement_owner(
        program,
        machine,
        super::super::caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    reference_leaves_before_statement(
        program,
        state,
        before,
        expression,
        expected,
        None,
        Some(inference),
    )
}

pub(in crate::machine_calls::calls::write_frames) fn reference_leaves_before_statement(
    program: &dyn crate::CheckingInput,
    state: &State,
    before: &StatementNode,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: Option<&[StoredLocalOrigins]>,
    inference: Option<&super::super::FrameInference>,
) -> Option<AggregateOrigins> {
    reference_leaves_before_statement_for_query(
        program, state, before, expression, expected, stored, inference, false,
    )
}

pub(in crate::machine_calls::calls::write_frames) fn reference_leaves_before_statement_for_query(
    program: &dyn crate::CheckingInput,
    state: &State,
    before: &StatementNode,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: Option<&[StoredLocalOrigins]>,
    inference: Option<&super::super::FrameInference>,
    include_shared: bool,
) -> Option<AggregateOrigins> {
    reference_leaves_with_projection(
        program,
        state,
        before,
        expression,
        expected,
        stored,
        inference,
        include_shared,
        ReferenceProjection::Origins,
    )
}

/// Result canonicalization must load each descriptor once. Keep its current
/// home until that canonicalizer consumes the frozen binding; returning its
/// already-resolved pointee here would load a later replacement a second time.
pub(in crate::machine_calls::calls::write_frames) fn reference_carriers_before_statement_for_query(
    program: &dyn crate::CheckingInput,
    state: &State,
    before: &StatementNode,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
) -> Option<AggregateOrigins> {
    reference_leaves_with_projection(
        program,
        state,
        before,
        expression,
        expected,
        Some(stored),
        None,
        include_shared,
        ReferenceProjection::DescriptorCarriers,
    )
}

#[derive(Clone, Copy)]
enum ReferenceProjection {
    Origins,
    DescriptorCarriers,
}

fn reference_leaves_with_projection(
    program: &dyn crate::CheckingInput,
    state: &State,
    before: &StatementNode,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: Option<&[StoredLocalOrigins]>,
    inference: Option<&super::super::FrameInference>,
    include_shared: bool,
    projection: ReferenceProjection,
) -> Option<AggregateOrigins> {
    let mut root_expression = expression;
    let root_name = loop {
        match program.expression_table.expression(root_expression) {
            ExpressionNode::Name(name) => break name,
            ExpressionNode::Member(member) => root_expression = member.receiver,
            ExpressionNode::Indexed(indexed) => root_expression = indexed.collection,
            _ => return None,
        }
    };
    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(program, expression);
    let place = facts.places.get(place);
    let root = place.root.as_symbol()?;
    if !root.is_valid() || root_name.head_symbol != root {
        return None;
    }
    let declaration = program.symbols.get(root);
    if declaration.parent != state.symbol {
        return None;
    }
    let parameter = (declaration.kind == SymbolKind::Parameter)
        .then(|| program.state_parameter_by_symbol(state, root))
        .flatten();
    let (source_name, source_reference) = if let Some(parameter) = parameter {
        (parameter.name.as_str(), parameter.type_reference)
    } else {
        if declaration.kind != SymbolKind::Local {
            return None;
        }
        let source = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take_while(|statement| !std::ptr::eq(*statement, before))
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == root => Some(local),
                _ => None,
            })?;
        (source.name.as_str(), source.type_reference)
    };
    // This query transports stored-leaf provenance, not permission to move
    // caller storage. A readable input exposes the same carrier inventory;
    // ownership-window checking separately requires restoration of its slot.
    let source_reference = if include_shared {
        super::super::reference_subjects::carrier_storage_type(program, source_reference)?
    } else {
        source_reference
    };
    let names = program
        .expression_table
        .name_path_members(root_name.members);
    if names.first()?.as_str() != source_name
        || program.symbols.name(root) != source_name
        || (names.len() == 1 && root_name.symbol != root)
    {
        return None;
    }
    let segments = facts.place_segments.span_or_empty(place.segments);
    let actual = projected_type(program, source_reference, segments)?;
    if !super::super::isolation::aggregate_storage_types_match(program, actual, expected) {
        return None;
    }
    // A transferred record supersedes the declared frontier: an assigned or
    // initialized binding already named its source selectors. Seeds and
    // frontier-only rows stay symbolic so a selected element can still
    // specialize an unknown index.
    let tracked = stored
        .and_then(|stored| stored.iter().find(|local| local.local_symbol == root))
        .or_else(|| inference.and_then(|inference| inference.local_origins(root)));
    let declared_origins = if tracked.is_none() && (parameter.is_some() || stored.is_none()) {
        let mut origins = super::type_origins::declared_origins_for_query(
            program,
            root,
            source_name,
            source_reference,
            include_shared,
        )?;
        if include_shared && stored.is_none() && inference.is_none() {
            // This raw argument query requests descriptor homes, not their
            // pointees. A local descriptor has the same symbolic identity as
            // an incoming one, even when its referee contains more pointers.
            // Its caller-prefix consumer must resolve the live contents; an
            // opaque region or unresolved stored row supplies no such fact.
            for leaf in &mut origins.references {
                if crate::reference_shell(
                    program,
                    projected_storage_type(program, source_reference, &leaf.local_segments)?,
                )
                .is_none()
                {
                    continue;
                }
                leaf.origin = Some(super::super::FramePlaceOrigin {
                    path: leaf.local_path.clone(),
                    precision: super::super::place_paths::coordinate_storage_path(
                        program,
                        source_name,
                        &leaf.local_segments,
                    )
                    .1,
                    source: super::super::FrameSourcePlace {
                        root,
                        segments: leaf.local_segments.clone(),
                        builtin_coordinates: true,
                        ..Default::default()
                    },
                });
            }
        }
        Some(origins)
    } else {
        None
    };
    let established = tracked.or(declared_origins.as_ref());
    let Some(established) = established else {
        return (super::super::type_is_caller_isolated_local(program, source_reference)
            && !segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. })))
        .then(AggregateOrigins::default);
    };
    let source = super::super::FrameSourcePlace::from_expression(program, expression);
    let mut projected = match projection {
        ReferenceProjection::Origins => {
            project_stored_origins(program, established, segments, established.symbolic)?
        }
        ReferenceProjection::DescriptorCarriers => {
            if source.root != root {
                return None;
            }
            project_stored_origins_with_sources(
                program,
                established,
                segments,
                established.symbolic,
                |_, leaf| {
                    // Unknown contents stay unknown. Opaque regions expose no
                    // descriptor boundary; never manufacture a load for them.
                    if leaf.origin.is_some()
                        && matches!(leaf.kind, super::StoredOriginKind::Reference(_))
                    {
                        let carrier = source.append_segments(&leaf.local_segments);
                        let (path, precision) = super::super::place_paths::coordinate_storage_path(
                            program,
                            source_name,
                            &carrier.segments,
                        );
                        leaf.origin =
                            Some(ReferenceSource::Symbolic(super::super::FramePlaceOrigin {
                                path,
                                precision,
                                source: carrier,
                            }));
                    }
                    Some(())
                },
            )?
        }
    };
    for leaf in &mut projected.references {
        if let Some(origin) = &mut leaf.origin {
            origin.place_mut().source.builtin_coordinates &= source.builtin_coordinates;
        }
    }
    for moved in &mut projected.moves {
        moved.source.builtin_coordinates &= source.builtin_coordinates;
    }
    Some(projected)
}

/// Project frozen or symbolic aggregate evidence after the caller validates
/// the root declaration and the selected type. Case presence remains part of
/// projection, including subtrees with no exclusive-reference leaves.
/// Callers transporting expression-derived selectors must conjoin their
/// builtin-coordinate flag into all returned reference and move sources.
pub(in crate::machine_calls::calls::write_frames) fn project_stored_origins(
    program: &dyn crate::CheckingInput,
    established: &StoredLocalOrigins,
    segments: &[PlaceSegment],
    symbolic: bool,
) -> Option<AggregateOrigins> {
    project_stored_origins_with_sources(program, established, segments, symbolic, |_, _| Some(()))
}

/// Project a completed aggregate through the ordinary stored-leaf geometry.
/// The roster index carries each completed descriptor association alongside
/// the shared selector/case specialization; path equality never reconstructs it.
pub(in crate::machine_calls::calls::write_frames) fn project_aggregate_origins(
    program: &dyn crate::CheckingInput,
    origins: AggregateOrigins,
    local_symbol: symbols::SymbolHandle,
    segments: &[PlaceSegment],
) -> Option<AggregateOrigins> {
    let sources = origins
        .references
        .iter()
        .map(|leaf| leaf.origin.clone())
        .collect::<Vec<_>>();
    let established = StoredLocalOrigins {
        local_symbol,
        references: origins
            .references
            .into_iter()
            .map(|leaf| super::StoredWriteOrigin {
                lifetime_dependencies: leaf.lifetime_dependencies,
                kind: leaf.kind,
                local_symbol,
                local_path: leaf.local_suffix,
                local_segments: leaf.local_segments,
                may_select: leaf.may_select,
                origin: leaf.origin.map(|origin| origin.place().clone()),
            })
            .collect(),
        cases: origins.cases,
        moves: origins.moves,
        symbolic: false,
    };
    project_stored_origins_with_sources(
        program,
        &established,
        segments,
        false,
        |index, projected| {
            if let Some(ReferenceSource::Captured(source)) = sources.get(index)? {
                let mut source = source.clone();
                source.source.origin = projected.origin.take()?.into_symbolic_place()?;
                source.base = source.source.origin.clone();
                projected.origin = Some(ReferenceSource::Captured(source));
            }
            Some(())
        },
    )
}

/// Rebase the same selected reference roster while its producer retains any
/// completed source association. The index is into the unchanged input roster,
/// including excluded/shadowed leaves; it is never inferred from path equality.
pub(in crate::machine_calls::calls::write_frames) fn project_stored_origins_with_sources(
    program: &dyn crate::CheckingInput,
    established: &StoredLocalOrigins,
    segments: &[PlaceSegment],
    symbolic: bool,
    mut retain_source: impl FnMut(usize, &mut ReferenceLeaf) -> Option<()>,
) -> Option<AggregateOrigins> {
    let mut leaves = AggregateOrigins::default();
    for (index, selected) in segments.iter().enumerate() {
        if let PlaceSegment::Case { .. } = selected {
            // A case-payload projection is a checked partial operation: it
            // traps unless the selected case is active, so every leaf it can
            // produce exists only on arms that actually select it. The query
            // therefore needs one possible matching case, not a universal
            // one — a sibling case arm contributes no writes through this
            // access. When no possible case matches, the projection is
            // unproven rather than evidence of a private payload.
            if !established.cases.iter().any(|case| {
                case.len() == index + 1
                    && case[index] == *selected
                    && prefix_matches(&segments[..index], &case[..index])
            }) {
                return None;
            }
        }
    }
    for (source_index, prior) in established.references.iter().enumerate() {
        if !prefix_matches(segments, &prior.local_segments)
            || reference_is_shadowed(established, prior, segments)
        {
            continue;
        }
        // Type-derived rows describe every possible branch. A caller's
        // retained cases may exclude one, including a fixed element selected
        // from a type-derived unknown-element row.
        let mut selected_path = prior.local_segments.clone();
        for (candidate, selected) in selected_path.iter_mut().zip(segments) {
            if matches!(*candidate, PlaceSegment::Index { expression } if !expression.is_valid()) {
                *candidate = *selected;
            }
        }
        if !case_path_is_possible(&selected_path, &established.cases)? {
            continue;
        }
        let local_segments = prior.local_segments[segments.len()..].to_vec();
        let mut local_suffix = String::new();
        for segment in &local_segments {
            match segment {
                PlaceSegment::Field { symbol } => {
                    local_suffix.push('.');
                    local_suffix.push_str(program.symbols.name(*symbol));
                }
                PlaceSegment::Case { .. } => {}
                _ => break,
            }
        }
        let mut origin = prior.origin.clone();
        if let Some(origin) = &mut origin
            && (symbolic
                || (origin.source.root == established.local_symbol
                    && origin.source.segments == prior.local_segments))
        {
            // A type-derived wildcard describes all possible elements, but a
            // moved fixed projection selects one. Preserve that selector in
            // source evidence without narrowing the coarse write footprint.
            for (source, selected) in origin.source.segments.iter_mut().zip(segments) {
                if matches!(*source, PlaceSegment::Index { expression } if !expression.is_valid()) {
                    *source = *selected;
                }
            }
        }
        let mut projected = ReferenceLeaf {
            kind: prior.kind,
            local_suffix,
            may_select: prior.may_select
                && local_segments
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Index { .. })),
            local_segments,
            origin: origin.map(ReferenceSource::Symbolic),
            lifetime_dependencies: prior.lifetime_dependencies.clone(),
        };
        retain_source(source_index, &mut projected)?;
        leaves.references.push(projected);
    }
    for case in &established.cases {
        if prefix_matches(segments, case) {
            leaves.cases.push(case[segments.len()..].to_vec());
        }
    }
    for moved in &established.moves {
        if prefix_matches(segments, &moved.local_segments) {
            leaves.moves.push(AggregateMove {
                local_segments: moved.local_segments[segments.len()..].to_vec(),
                source: moved.source.clone(),
                type_reference: moved.type_reference,
            });
        } else if prefix_matches(&moved.local_segments, segments) {
            let remaining = &segments[moved.local_segments.len()..];
            leaves.moves.push(AggregateMove {
                local_segments: Vec::new(),
                source: moved.source.append_segments(remaining),
                type_reference: projected_type(program, moved.type_reference, remaining)?,
            });
        }
    }
    // Zero leaves follows a validated shape/projection, not failed lookup.
    // Symbolic paths still require caller-prefix expansion; only the frozen
    // transfer has already accounted for the preceding declarations here.
    Some(leaves)
}

/// An excluded outer branch makes all of its inner rows absent. Otherwise,
/// missing container evidence is incomplete, not proof of a reference-free
/// subtree. Runtime selectors retain every matching possible branch.
fn case_path_is_possible(path: &[PlaceSegment], cases: &[Vec<PlaceSegment>]) -> Option<bool> {
    for (index, selected) in path.iter().enumerate() {
        if !matches!(selected, PlaceSegment::Case { .. }) {
            continue;
        }
        let mut candidates = cases.iter().filter(|case| {
            case.len() == index + 1 && prefix_matches(&path[..index], &case[..index])
        });
        let first = candidates.next()?;
        if first[index] != *selected && !candidates.any(|case| case[index] == *selected) {
            return Some(false);
        }
    }
    Some(true)
}

pub(super) fn prefix_matches(selected: &[PlaceSegment], candidate: &[PlaceSegment]) -> bool {
    selected.len() <= candidate.len()
        && selected.iter().zip(candidate).all(|(selected, candidate)| {
            selected == candidate
                || matches!(
                    (selected, candidate),
                    (PlaceSegment::Index { .. }, PlaceSegment::FixedIndex { .. })
                )
                // Type-derived parameter leaves use an unknown element, not
                // a fabricated executable index. Exact/dynamic projections
                // both overlap it; projected_type separately checks bounds.
                || matches!((selected, candidate),
                    (PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
                     PlaceSegment::Index { expression }) if !expression.is_valid())
        })
}

/// Containment, unlike overlap, can remove an old row. A type-derived unknown
/// element covers every concrete element, but replacing one concrete element
/// never deletes the unknown-element default for the rest of the collection.
pub(in crate::machine_calls::calls::write_frames) fn path_covers(
    container: &[PlaceSegment],
    selected: &[PlaceSegment],
) -> bool {
    container.len() <= selected.len()
        && container.iter().zip(selected).all(|(container, selected)| {
            container == selected
                || matches!(container, PlaceSegment::Index { expression } if !expression.is_valid())
                    && matches!(
                        selected,
                        PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
                    )
        })
}

/// Exact stored rows overlay the incoming slice inventory. A fixed projection
/// sees its replacement, while a dynamic projection still sees every possible
/// source. Unknown replacement rows shadow defaults too: missing knowledge is
/// not permission to revive an overwritten incoming descriptor.
pub(super) fn reference_is_shadowed(
    record: &StoredLocalOrigins,
    leaf: &super::StoredWriteOrigin,
    selected: &[PlaceSegment],
) -> bool {
    // Weak writes are possible contents, not fallback contents. A fixed write
    // cannot erase a transfer possibly held elsewhere in the same collection.
    if leaf.may_select {
        return false;
    }
    let path = &leaf.local_segments;
    let mut projected = path.to_vec();
    for (segment, selected) in projected.iter_mut().zip(selected) {
        if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid()) {
            *segment = *selected;
        }
    }
    record.references.iter().any(|candidate| {
        !candidate.may_select
            && candidate.local_segments.len() <= path.len()
            && candidate.local_segments != path[..candidate.local_segments.len()]
            && path_covers(
                &path[..candidate.local_segments.len()],
                &candidate.local_segments,
            )
            && path_covers(&candidate.local_segments, &projected)
    })
}

/// Read one normal-exit slot, falling back to its incoming inventory only when
/// no exact overlay covers it. Default queries do not absorb concrete overrides;
/// the effect solver queries those independently on every reachable exit.
pub(in crate::machine_calls::calls::write_frames) fn slot_origins(
    record: &StoredLocalOrigins,
    selected: &[PlaceSegment],
) -> Option<Vec<super::FramePlaceOrigin>> {
    selected_slot_origins(record, selected, None)
}

fn selected_slot_origins(
    record: &StoredLocalOrigins,
    selected: &[PlaceSegment],
    may_select: Option<bool>,
) -> Option<Vec<super::FramePlaceOrigin>> {
    selected_slot_sources(record, selected, may_select)?
        .into_iter()
        .map(|source| {
            (source.lifetime_access.is_none() && source.origin.source.output_identity().is_none())
                .then_some(source.origin)
        })
        .collect()
}

/// Capture a slot's full debt, keeping opaque lifetimes separate from spatial
/// provenance. Spatial queries above deliberately refuse opaque rows.
pub(in crate::machine_calls::calls::write_frames) fn selected_slot_sources(
    record: &StoredLocalOrigins,
    selected: &[PlaceSegment],
    may_select: Option<bool>,
) -> Option<Vec<super::super::stored_calls::SlotSource>> {
    use super::super::stored_calls::SlotSource;
    let mut origins = Vec::new();
    for leaf in &record.references {
        if may_select.is_some_and(|selected| selected != leaf.may_select)
            || leaf.local_segments.len() != selected.len()
            || !path_covers(&leaf.local_segments, selected)
            || reference_is_shadowed(record, leaf, selected)
        {
            continue;
        }
        for dependency in &leaf.lifetime_dependencies {
            if !origins.contains(dependency) {
                origins.push(dependency.clone());
            }
        }
        let Some(mut origin) = leaf.origin.clone() else {
            if leaf.lifetime_dependencies.is_empty() {
                return None;
            }
            continue;
        };
        if origin.source.root == record.local_symbol
            && origin.source.segments == leaf.local_segments
        {
            origin.source.segments = selected.to_vec();
        }
        let source = SlotSource {
            carrier_backing: None,
            carrier: origin.source.clone(),
            origin,
            lifetime_access: None,
        };
        if !origins.contains(&source) {
            origins.push(source);
        }
    }
    (!origins.is_empty()).then_some(origins)
}

pub(in crate::machine_calls::calls::write_frames) fn projected_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    let reference = projected_storage_type(program, reference, segments)?;
    (!super::super::type_reference_is_reference(program, reference)).then_some(reference)
}

/// Walk owned structure up to a slot, without dereferencing a reference leaf.
/// Exact-reference demand may then use separately frozen evidence for that slot.
pub(in crate::machine_calls::calls::write_frames) fn projected_storage_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    let mut selected_case = None;
    // Projection and inventory share exact declaration scopes. Finite generic
    // preparation does not promise a materialized row for every nested field.
    let mut scopes = DeclarationScopes::default();
    let mut scope = 0;
    for segment in segments {
        (reference, scope) = scopes.head(program, reference, scope);
        reference = unconstrained_type(program, reference)?;
        match (
            program.type_reference_table.type_reference(reference),
            segment,
        ) {
            (
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                PlaceSegment::Case { variant },
            ) => {
                if selected_case.is_some()
                    || program.symbols.get(*variant).kind != SymbolKind::Variant
                {
                    return None;
                }
                let (definition, nested_scope) = scopes.definition(program, reference, scope)?;
                scope = nested_scope;
                selected_case =
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(candidate) if candidate.symbol == *variant => {
                                Some(candidate)
                            }
                            _ => None,
                        });
                selected_case?;
            }
            (
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                PlaceSegment::Field { symbol: field },
            ) => {
                if program.symbols.get(*field).kind != SymbolKind::Field {
                    return None;
                }
                reference = if let Some(variant) = selected_case.take() {
                    program
                        .data_payload_field_by_symbol(variant, *field)?
                        .type_reference
                } else {
                    let (definition, nested_scope) =
                        scopes.definition(program, reference, scope)?;
                    scope = nested_scope;
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(candidate) if candidate.symbol == *field => {
                                Some(candidate.type_reference)
                            }
                            _ => None,
                        })?
                };
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
                index,
            ) => {
                if selected_case.is_some() {
                    return None;
                }
                match index {
                    PlaceSegment::FixedIndex { index } if index < length => {}
                    PlaceSegment::Index { expression }
                        if *length > 0
                            && program
                                .expression_table
                                .constant_integer_value(*expression)
                                .is_none()
                            && !matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Range(_)
                            ) => {}
                    _ => return None,
                }
                reference = *element_type;
            }
            (
                TypeReferenceNode::Slice { element_type },
                PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
            ) => {
                if selected_case.is_some() {
                    return None;
                }
                // Storage shape supplies the element type, never evidence
                // that an element exists. Executable access still needs the
                // selected descriptor's checked bound and loan authority.
                reference = *element_type;
            }
            // Never peel a reference to turn a loaded carrier into an owned
            // value move. Reference result relations have their own evidence.
            _ => return None,
        }
    }
    if selected_case.is_some() {
        return None;
    }
    unconstrained_type(program, scopes.head(program, reference, scope).0)
}

fn unconstrained_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    reference.is_valid().then_some(reference)
}
