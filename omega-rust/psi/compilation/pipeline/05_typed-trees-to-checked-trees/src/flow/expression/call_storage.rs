//! Project body-derived call storage onto actual evaluated operands. Both
//! installed slots and returned descriptors use this operation; neither may
//! replay an initializer or confuse a descriptor cell with its pointee.
use super::Execution;
use crate::published::places::place_at;
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowStorageSnapshot};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;
use validation::LocalWriteSource;
use validation::collection_element_type_reference;
use validation::completed_storage::reference_captures_at_path;
use validation::completed_storage::{StoredOperand, reference_sources_at_path};
use validation::project_type_reference_from_segments;
use validation::reference_shell;
use validation::storage_origin_types_match;

pub(super) struct CallPlaceProjection {
    pub(super) addresses: Vec<super::super::CapturedReferencePlace>,
    pub(super) type_reference: TypeReferenceHandle,
    sources: StoredOperand,
    projection: Vec<PlaceSegment>,
}

impl Execution<'_, '_, '_> {
    /// Stop at the descriptor slot. Only intermediate reference crossings
    /// load descriptors, and each such load uses the same pre-call snapshot.
    pub(super) fn project_call_place(
        &mut self,
        actual: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        at: Handle<FlowStorageSnapshot>,
    ) -> Option<CallPlaceProjection> {
        let captured_actual = actual;
        let actual = *self.context.control.operands.get(actual);
        let mut sources = StoredOperand::Value {
            capture: captured_actual,
            reference: actual.type_reference,
            projection: Vec::new(),
            slot: None,
        };
        let mut descriptor_end = 0;
        let mut reference = actual.type_reference;
        let mut places = if let Some(shell) = reference_shell(self.program, reference) {
            reference = shell.referee;
            let places = self.captured_reference_places(actual.referents);
            // An authenticated returned reference can intentionally withhold
            // absolute addresses. Keep its typed source; empty geometry is
            // unknown here and never grants borrowing permission.
            places
        } else {
            vec![super::super::CapturedReferencePlace::unformed(place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                actual.expression,
            )?)]
        };
        for (ordinal, segment) in path.iter().enumerate() {
            if matches!(
                segment,
                PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange
            ) {
                collection_element_type_reference(self.program, reference)?;
            } else {
                reference = project_type_reference_from_segments(
                    self.program,
                    reference,
                    std::slice::from_ref(segment),
                )?;
            }
            places = places
                .into_iter()
                .map(|place| place.projected(std::slice::from_ref(segment)))
                .collect::<Option<Vec<_>>>()?;
            if ordinal + 1 < path.len()
                && let Some(shell) = reference_shell(self.program, reference)
            {
                reference = shell.referee;
                // The evaluated value owns its inline descriptor children. A
                // constructor temporary need not have an installed storage home.
                // The shared selector preserves those operand epochs and switches
                // to the current cut only after crossing an actual reference.
                sources = reference_sources_at_path(
                    self.program,
                    &self.context.control,
                    captured_actual,
                    &path[..=ordinal],
                    at,
                );
                descriptor_end = ordinal + 1;
                places = super::super::stored_views::reference_places_at_snapshot(
                    self.program,
                    &self.context.control,
                    captured_actual,
                    &path[..=ordinal],
                    at,
                    |place| self.is_incoming_descriptor_place(place),
                )
                .unwrap_or_default();
            }
        }
        Some(CallPlaceProjection {
            addresses: places,
            type_reference: reference,
            sources,
            projection: path[descriptor_end..].to_vec(),
        })
    }

    pub(super) fn call_backing_values(
        &mut self,
        actual: Handle<FlowOperandFact>,
        source: &LocalWriteSource,
        destination: TypeReferenceHandle,
        at: Handle<FlowStorageSnapshot>,
    ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
        let actual_value = *self.context.control.operands.get(actual);
        let desired = reference_shell(self.program, destination)?;
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE call-backing-value-input state={:?} statement={} actual={actual:?} actual_type={:?} destination={destination:?} source_root={:?} source_path={:?} recasts={:?} representation={:?}",
                self.state.symbol,
                self.statement_index,
                actual_value.type_reference,
                source.root,
                source.segments,
                source.recasts,
                source.representation
            );
        }
        if source.recasts.is_empty() && source.representation.is_none() {
            if source.segments.is_empty()
                && reference_shell(self.program, actual_value.type_reference).is_some()
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
            if !window && reference_shell(self.program, projected.type_reference).is_some() {
                return self.call_slot_values(actual, &source.segments, at);
            }
            let (actual_type, expected_type) = if window {
                (
                    collection_element_type_reference(self.program, projected.type_reference)?,
                    collection_element_type_reference(self.program, desired.referee)?,
                )
            } else {
                (projected.type_reference, desired.referee)
            };
            if !storage_origin_types_match(self.program, actual_type, expected_type) {
                return None;
            }
            let referents = self.append_captured_references(&projected.addresses);
            let sources = projected.sources.captures()?;
            if sources.is_empty() {
                return None;
            }
            let mut roots = Vec::new();
            for source in &sources {
                let value = self.context.control.operands.get(source.capture);
                let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                    self.program,
                    &self.context.control,
                    value,
                    &source.projection,
                )?;
                if profiles.is_empty() {
                    return None;
                }
                for profile in profiles {
                    for root in self.context.control.retained_roots(profile)? {
                        crate::shared::collections::push_unique(&mut roots, *root);
                    }
                }
            }
            // Even a proved empty local lifetime frontier needs a profile to
            // retain its checked source relation. Image-backed references have
            // no activation-relative absolute address to publish.
            let retention = self.append_retention(&roots);
            // The last crossed descriptor lends this view. Earlier inline
            // values retain their argument epoch; a descriptor reached through
            // a pointer was selected at the invocation-entry storage cut.
            // An unavailable alternative withholds the complete association.
            let reborrow_sources = sources
                .into_iter()
                .map(|source| checked_trees::FlowOperandReborrowSource {
                    operand: source.capture,
                    source_path: self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(source.projection),
                    projection: self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(projected.projection.iter().copied()),
                })
                .collect::<Vec<_>>();
            let reborrow_sources = self
                .context
                .control
                .operand_reborrow_sources
                .insert_many(reborrow_sources);
            if self.context.control.operand_retentions.is_valid(retention) {
                self.context
                    .control
                    .operand_retentions
                    .get_mut(retention)
                    .reborrow_sources = reborrow_sources;
            }
            let value = self.context.control.operands.insert(FlowOperandFact {
                type_reference: destination,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: at,
                referents,
                retention,
                ..Default::default()
            });
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE call-backing-value-created kind=reborrow state={:?} statement={} value={value:?} actual={actual:?} retention={retention:?} reborrow_sources={reborrow_sources:?}",
                    self.state.symbol, self.statement_index
                );
            }
            return Some(vec![(value, Vec::new())]);
        }
        // A checked presentation relation may establish a different view
        // without exporting executable geometry into the caller. Preserve
        // its backing obligations, not a guessed raw address or field walk.
        let roots = super::super::operand_values::captured_lifetime_roots(
            self.program,
            &self.context.control,
            &actual_value,
        )?;
        let retention = self.append_retention(&roots);
        let places = Vec::new();
        let referents = self.append_referent_places(&places);
        let value = self.context.control.operands.insert(FlowOperandFact {
            type_reference: destination,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            storage_snapshot: at,
            referents,
            retention,
            ..Default::default()
        });
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE call-backing-value-created kind=presentation state={:?} statement={} value={value:?} actual={actual:?} retention={retention:?}",
                self.state.symbol, self.statement_index
            );
        }
        Some(vec![(value, Vec::new())])
    }

    pub(super) fn call_slot_values(
        &mut self,
        actual: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        at: Handle<FlowStorageSnapshot>,
    ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
        let actual_value = *self.context.control.operands.get(actual);
        if project_type_reference_from_segments(self.program, actual_value.type_reference, path)
            .is_some_and(|reference| reference_shell(self.program, reference).is_some())
        {
            if let Some(stored) =
                reference_captures_at_path(self.program, &self.context.control, actual, path, at)
            {
                return Some(
                    stored
                        .captures()?
                        .into_iter()
                        .map(|selected| {
                            (
                                self.withhold_projected_pointer(
                                    selected.capture,
                                    &selected.projection,
                                ),
                                selected.projection,
                            )
                        })
                        .collect(),
                );
            }
            // An unchanged input descriptor may retain an opaque pointer. Its
            // enclosing home's address cannot replace that selected pointer.
            return Some(vec![(
                self.withhold_projected_pointer(actual, path),
                path.to_vec(),
            )]);
        }
        if path.is_empty() {
            return Some(vec![(actual, Vec::new())]);
        }
        if reference_shell(self.program, actual_value.type_reference).is_none()
            && let Some(selected) = super::super::operand_values::captured_operand_handle_at_path(
                self.program,
                &self.context.control,
                actual,
                actual_value.type_reference,
                path,
            )
        {
            // This owned child is an existing evaluated occurrence.
            return Some(vec![(selected, Vec::new())]);
        }
        let projected = self.project_call_place(actual, path, at)?;
        let mut values = Vec::new();
        for place in projected.addresses {
            let stored = super::super::stored_values::at_snapshot(
                self.program,
                &self.context.control,
                at,
                &place.place,
                super::super::stored_values::StoredObservation::Binding,
            )?
            .captures()?;
            for selected in stored {
                values.push((selected.capture, selected.projection));
            }
        }
        (!values.is_empty()).then_some(values)
    }

    fn withhold_projected_pointer(
        &mut self,
        source: Handle<FlowOperandFact>,
        projection: &[PlaceSegment],
    ) -> Handle<FlowOperandFact> {
        if projection.is_empty() {
            return source;
        }
        // This opaque subtree has lifetime debt but no captured pointer.
        // Keep its exact source association without exposing the container's
        // address as the reference stored inside it.
        let mut value = *self.context.control.operands.get(source);
        value.referents = Default::default();
        value.value_source = source;
        self.context.control.operands.insert(value)
    }
}
