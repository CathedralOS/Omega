//! A representation view keeps the storage it actually borrowed before the
//! new interpretation. The whole source is a safe enclosure for owned fields;
//! its logical field symbols must never be appended to the source layout.
//! This relation is independent of lifetime roots and exact value referents.
//! Resident views borrow content, so their Extent descriptor is excluded.
use super::Execution;
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowOperandRetention};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};

impl Execution<'_, '_, '_> {
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
        let Some(formation) = validation::reference_formation_source(self.program, expression)
        else {
            return;
        };
        if formation.interpretation != validation::ReferenceFormationInterpretation::Placed {
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
        places: &[validation::CanonicalPlace],
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

    pub(super) fn completed_backing_places(
        &mut self,
        value: &FlowOperandFact,
    ) -> Option<Vec<validation::CanonicalPlace>> {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE projectable-backing machine={:?} statement={} expression={:?} exact_count={} retention={:?} completion={:?}",
                self.machine.symbol,
                self.statement_index,
                value.expression,
                value.referents.count(),
                self.context
                    .control
                    .operand_retentions
                    .is_valid(value.retention)
                    .then(|| self.context.control.operand_retentions.get(value.retention)),
                value.completion
            );
        }
        let places = if !value.referents.is_empty() {
            validation::completed_storage::direct_referent_places(
                &self.context.control,
                value.referents,
            )?
        } else {
            let operand = self.context.control.operands.insert(*value);
            validation::completed_storage::projectable_reference_places(
                self.program,
                &self.context.control,
                operand,
                &[],
            )?
        };
        Some(
            places
                .into_iter()
                .map(|place| validation::CanonicalPlace {
                    root: place.root,
                    segments: place.segments,
                })
                .collect(),
        )
    }

    pub(super) fn capture_invocation_backing(
        &mut self,
        invocation: Handle<checked_trees::FlowCallFact>,
        value: &FlowOperandFact,
    ) -> Option<Vec<validation::CanonicalPlace>> {
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
        let checked_dispatch =
            source
                .static_requirement_dispatch
                .as_ref()
                .is_some_and(|dispatch| {
                    self.program
                        .machine_by_symbol(dispatch.realization_machine)
                        .is_some_and(|machine| {
                            machine.body_is_present
                                && machine.supply_mode
                                    == language_semantics::MachineSupplyMode::CheckedBody
                        })
                });
        if !source.carries_only_positional_arguments()
            || (!source.selects_only_nominal_route() && !checked_dispatch)
        {
            return None;
        }
        let output = validation::reference_shell(self.program, value.type_reference)?;
        let state = validation::call_target_state(self.program, call.target_symbol)?;
        let machine = self.program.machine_holding_state(state.symbol)?;
        if !machine.body_is_present
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
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
            let input = validation::reference_shell(self.program, actual.type_reference)?;
            let mut segments = source
                .segments
                .into_iter()
                .map(|segment| {
                    segment.erase_runtime_selector(|expression| {
                        self.program
                            .expression_table
                            .range_node(expression)
                            .is_some()
                    })
                })
                .collect();
            crate::flow::compose_captured_view_segments(&mut segments)?;
            // A field below an unobserved stored pointer belongs to another
            // namespace. Refuse that crossing instead of treating its home as
            // the referee. Lifetime correspondence supplies no missing origin.
            let selected = validation::owned_projection_type_reference(
                self.program,
                input.referee,
                &segments,
            )?;
            if !validation::storage_origin_types_match(self.program, selected, output.referee) {
                return None;
            }
            let addresses = self.completed_backing_places(actual)?;
            if addresses.is_empty() {
                return None;
            }
            for address in addresses {
                let place = crate::flow::project_captured_view_place(address, &segments)?;
                crate::collections::push_unique(&mut backing, place);
            }
        }
        (!backing.is_empty()).then_some(backing)
    }
}
