//! Completed storage geometry stays separate from lifetime and exact identity.
//! Checked bodies retain complete possible coordinates for interference and
//! typed field projection. A representation view instead keeps its original
//! backing enclosure: its new field symbols cannot name the source layout.
//! Resident views borrow content, excluding their Extent descriptor.
use super::Execution;
use crate::flow::expressions;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowCallFact, FlowOperandFact, FlowOperandReferent, FlowOperandRetention};
use facts::PlaceSegment;
use language_semantics::MachineSupplyMode;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use validation::CanonicalPlace;
use validation::ReferenceFormationInterpretation;
use validation::call_target_state;
use validation::completed_storage::direct_referent_places;
use validation::completed_storage::projectable_reference_places;
use validation::owned_projection_type_reference;
use validation::reference_formation_source;
use validation::reference_shell;
use validation::storage_origin_types_match;

impl Execution<'_, '_, '_> {
    /// Owned projections preserve every possible backing without claiming an
    /// exact address. A dynamic selector contributes an element dimension, not
    /// an expression that a later write may evaluate again.
    pub(super) fn project_possible_storage(
        &mut self,
        value: &FlowOperandFact,
        path: &[PlaceSegment],
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        let backing = self.completed_backing_places(value)?;
        if backing.is_empty() {
            return None;
        }
        let path = path
            .iter()
            .map(|segment| match segment {
                PlaceSegment::Index { .. } => PlaceSegment::Index {
                    expression: typed_trees::expression::ExpressionHandle::invalid(),
                },
                segment => *segment,
            })
            .collect::<Vec<_>>();
        let projected = backing
            .into_iter()
            .map(|place| crate::flow::project_captured_view_place(place, &path))
            .collect::<Option<Vec<_>>>()?;
        Some(self.append_referent_places(&projected))
    }

    pub(super) fn view_enclosure(
        &self,
        profile: Handle<FlowOperandRetention>,
    ) -> Handle<FlowOperandFact> {
        if self.context.control.operand_retentions.is_valid(profile) {
            self.context
                .control
                .operand_retentions
                .get(profile)
                .enclosing_storage
        } else {
            Handle::invalid()
        }
    }

    pub(super) fn reinterpreted_view_retention(
        &mut self,
        source: &FlowOperandFact,
        lifetime: Handle<FlowOperandRetention>,
    ) -> Handle<FlowOperandRetention> {
        if !self.context.control.operand_retentions.is_valid(lifetime) {
            return lifetime;
        }
        let profile = *self.context.control.operand_retentions.get(lifetime);
        let enclosing_storage = self.context.control.operands.insert(*source);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                enclosing_storage,
                possible_storage: arena::HandleSpan::empty(),
                ..profile
            })
    }

    pub(super) fn capture_formation_enclosure(
        &mut self,
        expression: ExpressionHandle,
        call: &TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        retention: &mut Option<Handle<FlowOperandRetention>>,
    ) {
        let Some(formation) = reference_formation_source(self.program, expression) else {
            return;
        };
        if formation.interpretation != ReferenceFormationInterpretation::Access {
            return;
        }
        // The formation owner authenticated this single ordinary-source factory.
        // The namespace receiver was evaluated separately; it has no source slot.
        let Some([(_, source)]) = inputs.get(usize::from(call.receiver.is_valid())..) else {
            return;
        };
        if let Some(lifetime) = *retention {
            *retention = Some(self.reinterpreted_view_retention(source, lifetime));
        }
    }
    /// A complete conservative footprint is separate from the exact pointer
    /// roster. Preserve it with the descriptor through stores and forwarding.
    pub(super) fn retain_possible_storage(
        &mut self,
        value: &mut FlowOperandFact,
        places: &[CanonicalPlace],
    ) {
        if places.is_empty()
            || !self
                .context
                .control
                .operand_retentions
                .is_valid(value.retention)
        {
            return;
        }
        let possible_storage = self.append_referent_places(places);
        let profile = *self.context.control.operand_retentions.get(value.retention);
        value.retention = self
            .context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                possible_storage,
                ..profile
            });
    }

    /// A joined view is projectable only if every retained alternative is.
    /// Missing reference metadata or an enclosure supplies no typed field path.
    pub(super) fn backing_allows_projection(&self, value: &FlowOperandFact) -> bool {
        if !value.retention.is_valid() {
            // Owned storage can have independently captured coordinates without
            // carrying any borrowed lifetime. A reference needs its profile.
            return !self.is_view_type(value.type_reference);
        }
        retained_backing_allows_projection(&self.context.control, value.retention)
    }

    pub(super) fn completed_backing_places(
        &mut self,
        value: &FlowOperandFact,
    ) -> Option<Vec<CanonicalPlace>> {
        // An enclosure names the source interpretation, not typed coordinates
        // of this descriptor. Even a same-shaped type cannot project it.
        if !self.backing_allows_projection(value) {
            return None;
        }
        let places = if !value.referents.is_empty() {
            direct_referent_places(&self.context.control, value.referents)?
        } else {
            let operand = self.context.control.operands.insert(*value);
            projectable_reference_places(self.program, &self.context.control, operand, &[])?
        };
        Some(
            places
                .into_iter()
                .map(|place| CanonicalPlace {
                    root: place.root,
                    segments: place.segments,
                })
                .collect(),
        )
    }

    pub(super) fn capture_invocation_backing(
        &mut self,
        invocation: Handle<FlowCallFact>,
        value: &FlowOperandFact,
    ) -> Option<Vec<CanonicalPlace>> {
        if !self.context.control.calls.is_valid(invocation) {
            return None;
        }
        let call = self.context.control.calls.get(invocation).clone();
        if call.authored_expression != value.expression
            || call.statement_index != self.statement_index
        {
            return None;
        }
        let source = self.program.expression_table.call_node(value.expression)?;
        if !source.carries_only_positional_arguments() || !source.selects_only_nominal_route() {
            return None;
        }
        let output = reference_shell(self.program, value.type_reference)?;
        let state = call_target_state(self.program, call.target_symbol)?;
        let machine = self.program.machine_holding_state(state.symbol)?;
        if !machine.body_is_present
            || machine.supply_mode != MachineSupplyMode::CheckedBody
            || machine.attached_data.is_some()
        {
            return None;
        }
        let parameters = self.program.state_parameters(state);
        let actuals = self
            .context
            .control
            .operands
            .span_or_empty(call.operands)
            .to_vec();
        if actuals.len() != parameters.len() {
            return None;
        }
        let origins = self
            .context
            .call_frames?
            .reference_result_leaf_origins(state.symbol)?;
        let mut backing = Vec::new();
        for origin in origins {
            let source = origin.source;
            if !origin.result_path.is_empty()
                || source.is_image_storage
                || source.invocation_output.is_some()
                || !source.recasts.is_empty()
                || source.representation.is_some()
            {
                return None;
            }
            let argument_index = parameters
                .iter()
                .position(|parameter| parameter.symbol == source.root)?;
            let actual = actuals.get(argument_index)?;
            let input = reference_shell(self.program, actual.type_reference)?;
            let mut segments = source
                .segments
                .into_iter()
                .map(|segment| {
                    segment.erase_runtime_selector(|expression| {
                        expressions(self.program).range_node(expression).is_some()
                    })
                })
                .collect();
            crate::flow::compose_captured_view_segments(&mut segments)?;
            // A field below an unobserved stored pointer belongs to another
            // namespace. Refuse that crossing instead of treating its home as
            // the referee. Lifetime correspondence supplies no missing origin.
            let selected = owned_projection_type_reference(self.program, input.referee, &segments)?;
            if !storage_origin_types_match(self.program, selected, output.referee) {
                return None;
            }
            let addresses = self.completed_backing_places(actual)?;
            if addresses.is_empty() {
                return None;
            }
            for address in addresses {
                let place = crate::flow::project_captured_view_place(address, &segments)?;
                crate::shared::collections::push_unique(&mut backing, place);
            }
        }
        (!backing.is_empty()).then_some(backing)
    }
}

fn retained_backing_allows_projection(
    control: &checked_trees::FlowControlFacts,
    retention: Handle<FlowOperandRetention>,
) -> bool {
    let profiles = &control.operand_retentions;
    if !profiles.is_valid(retention) {
        return false;
    }
    let root = profiles.get(retention);
    if root.enclosing_storage.is_valid() || !root.enclosing_regions.is_empty() {
        return false;
    }
    if root.alternatives.is_empty() {
        return true;
    }
    let mut remaining = profiles.len();
    let mut pending = vec![(retention, false)];
    let mut active = std::collections::HashSet::new();
    let mut completed = std::collections::HashSet::new();
    while let Some((handle, leaving)) = pending.pop() {
        if leaving {
            active.remove(&handle);
            completed.insert(handle);
            continue;
        }
        if completed.contains(&handle) {
            continue;
        }
        if !profiles.is_valid(handle) || !active.insert(handle) {
            return false;
        }
        let Some(next) = remaining.checked_sub(1) else {
            return false;
        };
        remaining = next;
        let profile = profiles.get(handle);
        if profile.enclosing_storage.is_valid() || !profile.enclosing_regions.is_empty() {
            return false;
        }
        pending.push((handle, true));
        if !profile.alternatives.is_empty() {
            if profiles.span(profile.alternatives).is_none() {
                return false;
            }
            pending.extend(profile.alternatives.handles().map(|handle| (handle, false)));
        }
    }
    true
}

#[cfg(test)]
mod backing_profiles_tests {
    use super::retained_backing_allows_projection;
    use arena::Handle;
    use checked_trees::{FlowControlFacts, FlowOperandRetention};

    #[test]
    fn backing_profiles_keep_enclosures_after_joins() {
        let mut control = FlowControlFacts::default();
        let leaf = control
            .operand_retentions
            .insert_many([FlowOperandRetention::default()]);
        let shared = control.operand_retentions.insert_many([
            FlowOperandRetention {
                alternatives: leaf,
                ..Default::default()
            },
            FlowOperandRetention {
                alternatives: leaf,
                ..Default::default()
            },
        ]);
        let joined = control.operand_retentions.insert(FlowOperandRetention {
            alternatives: shared,
            ..Default::default()
        });
        // Both alternatives share a tail; sharing alone is not a cycle.
        assert!(retained_backing_allows_projection(&control, joined));
        control
            .operand_retentions
            .get_mut(leaf.handles().next().unwrap())
            .enclosing_storage = Handle::from_arena_index(1);
        // The wrapper must not turn one alternative's enclosure into an address.
        assert!(!retained_backing_allows_projection(&control, joined));
    }

    #[test]
    fn backing_profiles_reject_cycles_and_missing_rows() {
        let mut control = FlowControlFacts::default();
        let cycle = control
            .operand_retentions
            .insert_many([FlowOperandRetention::default()]);
        let root = cycle.handles().next().unwrap();
        control.operand_retentions.get_mut(root).alternatives = cycle;
        assert!(!retained_backing_allows_projection(&control, root));
        assert!(!retained_backing_allows_projection(
            &control,
            Handle::from_arena_index(999)
        ));
    }
}
