//! Complete-call stores consume the actual evaluated argument captures.
//! The callee relation selects slots and sources, not source syntax to replay.
//! Capture every output from one pre-call storage cut before installing any
//! output, so swaps and overlapping output rosters cannot change later sources.

use super::Execution;
use arena::Handle;
use checked_trees::{BorrowCallFact, FlowOperandFact};
use facts::PlaceSegment;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    pub(super) fn capture_call_stores(
        &mut self,
        call: &BorrowCallFact,
        actuals: &[FlowOperandFact],
    ) -> Vec<(CanonicalPlace, Handle<FlowOperandFact>, Vec<PlaceSegment>)> {
        let Some((machine, state, actuals)) = self.selected_store_target(call, actuals) else {
            return Vec::new();
        };
        let Some(effects) = self
            .context
            .call_frames
            .and_then(|frames| frames.state_stored_reference_effects(machine, state))
        else {
            return Vec::new();
        };
        let parameters = self.program.state_parameters(state);
        if parameters.len() != actuals.len() {
            return Vec::new();
        }
        let mut stores = Vec::new();
        for effect in effects {
            // A weak destination does not establish any single physical slot.
            // Its mutation remains on the ordinary invalidation timeline.
            if effect.may_select {
                continue;
            }
            let Some(position) = typed_trees::signature::state_parameter_position(
                parameters,
                effect.destination.local_symbol,
            ) else {
                continue;
            };
            let destination_actual = &actuals[position];
            let Some(projected) = self.project_call_place(
                destination_actual,
                &effect.destination.local_segments,
                self.storage_snapshot,
            ) else {
                continue;
            };
            let [destination] = projected.addresses.as_slice() else {
                continue;
            };
            let reference = projected.type_reference;
            if validation::reference_shell(self.program, reference).is_none() {
                continue;
            }
            let mut sources = Vec::new();
            let mut dependencies = Vec::new();
            let mut complete = true;
            if effect.retains_incoming {
                match super::super::stored_values::at_snapshot(
                    self.program,
                    &self.context.control,
                    self.storage_snapshot,
                    destination,
                    super::super::stored_values::StoredObservation::Binding,
                )
                .and_then(|value| value.captures())
                {
                    Some(incoming) => {
                        for value in incoming {
                            sources.push((
                                *self.context.control.operands.get(value.capture),
                                value.projection,
                            ));
                        }
                    }
                    None => complete = false,
                }
            }
            for source in effect.sources {
                let Some(position) = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.carrier_symbol,
                ) else {
                    complete = false;
                    break;
                };
                let Some(mut selected) = (if source.lifetime_access.is_some() {
                    self.call_slot_values(
                        &actuals[position],
                        &source.carrier_segments,
                        self.storage_snapshot,
                    )
                } else {
                    let backing_position = typed_trees::signature::state_parameter_position(
                        parameters,
                        source.backing.root,
                    );
                    backing_position.and_then(|position| {
                        self.call_backing_values(
                            &actuals[position],
                            &source.backing,
                            reference,
                            self.storage_snapshot,
                        )
                    })
                }) else {
                    complete = false;
                    break;
                };
                if source.lifetime_access.is_some() {
                    dependencies.append(&mut selected);
                } else {
                    sources.append(&mut selected);
                }
            }
            if !complete || (sources.is_empty() && !effect.has_static_values) {
                continue;
            }
            if sources.iter().any(|(source, projection)| {
                let Some(actual_type) = validation::project_type_reference_from_segments(
                    self.program,
                    source.type_reference,
                    projection,
                ) else {
                    return true;
                };
                let (Some(actual), Some(expected)) = (
                    validation::reference_shell(self.program, actual_type),
                    validation::reference_shell(self.program, reference),
                ) else {
                    return true;
                };
                !(actual.access == expected.access
                    || actual.access == language_semantics::ReferenceAccess::Mutable)
                    || !validation::storage_origin_types_match(
                        self.program,
                        actual.referee,
                        expected.referee,
                    )
            }) {
                continue;
            }

            // Alternative references have one runtime descriptor but retain
            // every possible backing. This receipt grants no scalar equality,
            // selected source, or new loan. Static alternatives have no local
            // backing; their exact image address remains unknown here.
            let mut places = Vec::new();
            let mut profiles = Vec::new();
            let mut source_images = Vec::with_capacity(sources.len());
            for (index, (source, projection)) in sources.iter().chain(&dependencies).enumerate() {
                let profiles_at_path =
                    super::super::operand_values::captured_retention_profiles_at_path(
                        self.program,
                        &self.context.control,
                        source,
                        projection,
                    );
                let Some(selected) = profiles_at_path else {
                    complete = false;
                    break;
                };
                if index < sources.len() {
                    source_images.push(
                        !selected.is_empty()
                            && selected.iter().all(|profile| {
                                self.context.control.operand_retentions.is_valid(*profile)
                                    && self
                                        .context
                                        .control
                                        .operand_retentions
                                        .get(*profile)
                                        .immutable_image
                            }),
                    );
                }
                profiles.extend(selected);
            }
            if !complete {
                continue;
            }
            if !effect.has_static_values {
                for ((source, projection), image) in sources.iter().zip(&source_images) {
                    // A retained literal image has no local referent address.
                    // Its authenticated image profile survives an incoming
                    // slot just as it survives an ordinary copied value.
                    if *image {
                        continue;
                    }
                    let selected = self
                        .context
                        .control
                        .operand_referents
                        .span_or_empty(source.referents)
                        .iter()
                        .map(|referent| {
                            super::super::operand_values::project_captured_view_place(
                                CanonicalPlace {
                                    root: referent.root,
                                    segments: self
                                        .context
                                        .control
                                        .operand_referent_segments
                                        .span_or_empty(referent.segments)
                                        .to_vec(),
                                },
                                projection,
                            )
                        })
                        .collect::<Option<Vec<_>>>();
                    let Some(selected) = selected else {
                        complete = false;
                        break;
                    };
                    if selected.is_empty() {
                        complete = false;
                        break;
                    }
                    for place in selected {
                        if !places.contains(&place) {
                            places.push(place);
                        }
                    }
                }
            }
            if !complete {
                continue;
            }
            // A local alternative plus an image is not exactly the local
            // referent set. Retain every supplier and lifetime dependency,
            // while withholding joined descriptor geometry.
            if source_images.iter().any(|image| *image) && source_images.iter().any(|image| !*image)
            {
                places.clear();
            }
            let mut roots = Vec::new();
            for profile in profiles {
                let Some(retained) = self.context.control.retained_roots(profile) else {
                    complete = false;
                    break;
                };
                for root in retained {
                    if !roots.contains(root) {
                        roots.push(*root);
                    }
                }
            }
            if !complete {
                continue;
            }
            // Only reference identity/backing crosses this completion. A
            // pre-call pointee profile is not a post-call contents snapshot or
            // a new scalar equality supplied by a symbolic store summary.
            let retention = self.append_retention(&roots);
            // Image scope comes from the body literal or every actual
            // selected input's own immutable-image receipt. Empty lifetime
            // roots alone and opaque dependency debts establish no geometry.
            self.context
                .control
                .operand_retentions
                .get_mut(retention)
                .immutable_image = dependencies.is_empty()
                && ((effect.has_static_values && sources.is_empty() && !effect.retains_incoming)
                    || (!source_images.is_empty() && source_images.iter().all(|image| *image)));
            // Retain the actual supplying evaluations independently of the
            // sanitized post-call address. Resource consumers must not recover
            // a permission from the destination slot's older loan generation.
            let suppliers = sources
                .iter()
                .map(|(source, projection)| {
                    let operand = self.context.control.operands.insert(*source);
                    let projection = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(projection.iter().copied());
                    checked_trees::FlowOperandRetainedSupplier {
                        operand,
                        projection,
                    }
                })
                .collect::<Vec<_>>();
            let suppliers = self
                .context
                .control
                .operand_retained_suppliers
                .insert_many(suppliers);
            self.context
                .control
                .operand_retentions
                .get_mut(retention)
                .suppliers = suppliers;
            let referents = self.append_referent_places(&places);
            let value = self.context.control.operands.insert(FlowOperandFact {
                type_reference: reference,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: self.storage_snapshot,
                referents,
                retention,
                ..Default::default()
            });
            stores.push((destination.clone(), value, Vec::new()));
        }
        stores
    }
}
