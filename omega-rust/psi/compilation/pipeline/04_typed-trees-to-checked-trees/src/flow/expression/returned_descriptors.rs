//! Preserve exact body-derived descriptors or admitted local dispatch regions.
//! Region coverage is exclusion-only; it does not manufacture an exact pointer.
//! The body-derived association retains evaluated
//! explicit actuals, after receiver separation. Declared lifetime relations still retain every owed input; the
//! body relation supplies geometry only. Its observation discards pre-call
//! predicates and pointee snapshots, so later field reads use current storage.
//! Body-relative source coordinates select the invocation-entry descriptor.
//! A callee can replace its containing slot before returning an earlier saved
//! descriptor; re-reading that slot at return would select a different pointer.
use super::Execution;
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowOperandRetention};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

impl Execution<'_, '_, '_> {
    pub(super) fn capture_returned_descriptor(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        returned: TypeReferenceHandle,
        path: &[PlaceSegment],
        retention: Handle<FlowOperandRetention>,
        source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Handle<FlowOperandFact> {
        self.returned_descriptor(call, inputs, returned, path, retention, source_snapshot)
            .or_else(|| {
                self.capture_returned_region_descriptor(
                    call,
                    inputs,
                    returned,
                    path,
                    retention,
                    source_snapshot,
                )
            })
            .unwrap_or_default()
    }

    fn returned_descriptor(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        returned: TypeReferenceHandle,
        path: &[PlaceSegment],
        retention: Handle<FlowOperandRetention>,
        source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
    ) -> Option<Handle<FlowOperandFact>> {
        let target = validation::call_target_symbol(self.program, call);
        let state = validation::call_target_state(self.program, target)?;
        let parameters = self.program.state_parameters(state);
        let sources = self
            .context
            .call_frames?
            .reference_result_leaf_origins(state.symbol)?;
        let reference =
            validation::project_type_reference_from_segments(self.program, returned, path)?;
        let expected = validation::reference_shell(self.program, reference)?;
        let mut roots = self.context.control.retained_roots(retention)?.to_vec();
        let mut places = Vec::new();
        let mut candidates = Vec::new();
        let mut has_unknown_geometry = false;
        let mut all_image = true;
        for source in sources {
            if self.returned_slot_path(returned, &source.result_path)? != path {
                continue;
            }
            if source.source.is_image_storage {
                has_unknown_geometry = true;
                candidates.push((Vec::new(), true, None));
                continue;
            }
            // An indexed descriptor home coarsens the may-write footprint,
            // even after its loaded value resolves to an exact supplier. The
            // structural source below owns descriptor identity; the footprint
            // cannot veto it or stand in for its path/type/access checks.
            if !source.source.recasts.is_empty()
                || source.source.representation.is_some()
                || !source
                    .source
                    .segments
                    .iter()
                    .all(super::super::place_segment_has_static_identity)
            {
                return None;
            }
            let position =
                typed_trees::signature::state_parameter_position(parameters, source.source.root)?;
            let (_, actual) = inputs.get(position)?;
            let values =
                self.call_backing_values(actual, &source.source, reference, source_snapshot)?;
            for (value, projection) in values {
                let supplied = validation::project_type_reference_from_segments(
                    self.program,
                    value.type_reference,
                    &projection,
                )?;
                let supplied = validation::reference_shell(self.program, supplied)?;
                if !(supplied.access == expected.access
                    || supplied.access == language_semantics::ReferenceAccess::Mutable)
                    || !validation::storage_origin_types_match(
                        self.program,
                        supplied.referee,
                        expected.referee,
                    )
                {
                    return None;
                }
                let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                    self.program,
                    &self.context.control,
                    &value,
                    &projection,
                )?;
                let image = !profiles.is_empty()
                    && profiles.iter().all(|profile| {
                        self.context.control.operand_retentions.is_valid(*profile)
                            && self
                                .context
                                .control
                                .operand_retentions
                                .get(*profile)
                                .immutable_image
                    });
                for profile in profiles {
                    for root in self.context.control.retained_roots(profile)? {
                        if !roots.contains(root) {
                            roots.push(*root);
                        }
                    }
                }
                // Preserve the exact captured formation namespace. An opaque
                // supplier can withhold geometry without losing its independent
                // evaluated-source association; roots never fill that gap.
                let selected =
                    super::super::operand_values::captured_operand_reference_places_with_formation(
                        self.program,
                        &self.context.control,
                        &value,
                        value.type_reference,
                        &projection,
                    );
                if selected.as_ref().is_some_and(Vec::is_empty) && !image {
                    return None;
                }
                has_unknown_geometry |= image || selected.is_none();
                let selected = selected.unwrap_or_default();
                for place in &selected {
                    if !places.contains(place) {
                        places.push(place.clone());
                    }
                }
                all_image &= image;
                candidates.push((selected, image, Some((value, projection))));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        // Candidates are complete mutually exclusive descriptor values, not a
        // partial address union. Image alternatives have no invented address;
        // every alternative retains the complete declared lifetime frontier.
        let mut alternatives = Vec::new();
        for (places, image, supplier) in candidates {
            let retention = self.append_retention(&roots);
            self.context
                .control
                .operand_retentions
                .get_mut(retention)
                .immutable_image = image;
            if let Some((value, projection)) = supplier {
                let operand = self.context.control.operands.insert(value);
                let projection = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(projection);
                let suppliers = self
                    .context
                    .control
                    .operand_retained_suppliers
                    .insert_many([checked_trees::FlowOperandRetainedSupplier {
                        operand,
                        projection,
                    }]);
                let profile = self.context.control.operand_retentions.get_mut(retention);
                profile.suppliers = suppliers;
                profile.descriptor_sources = suppliers;
            }
            let referents = self.append_captured_references(&places);
            alternatives.push(FlowOperandFact {
                type_reference: reference,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: self.storage_snapshot,
                referents,
                retention,
                ..Default::default()
            });
        }
        if alternatives.len() == 1 {
            return Some(self.context.control.operands.insert(alternatives.pop()?));
        }
        let mut profiles = Vec::new();
        for value in alternatives {
            let descriptor = self.context.control.operands.insert(value);
            let mut profile = *self.context.control.operand_retentions.get(value.retention);
            profile.descriptor = descriptor;
            profiles.push(profile);
        }
        let alternatives = self
            .context
            .control
            .operand_retentions
            .insert_many(profiles);
        let referents = if has_unknown_geometry {
            Default::default()
        } else {
            self.append_captured_references(&places)
        };
        let retention = self.append_retention(&roots);
        self.context
            .control
            .operand_retentions
            .get_mut(retention)
            .immutable_image = all_image;
        self.context
            .control
            .operand_retentions
            .get_mut(retention)
            .alternatives = alternatives;
        Some(self.context.control.operands.insert(FlowOperandFact {
            type_reference: reference,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            storage_snapshot: self.storage_snapshot,
            referents,
            retention,
            ..Default::default()
        }))
    }

    pub(super) fn returned_slot_path(
        &self,
        returned: TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<Vec<PlaceSegment>> {
        let owner = crate::borrow::owner_path_from_place_segments(self.program, path);
        let structural = crate::borrow::structural_owner_path(self.program, &owner)?;
        validation::reference_result_custody::loan_place_segments(
            self.program,
            returned,
            &structural,
        )
    }

    /// Complete body-derived image leaves, independent of the declaration's
    /// caller-lifetime inputs. A static alternative alone is insufficient.
    pub(super) fn returned_image_paths(
        &self,
        call: &typed_trees::expression::TableCallExpression,
        returned: TypeReferenceHandle,
    ) -> Option<Vec<Vec<PlaceSegment>>> {
        let target = validation::call_target_symbol(self.program, call);
        let state = validation::call_target_state(self.program, target)?;
        let sources = self
            .context
            .call_frames?
            .reference_result_leaf_origins(state.symbol)?;
        let mut paths = Vec::new();
        for source in &sources {
            if !source.source.is_image_storage
                || sources.iter().any(|other| {
                    other.result_path == source.result_path && !other.source.is_image_storage
                })
            {
                continue;
            }
            let path = self.returned_slot_path(returned, &source.result_path)?;
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        Some(paths)
    }
}
