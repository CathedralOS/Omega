//! Capture normal-return descriptor associations from the actual evaluated
//! tuple before invocation writes, then publish them on the one immutable
//! storage timeline. The checked relation supplies correspondence; retained
//! captures supply geometry and formation, never source replay or new authority.
use super::reference_projection::ReferenceProjection;
use super::{Execution, InvocationSite};
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowCallFact, FlowOperandFact, FlowStorageSnapshot};
use symbols::SymbolHandle;
use validation::CanonicalPlace;

pub(super) struct CallStore {
    pub(super) destination: CanonicalPlace,
    pub(super) value: CallStoredValue,
}

pub(super) enum CallStoredValue {
    Unavailable,
    Incoming(Handle<FlowStorageSnapshot>),
    Captured(Handle<FlowOperandFact>),
    Alternatives(Vec<CallStoredValue>),
}

impl Execution<'_, '_, '_> {
    pub(super) fn capture_call_stores(
        &mut self,
        site: InvocationSite,
        call: &BorrowCallFact,
        inputs: &[FlowOperandFact],
        binding_writes: Option<&[CanonicalPlace]>,
    ) -> Vec<CallStore> {
        // State jumps have an arrival owner. This inventory describes normal
        // returns to the invocation's next operand, never transfer edges.
        if matches!(site, InvocationSite::Transition(_)) {
            return Vec::new();
        }
        let Some(contract) = self.context.call_frames.and_then(|frames| {
            frames.reference_slot_contract(
                self.machine,
                self.state,
                self.statement_index,
                call.call_ordinal,
                call.target_symbol,
                call.authored_expression,
            )
        }) else {
            return Vec::new();
        };
        let (parameters, effects) = match contract {
            validation::ReferenceSlotContract::CheckedBody { machine, state } => {
                let Some(effects) = self.context.stored_reference_effects(machine, state) else {
                    return Vec::new();
                };
                (self.program.state_parameters(state), effects)
            }
            validation::ReferenceSlotContract::Opaque {
                parameters,
                effects,
            } => (parameters, std::rc::Rc::new(effects)),
        };
        if inputs.len() != parameters.len() {
            return Vec::new();
        }
        let input = |symbol: SymbolHandle| {
            typed_trees::signature::state_parameter_position(parameters, symbol)
                .and_then(|position| inputs.get(position))
                .copied()
        };
        let mut values: Vec<CallStore> = Vec::new();
        for effect in effects.iter() {
            // Exclusive exposure invalidates a descriptor association even
            // when the complete body later proves its incoming value survives.
            // Reinstall that identity from this call-entry inventory below.
            // Static sources may have been projected out of the public roster;
            // only the private relation owner can certify incoming identity.
            if effect.sources.is_empty() && !effect.incoming_identity && !effect.has_static_values {
                continue;
            }
            if effect.incoming_identity && binding_writes.is_some_and(<[_]>::is_empty) {
                continue;
            }
            let Some(actual) = input(effect.destination.local_symbol) else {
                continue;
            };
            let Some(reference) = validation::project_type_reference_from_segments(
                self.program,
                actual.type_reference,
                &effect.destination.local_segments,
            ) else {
                continue;
            };
            if validation::reference_shell(self.program, reference)
                .is_none_or(|shell| shell.access != effect.access)
            {
                continue;
            }
            let Some(destinations) = self.capture_reference_projection(
                &actual,
                &effect.destination.local_segments,
                ReferenceProjection::Destination,
            ) else {
                continue;
            };
            let mut origins = Vec::new();
            let mut roots = Vec::new();
            let mut known = !effect.has_static_values;
            let mut lifetime_known = true;
            let supplies_output = effect
                .sources
                .iter()
                .any(|source| source.backing.invocation_output.is_some());
            for source in &effect.sources {
                if supplies_output && source.lifetime_access.is_some() {
                    // The output branch below retains the exact completed
                    // cohort. A debt source supplies no address to reload.
                    continue;
                }
                if source.backing.invocation_output.is_some() {
                    // The complete callee relation authorizes this performed
                    // output. Inputs below retain only its lifetime debts; none
                    // of their addresses can stand in for the written pointer.
                    let Some(parameter) = parameters
                        .iter()
                        .find(|parameter| parameter.symbol == effect.destination.local_symbol)
                    else {
                        known = false;
                        lifetime_known = false;
                        break;
                    };
                    let identity = facts::InvocationOutputIdentity {
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        call_ordinal: call.call_ordinal,
                        parameter_symbol: parameter.symbol,
                    };
                    let origin = super::super::CapturedReferencePlace {
                        place: CanonicalPlace {
                            root: facts::PlaceRoot::InvocationOutput(identity),
                            segments: effect.destination.local_segments.clone(),
                        },
                        formation: Handle::invalid(),
                    };
                    if source.lifetime_access.is_some()
                        || !identity.is_valid()
                        || source.backing.is_image_storage
                        || !source.backing.recasts.is_empty()
                        || source.backing.representation.is_some()
                        || !self.is_incoming_descriptor_place(&origin.place)
                    {
                        known = false;
                        lifetime_known = false;
                        break;
                    }
                    let Some(retained) = super::super::call_phases::completed_output_retention(
                        self.borrow,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        call,
                        identity,
                        parameter.type_reference,
                        &crate::borrow::owner_path_from_place_segments(
                            self.program,
                            &effect.destination.local_segments,
                        ),
                    ) else {
                        known = false;
                        lifetime_known = false;
                        break;
                    };
                    for root in retained {
                        if !roots.contains(&root) {
                            roots.push(root);
                        }
                    }
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                    continue;
                }
                let Some(backing) = input(source.backing.root) else {
                    known = false;
                    lifetime_known = false;
                    break;
                };
                // The may-write footprint may cover a collection even when
                // its independent typed source still names one exact leaf.
                // capture_reference_projection below validates those retained
                // coordinates and every descriptor crossing at this call cut;
                // the display/overlap path does not decide address identity.
                if source.backing.is_image_storage
                    || !source.backing.recasts.is_empty()
                    || source.backing.representation.is_some()
                {
                    known = false;
                    lifetime_known = false;
                    break;
                }
                let Some(observed) = self.capture_reference_projection(
                    &backing,
                    &source.backing.segments,
                    ReferenceProjection::Value,
                ) else {
                    known = false;
                    lifetime_known = false;
                    break;
                };
                // Independent possible-alias rosters cannot pair a descriptor
                // carrier with a backing. A different formal route must have
                // one exact matching captured observation, including formation.
                if source.carrier_symbol != source.backing.root
                    || source.carrier_segments != source.backing.segments
                {
                    let Some(carrier) = input(source.carrier_symbol) else {
                        known = false;
                        lifetime_known = false;
                        break;
                    };
                    let Some(carrier) = self.capture_reference_projection(
                        &carrier,
                        &source.carrier_segments,
                        ReferenceProjection::Value,
                    ) else {
                        known = false;
                        lifetime_known = false;
                        break;
                    };
                    if observed.places.len() != 1 || observed.places != carrier.places {
                        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                            eprintln!(
                                "CHECKSITE stored-reference-source-correspondence machine={:?} state={:?} statement={} call={:?} observed_count={} carrier_count={} observed={:?} carrier={:?}",
                                self.machine.symbol,
                                self.state.symbol,
                                self.statement_index,
                                call.authored_expression,
                                observed.places.len(),
                                carrier.places.len(),
                                observed
                                    .places
                                    .iter()
                                    .take(4)
                                    .map(|place| (
                                        place.place.root,
                                        place.formation,
                                        place.place.segments.len(),
                                        &place.place.segments[..place.place.segments.len().min(32)],
                                    ))
                                    .collect::<Vec<_>>(),
                                carrier
                                    .places
                                    .iter()
                                    .take(4)
                                    .map(|place| (
                                        place.place.root,
                                        place.formation,
                                        place.place.segments.len(),
                                        &place.place.segments[..place.place.segments.len().min(32)],
                                    ))
                                    .collect::<Vec<_>>(),
                            );
                        }
                        known = false;
                        lifetime_known = false;
                        break;
                    }
                    if let Some(retained) = self.context.control.retained_roots(carrier.retention) {
                        for root in retained {
                            if !roots.contains(root) {
                                roots.push(*root);
                            }
                        }
                    } else {
                        lifetime_known = false;
                    }
                }
                if let Some(retained) = self.context.control.retained_roots(observed.retention) {
                    for root in retained {
                        if !roots.contains(root) {
                            roots.push(*root);
                        }
                    }
                } else {
                    lifetime_known = false;
                }
                if source.lifetime_access.is_none() {
                    for origin in observed.places {
                        if !origins.contains(&origin) {
                            origins.push(origin);
                        }
                    }
                }
            }
            if (!known || origins.is_empty())
                && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
            {
                eprintln!(
                    "CHECKSITE stored-reference-capture-unavailable machine={:?} state={:?} statement={} call={:?} target={:?} destination={:?}:path_count={}:prefix={:?} known={} origins={} sources={} incoming_identity={} retains_incoming={}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    call.authored_expression,
                    call.target_symbol,
                    effect.destination.local_symbol,
                    effect.destination.local_segments.len(),
                    &effect.destination.local_segments
                        [..effect.destination.local_segments.len().min(32)],
                    known,
                    origins.len(),
                    effect.sources.len(),
                    effect.incoming_identity,
                    effect.retains_incoming,
                );
                for source in effect.sources.iter().take(8) {
                    eprintln!(
                        "CHECKSITE stored-reference-capture-source carrier={:?}:path_count={}:prefix={:?} backing={:?}:path_count={}:prefix={:?} lifetime_only={:?} image={} coarse={} recasts={} representation={}",
                        source.carrier_symbol,
                        source.carrier_segments.len(),
                        &source.carrier_segments[..source.carrier_segments.len().min(32)],
                        source.backing.root,
                        source.backing.segments.len(),
                        &source.backing.segments[..source.backing.segments.len().min(32)],
                        source.lifetime_access,
                        source.backing.is_image_storage,
                        source.backing.collection_coarse,
                        source.backing.recasts.len(),
                        source.backing.representation.is_some(),
                    );
                }
            }
            // Capture suppliers from this same pre-call tuple. A lifetime-only
            // dependency cannot supply the written pointer. Performed outputs
            // keep their own incoming identity and full cohort above.
            let mut suppliers = Vec::new();
            let mut suppliers_known = !supplies_output;
            if suppliers_known {
                for source in &effect.sources {
                    if source.lifetime_access.is_some() {
                        continue;
                    }
                    let Some(actual) = input(source.backing.root) else {
                        suppliers_known = false;
                        break;
                    };
                    let Some(selected) = self.call_backing_values(
                        &actual,
                        &source.backing,
                        reference,
                        self.storage_snapshot,
                    ) else {
                        suppliers_known = false;
                        break;
                    };
                    if selected.is_empty() {
                        suppliers_known = false;
                        break;
                    }
                    for (value, projection) in selected {
                        let Some(supplied) = validation::project_type_reference_from_segments(
                            self.program,
                            value.type_reference,
                            &projection,
                        )
                        .and_then(|reference| validation::reference_shell(self.program, reference)) else {
                            suppliers_known = false;
                            break;
                        };
                        let expected =
                            validation::reference_shell(self.program, reference).unwrap();
                        if !(supplied.access == expected.access
                            || supplied.access == language_semantics::ReferenceAccess::Mutable)
                            || !validation::storage_origin_types_match(
                                self.program,
                                supplied.referee,
                                expected.referee,
                            )
                        {
                            suppliers_known = false;
                            break;
                        }
                        suppliers.push((value, projection));
                    }
                    if !suppliers_known {
                        break;
                    }
                }
            }
            // Unknown coverage is explicit, never a partial source roster.
            let suppliers = if suppliers_known {
                let rows = suppliers
                    .into_iter()
                    .map(
                        |(value, projection)| checked_trees::FlowOperandRetainedSupplier {
                            operand: self.context.control.operands.insert(value),
                            projection: self
                                .context
                                .control
                                .operand_retained_paths
                                .insert_many(projection),
                        },
                    )
                    .collect::<Vec<_>>();
                self.context
                    .control
                    .operand_retained_suppliers
                    .insert_many(rows)
            } else {
                HandleSpan::empty()
            };
            let weak_destination = destinations.places.len() != 1;
            for destination in destinations.places {
                if effect.incoming_identity
                    && !super::super::stored_values::invalidates_destination(
                        self.program,
                        binding_writes,
                        &destination.place,
                    )
                {
                    continue;
                }
                let mut alternatives = Vec::new();
                if !effect.sources.is_empty() || !effect.incoming_identity {
                    let referents = if known && !origins.is_empty() {
                        self.append_captured_references(&origins)
                    } else {
                        HandleSpan::empty()
                    };
                    let retention = if lifetime_known {
                        self.append_retention(&roots)
                    } else {
                        Handle::invalid()
                    };
                    if self.context.control.operand_retentions.is_valid(retention) {
                        let profile = self.context.control.operand_retentions.get_mut(retention);
                        profile.immutable_image = effect.has_static_values
                            && effect.sources.is_empty()
                            && !effect.retains_incoming;
                        profile.suppliers = suppliers;
                    }
                    // Only descriptor identity and lifetime cross. Earlier
                    // pointee facts and scalar predicates do not become output.
                    let value = self.context.control.operands.insert(FlowOperandFact {
                        type_reference: reference,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        storage_snapshot: self.storage_snapshot,
                        referents,
                        retention,
                        ..Default::default()
                    });
                    alternatives.push(CallStoredValue::Captured(value));
                }
                if effect.retains_incoming || effect.may_select || weak_destination {
                    alternatives.push(CallStoredValue::Incoming(self.storage_snapshot));
                }
                let value = match alternatives.len() {
                    0 => CallStoredValue::Unavailable,
                    1 => alternatives.pop().unwrap(),
                    _ => CallStoredValue::Alternatives(alternatives),
                };
                if let Some(previous) = values
                    .iter_mut()
                    .find(|value| value.destination == destination.place)
                {
                    // Independently summarized aliased outputs provide no
                    // ordering between their descriptor replacements.
                    previous.value = CallStoredValue::Unavailable;
                } else {
                    values.push(CallStore {
                        destination: destination.place,
                        value,
                    });
                }
            }
        }
        values
    }

    pub(super) fn install_call_stores(&mut self, stores: Vec<CallStore>) {
        for store in stores {
            self.storage_snapshot = self.install_call_stored_value(
                self.storage_snapshot,
                &store.destination,
                store.value,
            );
        }
    }

    fn install_call_stored_value(
        &mut self,
        previous: Handle<FlowStorageSnapshot>,
        destination: &CanonicalPlace,
        value: CallStoredValue,
    ) -> Handle<FlowStorageSnapshot> {
        match value {
            CallStoredValue::Unavailable => super::super::stored_values::append_store(
                &mut self.context.control,
                previous,
                destination,
                Handle::invalid(),
                &[],
            ),
            CallStoredValue::Captured(value) => super::super::stored_values::append_store(
                &mut self.context.control,
                previous,
                destination,
                value,
                &[],
            ),
            CallStoredValue::Incoming(source) => {
                super::super::stored_values::append_preserved_binding(
                    &mut self.context.control,
                    previous,
                    destination,
                    source,
                )
            }
            CallStoredValue::Alternatives(values) => {
                let predecessors = values
                    .into_iter()
                    .map(|value| self.install_call_stored_value(previous, destination, value))
                    .collect::<Vec<_>>();
                super::super::stored_values::join(&mut self.context.control, &predecessors)
            }
        }
    }
}
