//! Preserve a returned descriptor's body-derived association to evaluated
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
        let mut matched = false;
        for source in sources {
            let owner_path =
                crate::borrow::owner_path_from_place_segments(self.program, &source.result_path);
            let structural = crate::borrow::structural_owner_path(self.program, &owner_path)?;
            let selected = validation::reference_result_custody::loan_place_segments(
                self.program,
                returned,
                &structural,
            )?;
            if selected != path {
                continue;
            }
            matched = true;
            // Callee runtime selectors describe regions, not captured caller
            // coordinates. Their lifetime frontier remains available above.
            if source.source.collection_coarse
                || source.source.is_image_storage
                || !source.source.recasts.is_empty()
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
                for profile in profiles {
                    for root in self.context.control.retained_roots(profile)? {
                        if !roots.contains(root) {
                            roots.push(*root);
                        }
                    }
                }
                let referents = self.captured_referent_places(value.referents);
                if referents.is_empty() {
                    return None;
                }
                for referent in referents {
                    let place = validation::completed_storage::project_captured_view_place(
                        referent,
                        &projection,
                    )?;
                    if !places.contains(&place) {
                        places.push(place);
                    }
                }
            }
        }
        if !matched || places.is_empty() {
            return None;
        }
        let referents = self.append_referent_places(&places);
        let retention = self.append_retention(&roots);
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
}
