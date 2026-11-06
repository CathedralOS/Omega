//! Checked supplier capture from immutable completed operands.
//! Every destination is captured before publishing any write. Unknown or
//! lifetime-only suppliers cannot manufacture an address or current-slot read.
use super::super::Execution;
use super::super::reference_projection::ReferenceProjection;
use super::{CallStore, CallStoredValue};
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowCallFact, FlowOperandFact};
use symbols::SymbolHandle;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    pub(in super::super) fn capture_stored_suppliers(
        &mut self,
        parameters: &[typed_trees::signature::StateParameter],
        effects: &[validation::StoredReferenceEffect],
        inputs: HandleSpan<FlowOperandFact>,
        call: &BorrowCallFact,
        binding_writes: Option<&[CanonicalPlace]>,
    ) -> Vec<CallStore> {
        if parameters.len() != inputs.len() {
            return Vec::new();
        }
        let input = |symbol: SymbolHandle| {
            typed_trees::signature::state_parameter_position(parameters, symbol)
                .map(|position| inputs.handle_at(position as u32))
        };
        let mut values: Vec<CallStore> = Vec::new();
        for effect in effects.iter() {
            // Logical debts join the borrow roster; they supply no pointer.
            let Some(access) = effect.reference_access() else {
                continue;
            };
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
            let Some(actual_handle) = input(effect.destination.local_symbol) else {
                continue;
            };
            let actual = *self.context.control.operands.get(actual_handle);
            // Stores select the data reached by the actual's outer reference,
            // including an empty path that replaces an inner pointer slot.
            // This is destination typing; a write-only address need not read
            // the previous descriptor to install its replacement.
            let carrier = validation::reference_shell(self.program, actual.type_reference)
                .map_or(actual.type_reference, |reference| reference.referee);
            let Some(reference) = validation::project_type_reference_from_segments(
                self.program,
                carrier,
                &effect.destination.local_segments,
            ) else {
                continue;
            };
            if validation::reference_shell(self.program, reference)
                .is_none_or(|shell| shell.access != access)
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
            let output_lifetime_sources = if supplies_output {
                let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == effect.destination.local_symbol)
                else {
                    continue;
                };
                self.capture_output_lifetime_sources(
                    parameters,
                    inputs,
                    call,
                    facts::InvocationOutputIdentity {
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        call_ordinal: call.call_ordinal,
                        parameter_symbol: parameter.symbol,
                    },
                    &effect.destination.local_segments,
                )
            } else {
                Handle::invalid()
            };
            // Capture suppliers from this same pre-call tuple. A lifetime-only
            // dependency cannot supply the written pointer. Performed outputs
            // keep their own incoming identity and full cohort above.
            let mut suppliers = Vec::new();
            let mut source_images = Vec::new();
            let mut suppliers_known = effect.suppliers_complete && !supplies_output;
            if suppliers_known {
                for source in &effect.sources {
                    if source.lifetime_access.is_some() {
                        source_images.push(None);
                        continue;
                    }
                    let Some(actual) = input(source.backing.root) else {
                        suppliers_known = false;
                        break;
                    };
                    let Some(selected) = self.call_backing_values(
                        actual,
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
                    let mut image_profiles = Vec::new();
                    let mut is_image = source.carrier_symbol == source.backing.root
                        && source.carrier_segments == source.backing.segments;
                    for (operand, projection) in selected {
                        let value = *self.context.control.operands.get(operand);
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
                        // A helper may establish a predicate on the same data
                        // before storing its reference. The checked body owns
                        // that predicate; this join retains the original
                        // pointer and its access, comparing only storage shape.
                        if !(supplied.access == expected.access
                            || supplied.access == language_semantics::ReferenceAccess::Mutable)
                            || !validation::storage_origin_types_match(
                                self.program,
                                validation::peel_declared_constraints(
                                    self.program,
                                    supplied.referee,
                                ),
                                validation::peel_declared_constraints(
                                    self.program,
                                    expected.referee,
                                ),
                            )
                        {
                            suppliers_known = false;
                            break;
                        }
                        match crate::flow::operand_values::captured_retention_profiles_at_path(
                            self.program,
                            &self.context.control,
                            &value,
                            &projection,
                        ) {
                            Some(profiles) if !profiles.is_empty() => {
                                is_image &= profiles.iter().all(|profile| {
                                    self.context.control.operand_retentions.is_valid(*profile)
                                        && self
                                            .context
                                            .control
                                            .operand_retentions
                                            .get(*profile)
                                            .immutable_image
                                });
                                image_profiles.extend(profiles);
                            }
                            _ => is_image = false,
                        }
                        suppliers.push((operand, projection));
                    }
                    if !suppliers_known {
                        break;
                    }
                    source_images.push(is_image.then_some(image_profiles));
                }
            }
            if !effect.suppliers_complete && !supplies_output {
                known = false;
            }
            for (source_index, source) in effect.sources.iter().enumerate() {
                if let Some(Some(profiles)) = source_images.get(source_index) {
                    // An authenticated immutable image has no caller-local
                    // address. Keep its exact supplier and debts, not a guessed
                    // pointee or the descriptor slot's previous generation.
                    for profile in profiles {
                        if let Some(retained) = self.context.control.retained_roots(*profile) {
                            for root in retained {
                                if !roots.contains(root) {
                                    roots.push(*root);
                                }
                            }
                        } else {
                            lifetime_known = false;
                        }
                    }
                    continue;
                }
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
                    let origin = crate::flow::CapturedReferencePlace {
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
                    let retained = crate::borrow::lifetime_contract::completed_output_retention(
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
                    );
                    let retained = match retained {
                        Ok(retained) => retained,
                        Err(reason) => {
                            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                                eprintln!(
                                    "STRUCT supplied-output-retention-refused identity={identity:?} destination={:?} reason={reason}",
                                    effect.destination.local_segments
                                );
                            }
                            known = false;
                            lifetime_known = false;
                            break;
                        }
                    };
                    for root in retained {
                        crate::shared::collections::push_unique(&mut roots, root);
                    }
                    crate::shared::collections::push_unique(&mut origins, origin);
                    continue;
                }
                let Some(backing) = input(source.backing.root) else {
                    known = false;
                    lifetime_known = false;
                    break;
                };
                let backing = *self.context.control.operands.get(backing);
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
                    let carrier = *self.context.control.operands.get(carrier);
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
                        crate::shared::collections::push_unique(&mut origins, origin);
                    }
                }
            }
            let has_image = effect.has_static_values || source_images.iter().any(Option::is_some);
            let has_local = effect.sources.iter().enumerate().any(|(position, source)| {
                source.lifetime_access.is_none()
                    && source_images.get(position).is_none_or(Option::is_none)
            });
            if has_image && has_local {
                // Local alternatives cannot locate an unknown image address. Keep
                // all suppliers and lifetime debts, withholding joined geometry.
                known = false;
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
            // Unknown coverage is explicit, never a partial source roster.
            let suppliers = if suppliers_known {
                let rows = suppliers
                    .into_iter()
                    .map(
                        |(value, projection)| checked_trees::FlowOperandRetainedSupplier {
                            contract: None,
                            operand: value,
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
                    && !crate::flow::stored_values::invalidates_destination(
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
                        // This profile describes only the newly captured supplier.
                        // An unchanged descriptor is a separate Incoming alternative;
                        // its backing must be resolved independently at the join.
                        profile.immutable_image = suppliers_known
                            && effect
                                .sources
                                .iter()
                                .all(|source| source.lifetime_access.is_none())
                            && source_images.len() == effect.sources.len()
                            && source_images.iter().all(Option::is_some)
                            && (effect.has_static_values || !effect.sources.is_empty());
                        profile.suppliers = suppliers;
                        profile.output_lifetime_sources = output_lifetime_sources;
                    }
                    // Only descriptor identity and lifetime cross. Earlier
                    // pointee facts and scalar predicates do not become output.
                    let descriptor_loan = if known {
                        self.supplied_descriptor_permission(call, &origins, reference)
                    } else {
                        Handle::invalid()
                    };
                    if supplies_output
                        && !descriptor_loan.is_valid()
                        && std::env::var_os("OMEGA_STRUCT_TRACE").is_some()
                    {
                        eprintln!(
                            "STRUCT supplied-descriptor-binding-refused known={known} type={reference:?} origins={origins:?} completed={:?}",
                            call.completed_reference_loans
                                .handles()
                                .filter_map(|handle| {
                                    let loan = self.borrow.loans.get(handle);
                                    (loan.lineage
                                        == checked_trees::BorrowLoanLineage::SuppliedOutput)
                                        .then(|| {
                                            let incoming = self
                                                .borrow
                                                .incoming_references
                                                .get(loan.incoming_reference);
                                            (
                                                handle,
                                                &incoming.origin,
                                                incoming.carrier_type,
                                                self.borrow
                                                    .owner_segments
                                                    .span_or_empty(incoming.owner_path),
                                            )
                                        })
                                })
                                .collect::<Vec<_>>()
                        );
                    }
                    let value = self.context.control.operands.insert(FlowOperandFact {
                        descriptor_permission: if descriptor_loan.is_valid() {
                            checked_trees::FlowDescriptorPermission::SuppliedOutput {
                                loan: descriptor_loan,
                            }
                        } else {
                            Default::default()
                        },
                        type_reference: reference,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        storage_snapshot: self.storage_snapshot,
                        referents,
                        retention,
                        ..Default::default()
                    });
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE stored-call-value-created state={:?} statement={} target={:?} value={value:?} retention={retention:?} reference={reference:?} suppliers_known={suppliers_known} suppliers={:?} sources={:?}",
                            self.state.symbol,
                            self.statement_index,
                            call.target_symbol,
                            self.context
                                .control
                                .operand_retained_suppliers
                                .span_or_empty(suppliers),
                            effect
                                .sources
                                .iter()
                                .map(|source| (
                                    source.backing.root,
                                    &source.backing.segments,
                                    &source.backing.recasts,
                                    &source.backing.representation,
                                    source.lifetime_access
                                ))
                                .collect::<Vec<_>>()
                        );
                    }
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
                        actual: actual_handle,
                        path: effect.destination.local_segments.clone(),
                        value,
                    });
                }
            }
        }
        values
    }

    /// The helper's produced pointer keeps the permission of that exact
    /// output. A later copy can use it after the output variable changes.
    /// The resource consumer still authenticates normal completion; a pending
    /// output coordinate or equal address cannot authorize the pointer.
    fn supplied_descriptor_permission(
        &self,
        call: &BorrowCallFact,
        origins: &[crate::flow::CapturedReferencePlace],
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Handle<checked_trees::BorrowLoanFact> {
        let [origin] = origins else {
            return Handle::invalid();
        };
        let facts::PlaceRoot::InvocationOutput(identity) = origin.place.root else {
            return Handle::invalid();
        };
        let Some(expected) = validation::reference_shell(self.program, reference) else {
            return Handle::invalid();
        };
        let mut permissions = call.completed_reference_loans.handles().filter(|loan| {
            let row = self.borrow.loans.get(*loan);
            if row.lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
                || !row.established_by_call.is_valid()
                || !self
                    .borrow
                    .incoming_references
                    .is_valid(row.incoming_reference)
            {
                return false;
            }
            let incoming = self.borrow.incoming_references.get(row.incoming_reference);
            let path = self
                .borrow
                .owner_segments
                .span_or_empty(incoming.owner_path);
            let place_path = crate::borrow::owner_place_segments(path);
            incoming.origin == (checked_trees::IncomingReferenceOrigin::Output { identity })
                && place_path.len() == origin.place.segments.len()
                && place_path
                    .iter()
                    .zip(&origin.place.segments)
                    .all(|(left, right)| {
                        validation::storage_path_relations::canonical_place_segments_equal(
                            self.program,
                            *left,
                            *right,
                        )
                    })
                && validation::borrow_contracts::installed_output_path_type(
                    self.program,
                    incoming.carrier_type,
                    &place_path,
                )
                .and_then(|reference| validation::reference_shell(self.program, reference))
                .is_some_and(|supplied| {
                    supplied.access == expected.access
                        && validation::storage_origin_types_match(
                            self.program,
                            supplied.referee,
                            expected.referee,
                        )
                })
        });
        let Some(permission) = permissions.next() else {
            return Handle::invalid();
        };
        if permissions.next().is_some() {
            Handle::invalid()
        } else {
            permission
        }
    }
}
