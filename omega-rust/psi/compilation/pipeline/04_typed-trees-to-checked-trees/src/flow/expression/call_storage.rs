//! Project body-derived call storage onto actual evaluated operands. Both
//! installed slots and returned descriptors use this operation; neither may
//! replay an initializer or confuse a descriptor cell with its pointee.
use super::Execution;
use arena::Handle;
use checked_trees::FlowOperandFact;
use facts::PlaceSegment;
use validation::CanonicalPlace;

pub(super) struct CallPlaceProjection {
    pub(super) addresses: Vec<CanonicalPlace>,
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
}

impl Execution<'_, '_, '_> {
    /// Stop at the descriptor slot. Only intermediate reference crossings
    /// load descriptors, and each such load uses the same pre-call snapshot.
    pub(super) fn project_call_place(
        &mut self,
        actual: &FlowOperandFact,
        path: &[PlaceSegment],
        at: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<CallPlaceProjection> {
        let mut captured_actual = Handle::default();
        let mut reference = actual.type_reference;
        let mut places = if let Some(shell) = validation::reference_shell(self.program, reference) {
            reference = shell.referee;
            let places = self.captured_referent_places(actual.referents);
            if places.is_empty() {
                return None;
            }
            places
        } else {
            vec![validation::canonical_place_from_expression_in_state(
                self.program,
                self.state.symbol,
                self.statement_index,
                actual.expression,
            )?]
        };
        for (ordinal, segment) in path.iter().enumerate() {
            if matches!(
                segment,
                PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange
            ) {
                validation::collection_element_type_reference(self.program, reference)?;
            } else {
                reference = validation::project_type_reference_from_segments(
                    self.program,
                    reference,
                    std::slice::from_ref(segment),
                )?;
            }
            places = places
                .into_iter()
                .map(|place| {
                    super::super::operand_values::project_captured_view_place(
                        place,
                        std::slice::from_ref(segment),
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            if ordinal + 1 < path.len()
                && let Some(shell) = validation::reference_shell(self.program, reference)
            {
                reference = shell.referee;
                // The evaluated value owns its inline descriptor children. A
                // constructor temporary need not have an installed storage home.
                // The shared selector preserves those operand epochs and switches
                // to the current cut only after crossing an actual reference.
                if !captured_actual.is_valid() {
                    captured_actual = self.context.control.operands.insert(*actual);
                }
                let selected = validation::completed_storage::reference_captures_at_path(
                    self.program,
                    &self.context.control,
                    captured_actual,
                    &path[..=ordinal],
                    at,
                )?
                .captures()?;
                let mut pointees = Vec::new();
                for selected in selected {
                    // Lifetime-only subtree evidence supplies no address.
                    if !selected.projection.is_empty() {
                        return None;
                    }
                    let descriptor = self.context.control.operands.get(selected.capture);
                    let addresses = self.captured_referent_places(descriptor.referents);
                    if addresses.is_empty() {
                        return None;
                    }
                    for place in addresses {
                        if !pointees.contains(&place) {
                            pointees.push(place);
                        }
                    }
                }
                places = pointees;
            }
        }
        Some(CallPlaceProjection {
            addresses: places,
            type_reference: reference,
        })
    }

    pub(super) fn call_backing_values(
        &mut self,
        actual: &FlowOperandFact,
        source: &validation::LocalWriteSource,
        destination: typed_trees::types::TypeReferenceHandle,
        at: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<Vec<(FlowOperandFact, Vec<PlaceSegment>)>> {
        let desired = validation::reference_shell(self.program, destination)?;
        if source.recasts.is_empty() && source.representation.is_none() {
            if source.segments.is_empty()
                && validation::reference_shell(self.program, actual.type_reference).is_some()
            {
                return self.call_slot_values(actual, &[], at);
            }
            let projected = self.project_call_place(actual, &source.segments, at)?;
            let window = source.segments.last().is_some_and(|segment| {
                matches!(
                    segment,
                    PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange
                )
            });
            if !window
                && validation::reference_shell(self.program, projected.type_reference).is_some()
            {
                return self.call_slot_values(actual, &source.segments, at);
            }
            let (actual_type, expected_type) = if window {
                (
                    validation::collection_element_type_reference(
                        self.program,
                        projected.type_reference,
                    )?,
                    validation::collection_element_type_reference(self.program, desired.referee)?,
                )
            } else {
                (projected.type_reference, desired.referee)
            };
            if !validation::storage_origin_types_match(self.program, actual_type, expected_type) {
                return None;
            }
            let referents = self.append_referent_places(&projected.addresses);
            let retention = self.retention_from_referents(referents);
            return Some(vec![(
                FlowOperandFact {
                    type_reference: destination,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                    storage_snapshot: at,
                    referents,
                    retention,
                    ..Default::default()
                },
                Vec::new(),
            )]);
        }
        // A checked presentation relation may establish a different view
        // without exporting executable geometry into the caller. Preserve
        // its backing obligations, not a guessed raw address or field walk.
        let roots = super::super::operand_values::captured_lifetime_roots(
            self.program,
            &self.context.control,
            actual,
        )?;
        let retention = self.append_retention(&roots);
        let places = Vec::new();
        let referents = self.append_referent_places(&places);
        Some(vec![(
            FlowOperandFact {
                type_reference: destination,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: at,
                referents,
                retention,
                ..Default::default()
            },
            Vec::new(),
        )])
    }

    pub(super) fn call_slot_values(
        &mut self,
        actual: &FlowOperandFact,
        path: &[PlaceSegment],
        at: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<Vec<(FlowOperandFact, Vec<PlaceSegment>)>> {
        if validation::project_type_reference_from_segments(
            self.program,
            actual.type_reference,
            path,
        )
        .is_some_and(|reference| validation::reference_shell(self.program, reference).is_some())
        {
            let actual = self.context.control.operands.insert(*actual);
            if let Some(stored) = validation::completed_storage::reference_captures_at_path(
                self.program,
                &self.context.control,
                actual,
                path,
                at,
            ) {
                return Some(
                    stored
                        .captures()?
                        .into_iter()
                        .map(|selected| {
                            let mut value = *self.context.control.operands.get(selected.capture);
                            if !selected.projection.is_empty() {
                                // An opaque descriptor subtree has lifetime debt but
                                // no captured pointer. Its containing home's address
                                // cannot stand in for the selected descriptor value.
                                value.referents = Default::default();
                            }
                            (value, selected.projection)
                        })
                        .collect(),
                );
            }
        }
        if path.is_empty() {
            return Some(vec![(*actual, Vec::new())]);
        }
        if validation::reference_shell(self.program, actual.type_reference).is_none()
            && let Some((selected, _)) = super::super::operand_values::captured_operand_at_path(
                self.program,
                &self.context.control,
                actual,
                actual.type_reference,
                path,
            )
        {
            return Some(vec![(*selected, Vec::new())]);
        }
        let projected = self.project_call_place(actual, path, at)?;
        let mut values = Vec::new();
        for place in projected.addresses {
            let stored = super::super::stored_values::at_snapshot(
                self.program,
                &self.context.control,
                at,
                &place,
                super::super::stored_values::StoredObservation::Binding,
            )?
            .captures()?;
            for selected in stored {
                values.push((
                    *self.context.control.operands.get(selected.capture),
                    selected.projection,
                ));
            }
        }
        (!values.is_empty()).then_some(values)
    }
}
