//! Transport frozen structural origins without replaying selector expressions.

use validation::CanonicalPlace;
use validation::canonical_place_type_reference;

use crate::flow::canonical_place_from_symbol;

use crate::flow::mutation::StorageRegion;
use crate::flow::mutation::local_origins::place_from_origin_path;
use crate::flow::place_segment_has_unresolved_identity;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OriginPrecision {
    Exact,
    /// Every structural dimension is retained, but an element is unknown.
    MaySelect,
    /// Only an enclosing region survived. Descendant projection is unsound.
    Enclosing,
}

/// Precision separates complete structural projection from exact identity.
/// A runtime element selector retains its known field suffix for conservative
/// overlap, but cannot feed value transport or
/// returned-reference evidence re-anchoring as a current selector value.
pub(crate) fn origin_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<(CanonicalPlace, OriginPrecision)> {
    let source = origin.source.as_ref()?;
    source_place(program, state, statement_index, source)
}

fn source_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::LocalWriteSource,
) -> Option<(CanonicalPlace, OriginPrecision)> {
    if source.invocation_output.is_some() {
        // Opaque output identity is not a declaration-relative write region.
        // Unknown physical aliasing must not fall back to its display label.
        return None;
    }
    if let Some(first) = source.recasts.first() {
        for recast in &source.recasts {
            if matches!(
                validation::reference_formation_source(program, recast.formation.expression())?
                    .interpretation,
                validation::ReferenceFormationInterpretation::ResidentPlaced { .. }
            ) {
                // A resident's Extent descriptor is its loan anchor, not the
                // bytes being written. Until backing identity is available,
                // its write footprint must remain unknown.
                return None;
            }
        }
        // A view's field symbols describe its interpretation, not the
        // declaration backing it. Without a translated leaf footprint, keep
        // the complete pre-view storage and prohibit descendant projection.
        // This also withholds value-transfer precision across a recast.
        let backing = validation::LocalWriteSource {
            root: first.root,
            segments: first.segments.clone(),
            recasts: Vec::new(),
            representation: None,
            ..source.clone()
        };
        let (place, _) = structural_origin(program, state, statement_index, &backing)?;
        return Some((place, OriginPrecision::Enclosing));
    }
    if source.representation.is_some() {
        return None;
    }
    if let Some(place) = structural_origin(program, state, statement_index, source) {
        return Some(place);
    }
    place_from_origin_path(program, state, statement_index, &source.path).map(|place| {
        (
            place,
            if source.collection_coarse {
                OriginPrecision::Enclosing
            } else {
                OriginPrecision::Exact
            },
        )
    })
}

/// Translate a callee's logical write into the captured actual's storage.
/// Exact scalar geometry preserves disjoint backing fields. When a view has
/// no exact leaf translation, its enclosing footprint is still a write;
/// that conservative answer never becomes value-preservation evidence.
pub(in crate::flow::mutation) fn write_source_region(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::LocalWriteSource,
    relative: &[facts::PlaceSegment],
) -> Option<StorageRegion> {
    if source.invocation_output.is_some() {
        return output_write_region(program, source, relative);
    }
    if !source.root.is_valid() {
        return None;
    }
    for recast in &source.recasts {
        if let validation::ReferenceViewFormation::ExtentView(declaration) = recast.formation {
            if recast.representation.is_some()
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
            let backing = validation::LocalWriteSource {
                root: recast.root,
                segments: recast.segments.clone(),
                recasts: Vec::new(),
                representation: None,
                ..source.clone()
            };
            let (owner, precision) = structural_origin(program, state, statement_index, &backing)?;
            if precision != OriginPrecision::Exact {
                return None;
            }
            return Some(StorageRegion::NonresidentContent {
                owner,
                formation: declaration,
            });
        }
        let formation =
            validation::reference_formation_source(program, recast.formation.expression())?;
        if !matches!(
            formation.interpretation,
            validation::ReferenceFormationInterpretation::ResidentPlaced { .. }
        ) {
            continue;
        }
        // Recast selectors belong to the content interpretation. The retained
        // pre-formation projection identifies the owner independently of them.
        // Until a content leaf has translated geometry, its whole content is
        // the conservative footprint, not a field of the Extent descriptor.
        let backing = validation::LocalWriteSource {
            root: recast.root,
            segments: recast.segments.clone(),
            recasts: Vec::new(),
            representation: None,
            ..source.clone()
        };
        let (owner, precision) = structural_origin(program, state, statement_index, &backing)?;
        if precision != OriginPrecision::Exact {
            return None;
        }
        let endpoint =
            canonical_place_type_reference(program, state.symbol, statement_index, &owner)?;
        let view = program
            .type_reference_table
            .peeled_type_reference(formation.view_type);
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(view)
        else {
            return None;
        };
        let mut plans = program
            .placed_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == *symbol);
        let (Some(plan), None) = (plans.next(), plans.next()) else {
            return None;
        };
        let checked_trees::CheckedReferenceStorage::ResidentPlaced {
            granted, resident, ..
        } = formation
            .interpretation
            .placed_storage(program, endpoint, plan)?
        else {
            return None;
        };
        return Some(StorageRegion::ResidentContent {
            owner,
            formation: recast.formation.expression(),
            granted,
            resident,
        });
    }
    if !source.recasts.is_empty() {
        let mut projected = source.clone();
        projected.segments.extend_from_slice(relative);
        if let Some((_, selectors)) = &mut projected.representation {
            selectors.extend_from_slice(relative);
        }
        if let Some(coordinate) = projected.scalar_storage_coordinate(program, state) {
            return Some(StorageRegion::Declared(coordinate.place));
        }
    }
    let (place, precision) = source_place(program, state, statement_index, source)?;
    if precision == OriginPrecision::Enclosing {
        return Some(StorageRegion::Declared(place));
    }
    crate::flow::project_captured_view_place(place, relative).map(StorageRegion::Declared)
}

/// The chronological origin producer already retained this performed output.
/// Rejoin its typed reference slot, without treating lifetime inputs or the
/// display path as an address. Its backing may alias other storage.
fn output_write_region(
    program: &dyn validation::CheckingInput,
    source: &validation::LocalWriteSource,
    relative: &[facts::PlaceSegment],
) -> Option<StorageRegion> {
    if source.is_image_storage || !source.recasts.is_empty() || source.representation.is_some() {
        return None;
    }
    let identity = source.invocation_output?;
    let parameter = validation::invocation_output_parameter(program, identity)?;
    let shell = validation::reference_shell(program, parameter.type_reference)?;
    if !shell.access.is_exclusive() {
        return None;
    }
    // The parameter's outer reference addresses its receiving storage. The
    // first reference below it is the actual output leaf, including a root
    // reference-valued slot. Later selectors stay inside that opaque referent.
    let length = (0..=source.segments.len()).find(|length| {
        validation::project_type_reference_from_segments(
            program,
            shell.referee,
            &source.segments[..*length],
        )
        .and_then(|reference| validation::reference_shell(program, reference))
        .is_some()
    })?;
    let mut projected = source.segments.clone();
    projected.extend_from_slice(relative);
    validation::project_type_reference_from_segments(program, shell.referee, &projected)?;
    Some(StorageRegion::UnresolvedContent {
        carrier: CanonicalPlace {
            root: facts::PlaceRoot::InvocationOutput(identity),
            segments: source.segments[..length].to_vec(),
        },
    })
}

fn structural_origin(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::LocalWriteSource,
) -> Option<(CanonicalPlace, OriginPrecision)> {
    let mut place = canonical_place_from_symbol(source.root)?;
    canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    let mut exact = true;
    for &segment in &source.segments {
        if place_segment_has_unresolved_identity(segment) {
            return None;
        }
        // Classify a live range before erasing its invocation-local selector.
        // It keeps a collection dimension, but supplies no extent or address.
        let segment = match segment {
            facts::PlaceSegment::Index { expression }
                if program.expression_table.expression_is_valid(expression)
                    && program.expression_table.range_node(expression).is_some() =>
            {
                facts::PlaceSegment::OpaqueRange
            }
            _ => segment,
        };
        if let facts::PlaceSegment::Index { expression } = segment
            && !program.expression_table.expression_is_valid(expression)
        {
            // A window retains the collection dimension; an erased selector
            // does not establish whether it was a window or one element.
            // Stop before that coordinate rather than shifting later fields
            // or indexes onto the wrong dimension of the backing storage.
            return Some((place, OriginPrecision::Enclosing));
        }
        // Overlap treats a runtime selector as any element, independently of
        // its later value. Keep the known descendant fields: an unknown row
        // does not make a write to its second field touch its first field.
        // Exact consumers must still refuse this captured selector identity.
        exact &= !matches!(
            segment,
            facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
        );
        place.segments.push(segment);
        crate::flow::compose_captured_view_segments(&mut place.segments)?;
        origin_type_reference(program, state, statement_index, &place)?;
    }
    Some((
        place,
        if exact {
            OriginPrecision::Exact
        } else {
            OriginPrecision::MaySelect
        },
    ))
}

/// Value and descriptor equality require the original resolved namespace.
/// Conservative write paths may name enclosing bytes, but a recast can change
/// element count and field interpretation without changing their address.
pub(super) fn exact_origin_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<CanonicalPlace> {
    let source = origin.source.as_ref()?;
    if !source.recasts.is_empty() || source.representation.is_some() {
        return None;
    }
    let (place, precision) = structural_origin(program, state, statement_index, source)?;
    (precision == OriginPrecision::Exact).then_some(place)
}

/// Translate a backing write into a captured alias's coordinates. A window
/// shares its final dimension with the backing selector; it does not consume
/// that selector from the alias path. Unknown offsets preserve an unknown
/// element/window and the known descendant suffix, never a later selector value.
pub(in crate::flow) fn alias_projection(
    captured: &[facts::PlaceSegment],
    storage: &[facts::PlaceSegment],
) -> Option<Vec<facts::PlaceSegment>> {
    use facts::PlaceSegment;
    let mut suffix = storage.get(captured.len()..)?.to_vec();
    let Some(window @ (PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange)) =
        captured.last().copied()
    else {
        return Some(suffix);
    };
    let selected = *storage.get(captured.len().checked_sub(1)?)?;
    let unknown_element = PlaceSegment::Index {
        expression: typed_trees::expression::ExpressionHandle::invalid(),
    };
    let relative = match (window, selected) {
        (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index })
            if index >= start && index < end =>
        {
            PlaceSegment::FixedIndex {
                index: index.checked_sub(start)?,
            }
        }
        (
            PlaceSegment::FixedRange { start, end },
            PlaceSegment::FixedRange {
                start: selected_start,
                end: selected_end,
            },
        ) => {
            let overlap_start = start.max(selected_start);
            let overlap_end = end.min(selected_end);
            if overlap_start >= overlap_end {
                return None;
            }
            PlaceSegment::FixedRange {
                start: overlap_start.checked_sub(start)?,
                end: overlap_end.checked_sub(start)?,
            }
        }
        (PlaceSegment::OpaqueRange, PlaceSegment::FixedIndex { .. })
        | (_, PlaceSegment::Index { .. }) => unknown_element,
        (_, PlaceSegment::OpaqueRange)
        | (PlaceSegment::OpaqueRange, PlaceSegment::FixedRange { .. }) => PlaceSegment::OpaqueRange,
        _ => return None,
    };
    suffix.insert(0, relative);
    Some(suffix)
}

/// A frozen window is an exact descriptor origin, not an unknown element.
/// It retains its collection dimension; the type query below checks the
/// underlying carrier without pretending the window selected one child.
fn origin_type_reference(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut carrier = place.clone();
    if let Some(
        segment @ (facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange),
    ) = carrier.segments.last().copied()
    {
        if let facts::PlaceSegment::FixedRange { start, end } = segment
            && start > end
        {
            return None;
        }
        carrier.segments.pop();
        let reference =
            canonical_place_type_reference(program, state.symbol, statement_index, &carrier)?;
        validation::collection_element_type_reference(program, reference)?;
        Some(reference)
    } else {
        canonical_place_type_reference(program, state.symbol, statement_index, place)
    }
}

/// The sole whole-local write origin of `local_symbol` before `statement`,
/// restricted to referents rooted outside the local's own storage. The range
/// referent lookup and the projected-binding walk share this admission.
pub(crate) fn sole_local_referent_origin(
    resolver: &validation::CallFrameResolver<'_>,
    machine: &typed_trees::machine::Machine,
    statement: &typed_trees::statement::StatementNode,
    local_symbol: symbols::SymbolHandle,
) -> Option<validation::LocalWriteOrigin> {
    let mut referents = resolver
        .local_write_origins_before_statement(machine, statement)?
        .into_iter()
        .filter(|origin| {
            origin.local_symbol == local_symbol
                && origin.local_segments.is_empty()
                && origin
                    .source
                    .as_ref()
                    .is_none_or(|source| source.root != local_symbol)
        });
    let origin = referents.next()?;
    (referents.next().is_none()).then_some(origin)
}

#[cfg(test)]
mod tests {
    use super::alias_projection;
    use facts::PlaceSegment::{self, FixedIndex, FixedRange, OpaqueRange};

    #[test]
    fn unknown_window_keeps_alias_row_and_descendant_column() {
        let projected = alias_projection(
            &[OpaqueRange],
            &[FixedIndex { index: 0 }, FixedIndex { index: 1 }],
        )
        .unwrap();
        assert!(
            matches!(projected[0], PlaceSegment::Index { expression } if !expression.is_valid())
        );
        assert_eq!(projected[1..], [FixedIndex { index: 1 }]);
    }

    #[test]
    fn known_window_translates_only_its_boundary_dimension() {
        assert_eq!(
            alias_projection(
                &[FixedRange { start: 2, end: 6 }],
                &[FixedIndex { index: 3 }, FixedRange { start: 1, end: 4 }],
            ),
            Some(vec![
                FixedIndex { index: 1 },
                FixedRange { start: 1, end: 4 }
            ]),
        );
        assert_eq!(
            alias_projection(
                &[FixedRange { start: 2, end: 6 }],
                &[FixedRange { start: 1, end: 4 }, FixedIndex { index: 1 }],
            ),
            Some(vec![
                FixedRange { start: 0, end: 2 },
                FixedIndex { index: 1 }
            ]),
        );
    }
}
