//! A call can install a reference in caller-owned storage independently of its
//! result value. Its body-derived slot relation supplies the same exact
//! owner/backing pairs as an assignment; type shape alone supplies no loan.
//! The persistent roster follows lexical fallthrough. Departing transition
//! operands retain their own normal-call custody and outgoing-edge records.

use super::{StatementBorrowLoan, owner_path_from_place_segments};

/// Observe the call-completed slots at one operand's completion. This scratch roster
/// retains incoming descriptor identities; it publishes no local loan node.
pub(super) fn roster_at_expression(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    frames: &validation::CallFrameResolver<'_>,
    incoming: &[super::StateLoanTracker],
    parameter_referents: &super::ParameterReferentGrants,
) -> Result<Vec<super::StateLoanTracker>, validation::StoredReferenceRelationRefusal> {
    let effects = match frames.stored_reference_effects_at_expression(
        machine,
        state,
        statement_index,
        expression,
    ) {
        Ok(effects) => effects,
        Err(validation::StoredReferenceRelationRefusal::TemplateDependent)
            if independent_owned_formation(
                program,
                machine,
                state,
                statement_index,
                expression,
            ) =>
        {
            // An owned address needs no loaded descriptor or previous parent.
            // Ordinary formation still records and checks its loan; no incoming
            // descriptor source is made current by this empty source roster.
            return Ok(Vec::new());
        }
        Err(reason) => return Err(reason),
    };
    let produced = from_effects(
        program,
        state,
        statement_index,
        effects.clone(),
        incoming,
        parameter_referents,
    );
    let mut current = incoming
        .iter()
        .filter(|loan| {
            !effects.iter().any(|effect| {
                !effect.may_select
                    && loan.storage_owner
                        == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                    && super::place_path_covers_owner(
                        program,
                        &effect.destination.local_segments,
                        &loan.owner_path,
                    )
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    current.extend(produced.into_iter().map(|loan| super::StateLoanTracker {
        representation_address: loan.representation_address,
        handle: Default::default(),
        owner_symbol: loan.owner_symbol,
        storage_owner: loan.storage_owner,
        owner_name: loan.owner_name,
        kind: loan.kind,
        lineage: loan.lineage,
        formation: loan.formation,
        permission_sources: Vec::new(),
        owner_path: loan.owner_path,
        place: loan.place,
        retained_operand: loan.retained_operand,
    }));
    Ok(current)
}

/// A declared owned address can be borrowed even while unrelated descriptor
/// transfers remain pending. Use the ordinary geometry and type projection;
/// a loaded pointer, incoming referent or unavailable type still needs its
/// current source roster. Case, index and access admission remain with their
/// ordinary formation owners; this establishes neither contents nor permission.
fn independent_owned_formation(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(reference) =
        validation::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    if validation::reference_shell(program, reference).is_none()
        && validation::slice_view_shell(program, reference, &[]).is_none()
    {
        return false;
    }
    let Some(geometry) = super::call_views::argument_borrow_loan_geometry(
        program,
        state.symbol,
        statement_index,
        machine.symbol,
        expression,
    ) else {
        return false;
    };
    let owned = |place: &super::accesses::BorrowAccessPlace| {
        if place.incoming_reference.is_valid() || !place.incoming_owner_path.is_empty() {
            return false;
        }
        if super::accesses::is_image_storage_root(program, place.root) {
            return true;
        }
        if !matches!(
            place.root,
            facts::PlaceRoot::Symbol(_) | facts::PlaceRoot::Expression(_)
        ) {
            return false;
        }
        // Owner paths describe the storage containing a range as element
        // storage. The original geometry retains its full interval/footprint;
        // this projection establishes only that no pointer was crossed.
        let owner_path = super::owner_path_from_place_segments(program, &place.segments);
        validation::canonical_owned_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: super::owner_place_segments(&owner_path),
            },
        )
        .is_some()
    };
    owned(&geometry.place) && geometry.representation_address.as_ref().is_none_or(owned)
}

pub(super) fn loans(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    statement: &checked_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming: &[super::StateLoanTracker],
    completed: Option<&[super::StateLoanTracker]>,
) -> Result<Vec<StatementBorrowLoan>, &'static str> {
    let Some(machine) = program.machine(machine_symbol) else {
        return Ok(Vec::new());
    };
    let Some(frames) = call_frames else {
        return Ok(Vec::new());
    };
    let effects = match frames.stored_reference_fallthrough_effects(machine, statement) {
        Ok(effects) => effects,
        Err(validation::StoredReferenceRelationRefusal::TemplateDependent) => {
            // No successor generation is established by a pending relation.
            // Keep incoming debts active; the selected closed body rechecks
            // this statement before it can publish concrete descriptor loans.
            return Ok(Vec::new());
        }
        Err(validation::StoredReferenceRelationRefusal::Unavailable(reason)) => {
            return Err(reason);
        }
    };
    if effects.is_empty() {
        return Ok(Vec::new());
    }
    let completed = completed.ok_or("stored call has no complete statement-successor roster")?;
    let mut output = Vec::new();
    for source in completed {
        let selected = effects.iter().any(|effect| {
            source.storage_owner == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                && super::place_path_covers_owner(
                    program, &effect.destination.local_segments, &source.owner_path,
                )
                // A possible destination keeps its original physical slots.
                // Only its actually produced alternatives need new rows.
                && (!effect.may_select
                    || !incoming.iter().any(|entry| entry.handle == source.handle))
        });
        if !selected {
            continue;
        }
        // This statement copy retains the exact final descriptor generation,
        // including skipped-call alternatives. Merged effects select storage
        // slots only; they cannot reconstruct a pointer's supplying occurrence.
        output.push(StatementBorrowLoan {
            formation_owner_path: None,
            incoming_origin: None,
            formation_expression: Default::default(),
            representation_address: source.representation_address.clone(),
            owner_symbol: source.owner_symbol,
            storage_owner: source.storage_owner,
            owner_name: source.owner_name.clone(),
            owner_path: source.owner_path.clone(),
            retained_operand: source.retained_operand,
            place: source.place.clone(),
            source_owner_symbol: source.owner_symbol,
            supplying_loans: std::iter::once(source.handle)
                .filter(|handle| handle.is_valid())
                .collect(),
            lineage: source.lineage.derived(),
            kind: source.kind.clone(),
            formation: super::BorrowFormation::ReturnedDescriptor,
        });
    }
    Ok(output)
}

pub(in crate::borrow) fn from_effects(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    effects: Vec<validation::StoredReferenceEffect>,
    loan_trackers: &[super::StateLoanTracker],
    parameter_referents: &super::ParameterReferentGrants,
) -> Vec<StatementBorrowLoan> {
    let mut loans = Vec::new();
    for effect in effects {
        let owner = effect.destination.local_symbol;
        let owner_path =
            owner_path_from_place_segments(program, &effect.destination.local_segments);
        // An exact slot has one post-call occurrence. A route which does
        // not write contributes its incoming alternatives to that same
        // occurrence, not another simultaneously live descriptor. Capture
        // these transfers before the caller retires the old slot. Weak
        // stores keep their physical untouched slots instead.
        if effect.retains_incoming && !effect.may_select {
            for source in loan_trackers.iter().filter(|source| {
                source.storage_owner == facts::PlaceRoot::Symbol(owner)
                    && super::place_path_covers_owner(
                        program,
                        &effect.destination.local_segments,
                        &source.owner_path,
                    )
            }) {
                if source.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                    && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
                {
                    eprintln!(
                        "CHECKSITE stored-input-debt-constructor phase=retained owner={owner:?} source={:?} owner_path={:?} place={:?}",
                        source.handle, source.owner_path, source.place
                    );
                }
                loans.push(StatementBorrowLoan {
                    formation_owner_path: None,
                    incoming_origin: None,
                    formation_expression: Default::default(),
                    representation_address: None,
                    owner_symbol: owner,
                    storage_owner: facts::PlaceRoot::Symbol(owner),
                    owner_name: program.symbols.name(owner).into(),
                    owner_path: source.owner_path.clone(),
                    retained_operand: source.retained_operand,
                    place: source.place.clone(),
                    source_owner_symbol: source.owner_symbol,
                    // Preserve ingress identity in the captured place without
                    // manufacturing a local predecessor from its zero handle.
                    supplying_loans: std::iter::once(source.handle)
                        .filter(|handle| handle.is_valid())
                        .collect(),
                    lineage: source.lineage.derived(),
                    kind: source.kind.clone(),
                    formation: super::BorrowFormation::ReturnedDescriptor,
                });
            }
        }
        for source in &effect.sources {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE stored-input-source-constructor phase=entry owner={owner:?} owner_path={owner_path:?} carrier={:?} carrier_path={:?} backing={:?} carrier_backing={:?} lifetime_access={:?} reference_access={:?}",
                    source.carrier_symbol,
                    source.carrier_segments,
                    source.backing,
                    source.carrier_backing,
                    source.lifetime_access,
                    effect.reference_access()
                );
            }
            let Some(source_access) = source.lifetime_access.or(effect.reference_access()) else {
                continue;
            };
            let backing = &source.backing;
            if let Some(identity) = backing.invocation_output {
                // Both a supplied pointer and an already captured output
                // retained as a lifetime debt have this exact opaque namespace.
                // A debt does not become the current descriptor slot's address
                // or acquire a supplied permission. All other input debts and
                // alternatives remain independent rows of the complete relation.
                let access = source_access;
                let supplied = (identity.is_valid()
                    && !backing.is_image_storage
                    && backing.recasts.is_empty()
                    && backing.representation.is_none())
                .then(|| validation::find_state(program, identity.state_symbol))
                .flatten()
                .and_then(|(machine, state)| {
                    let parameter = validation::invocation_output_parameter(program, identity)?;
                    let reference = validation::borrow_contracts::installed_output_path_type(
                        program,
                        parameter.type_reference,
                        &backing.segments,
                    )?;
                    let shell = validation::reference_shell(program, reference)?;
                    (shell.access == access).then_some((machine, state, parameter.type_reference))
                });
                if let Some((machine, state, carrier_type)) = supplied {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE stored-input-source-constructor phase=opaque owner={owner:?} owner_path={owner_path:?} identity={identity:?} lifetime_access={:?}",
                            source.lifetime_access
                        );
                    }
                    loans.push(StatementBorrowLoan {
                        formation_owner_path: None,
                        incoming_origin: Some(super::PendingIncomingReference {
                            identity: checked_trees::IncomingReferenceIdentity {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                origin: checked_trees::IncomingReferenceOrigin::Output { identity },
                                carrier_type,
                                owner_path: Default::default(),
                            },
                            owner_path: owner_path_from_place_segments(program, &backing.segments),
                        }),
                        formation_expression: match validation::find_call_site_in_state(
                            program,
                            machine,
                            state,
                            identity.statement_index,
                            identity.call_ordinal,
                        ) {
                            Some(validation::CallSite::Expression { expression, .. }) => expression,
                            _ => Default::default(),
                        },
                        representation_address: None,
                        owner_symbol: owner,
                        storage_owner: facts::PlaceRoot::Symbol(owner),
                        owner_name: program.symbols.name(owner).into(),
                        owner_path: owner_path.clone(),
                        retained_operand: None,
                        place: checked_trees::CapturedPlace {
                            incoming_reference: Default::default(),
                            incoming_owner_path: Default::default(),
                            root: facts::PlaceRoot::InvocationOutput(identity),
                            segments: Vec::new(),
                        },
                        source_owner_symbol: Default::default(),
                        supplying_loans: Vec::new(),
                        lineage: if source.lifetime_access.is_some() {
                            checked_trees::BorrowLoanLineage::LifetimeDependency
                        } else {
                            checked_trees::BorrowLoanLineage::SuppliedOutput
                        },
                        kind: checked_trees::BorrowAccessKind::from(access),
                        formation: super::BorrowFormation::ReturnedDescriptor,
                    });
                    continue;
                }
                // The explicit output alternative is malformed/unavailable.
                // Keep its zero-root unknown alternative below; deleting it
                // would let a sibling source falsely prove a complete result.
            }
            let mut captured = match source_capture_place(program, state, statement_index, source) {
                Ok(place) => super::call_views::rebase_borrow_places_through_local_loans(
                    program,
                    place,
                    loan_trackers,
                ),
                Err(reason) => {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE stored-input-source-unavailable state={:?} statement={statement_index} carrier={:?} reason={reason}",
                            state.symbol, source.carrier_symbol
                        );
                    }
                    Vec::new()
                }
            };
            // Bare formal forwarding consumes the already published root grant,
            // not a local predecessor or the next stored descriptor.
            for candidate in &mut captured {
                parameter_referents.bind(state.symbol, candidate);
            }
            // A completed source records its input referent before deriving
            // the output view. Account for every actual carrier alternative
            // across the complete source roster, never a convenient subset.
            // Lifetime debts are separate obligations, not spatial choices.
            let relevant = |candidate: &&super::RebasedBorrowPlace| {
                source.lifetime_access.is_some() || candidate.lifetime_access.is_none()
            };
            let complete_carrier = captured.iter().find(relevant).is_some()
                && captured.iter().filter(relevant).all(|candidate| {
                    effect.sources.iter().any(|alternative| {
                        alternative.carrier_symbol == source.carrier_symbol
                            && alternative.carrier_segments == source.carrier_segments
                            && alternative.lifetime_access.is_some()
                                == source.lifetime_access.is_some()
                            && transferred_place(
                                &alternative.backing,
                                alternative.carrier_backing.as_ref(),
                                alternative.lifetime_access.is_some(),
                                candidate,
                            )
                            .is_some()
                    })
                });
            let matching: Vec<_> = captured
                .iter()
                .filter_map(|candidate| {
                    complete_carrier
                        .then(|| {
                            transferred_place(
                                backing,
                                source.carrier_backing.as_ref(),
                                source.lifetime_access.is_some(),
                                candidate,
                            )
                        })
                        .flatten()
                        .map(|place| Some((candidate, place)))
                })
                .collect();
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE stored-input-source-constructor phase=resolved destination={owner:?} owner_path={owner_path:?} carrier={:?} carrier_path={:?} backing={backing:?} carrier_backing={:?} lifetime_access={:?} complete_carrier={complete_carrier} matched={} captured={:?} entry_descriptors={:?}",
                    source.carrier_symbol,
                    source.carrier_segments,
                    source.carrier_backing,
                    source.lifetime_access,
                    matching.len(),
                    captured
                        .iter()
                        .map(|candidate| (
                            &candidate.place,
                            &candidate.source_place,
                            candidate.source_loan,
                            candidate.parent_loan,
                            &candidate.lifetime_access,
                        ))
                        .collect::<Vec<_>>(),
                    loan_trackers
                        .iter()
                        .filter(|candidate| {
                            candidate.storage_owner
                                == facts::PlaceRoot::Symbol(source.carrier_symbol)
                                || candidate.storage_owner == facts::PlaceRoot::Symbol(backing.root)
                        })
                        .map(|candidate| (
                            candidate.handle,
                            &candidate.owner_path,
                            &candidate.place,
                            &candidate.kind,
                            &candidate.lineage,
                        ))
                        .collect::<Vec<_>>(),
                );
            }
            // Equal geometry can belong to several alternatives of the
            // actual descriptor. Preserve every supplier, never choose one
            // by position or erase their identities by deduplicating places.
            let matching = if matching.is_empty() {
                vec![None]
            } else {
                matching
            };
            for supplying in matching {
                let supplier = supplying.as_ref().map(|(source, _)| *source);
                // Provenance alone cannot select a convenient same-backing
                // loan. Only the exact actual carrier supplies transfer
                // authority; otherwise retain the backing conflict without
                // inventing a parent.
                loans.push(StatementBorrowLoan {
                    formation_owner_path: None,
                    incoming_origin: None,
                    formation_expression: Default::default(),
                    representation_address: None,
                    owner_symbol: owner,
                    storage_owner: facts::PlaceRoot::Symbol(owner),
                    owner_name: program.symbols.name(owner).into(),
                    owner_path: owner_path.clone(),
                    retained_operand: supplier.and_then(|source| source.retained_operand),
                    place: supplying.as_ref().map_or_else(
                        || checked_trees::CapturedPlace {
                            incoming_reference: Default::default(),
                            incoming_owner_path: Default::default(),
                            root: facts::PlaceRoot::Symbol(backing.root),
                            segments: backing.segments.clone(),
                        },
                        |(_, place)| place.clone(),
                    ),
                    source_owner_symbol: supplier
                        .map_or(Default::default(), |source| source.source_owner_symbol),
                    supplying_loans: supplier
                        .map(|source| source.parent_loan)
                        .filter(|loan| loan.is_valid())
                        .into_iter()
                        .collect(),
                    lineage: if source.lifetime_access.is_some()
                        || supplier.is_some_and(|source| source.lifetime_access.is_some())
                    {
                        checked_trees::BorrowLoanLineage::LifetimeDependency
                    } else {
                        checked_trees::BorrowLoanLineage::UnretainedDerived
                    },
                    kind: supplier
                        .and_then(|source| source.lifetime_access.clone())
                        .unwrap_or_else(|| checked_trees::BorrowAccessKind::from(source_access)),
                    formation: super::BorrowFormation::ReturnedDescriptor,
                });
            }
        }
    }
    loans
}

/// A record can keep a pointer that a call later replaces. Its lifetime debt
/// follows that saved pointer, so select the field before reading the original
/// loan. Projecting a whole-record observation afterward would name the slot
/// itself and lose the data and loan it held at this call's entry.
fn source_capture_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::StoredReferenceSource,
) -> Result<checked_trees::CapturedPlace, &'static str> {
    let mut place = checked_trees::CapturedPlace {
        incoming_reference: Default::default(),
        incoming_owner_path: Default::default(),
        root: facts::PlaceRoot::Symbol(source.carrier_symbol),
        segments: source.carrier_segments.clone(),
    };
    let Some(base) = source
        .carrier_backing
        .as_ref()
        .filter(|_| source.lifetime_access.is_some())
    else {
        return Ok(place);
    };
    let projection = source_projection(&source.backing, base)
        .ok_or("the lifetime source has no exact captured projection")?;
    if projection.is_empty() {
        return Ok(place);
    }
    place.segments.extend_from_slice(projection);
    let reference = validation::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        },
    )
    .ok_or("the lifetime source has no declared carrier type")?;
    let selected = validation::corresponding_place_projection(program, reference, &place.segments)
        .ok_or("the lifetime source projection has no exact applied member correspondence")?;
    let leaf = validation::project_type_reference_from_segments(program, reference, &selected)
        .ok_or("the lifetime source projection has no complete descriptor type")?;
    validation::reference_shell(program, leaf)
        .ok_or("the lifetime source projection does not select a reference descriptor")?;
    // Keep the producer's selector identities for the captured source-place
    // comparison. The typed correspondence above validates the full path;
    // the loan owner query joins that same applied declaration's ledger path.
    Ok(place)
}

fn source_projection<'source>(
    backing: &'source validation::LocalWriteSource,
    base: &validation::LocalWriteSource,
) -> Option<&'source [facts::PlaceSegment]> {
    if !base.root.is_valid()
        || base.invocation_output.is_some()
        || backing.invocation_output.is_some()
        || base.is_image_storage
        || backing.is_image_storage
        || !base.recasts.is_empty()
        || !backing.recasts.is_empty()
        || base.representation.is_some()
        || backing.representation.is_some()
        || backing.root != base.root
        || !backing.segments.starts_with(&base.segments)
    {
        return None;
    }
    Some(&backing.segments[base.segments.len()..])
}

/// Rejoin the exact observed descriptor to its produced projection. The
/// completed checking query has already crossed every real pointer load;
/// `carrier_backing` names that final load's unprojected referent. Geometry
/// alone, a common root, or another loan cannot establish this association.
fn transferred_place(
    backing: &validation::LocalWriteSource,
    carrier_backing: Option<&validation::LocalWriteSource>,
    lifetime_only: bool,
    candidate: &super::RebasedBorrowPlace,
) -> Option<checked_trees::CapturedPlace> {
    if candidate.lifetime_access.is_some() && !lifetime_only {
        return None;
    }
    // A saved pointer retains its original borrow after its variable changes.
    // A lifetime obligation can follow that exact captured descriptor, while
    // a physical projection still requires its own checked backing relation.
    let captured_descriptor = candidate.place.incoming_reference.is_valid()
        || (lifetime_only && candidate.source_loan.is_valid() && candidate.parent_loan.is_valid());
    if (candidate.place.root == facts::PlaceRoot::Symbol(backing.root)
        && candidate.place.segments == backing.segments)
        || (captured_descriptor
            && candidate.source_place.root == facts::PlaceRoot::Symbol(backing.root)
            && candidate.source_place.segments == backing.segments)
    {
        return Some(candidate.place.clone());
    }
    let base = carrier_backing?;
    let projection = source_projection(backing, base)?;
    if candidate.place.incoming_reference.is_valid() {
        // The checking coordinate keeps the formal's descriptor home prefix;
        // the incoming identity's pointee coordinate begins after that home.
        // Compare the entire relative input before appending the output path.
        let home = &candidate.source_place;
        if home.root != facts::PlaceRoot::Symbol(base.root)
            || candidate.place.root != home.root
            || !base.segments.starts_with(&home.segments)
            || base.segments[home.segments.len()..] != candidate.place.segments
        {
            return None;
        }
        let mut place = candidate.place.clone();
        place.segments.extend_from_slice(projection);
        Some(place)
    } else {
        (candidate.place.root == facts::PlaceRoot::Symbol(base.root)
            && candidate.place.segments == base.segments)
            .then(|| checked_trees::CapturedPlace {
                incoming_reference: Default::default(),
                incoming_owner_path: Default::default(),
                root: facts::PlaceRoot::Symbol(backing.root),
                segments: backing.segments.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::Handle;
    use symbols::SymbolHandle;

    #[test]
    fn lifetime_debt_follows_original_captured_descriptor() {
        let slot = SymbolHandle::from_parts(1, 1);
        let old_pointee = SymbolHandle::from_parts(2, 1);
        let original_loan = Handle::from_parts(1, 1);
        let backing = validation::LocalWriteSource {
            invocation_output: None,
            invocation_result: None,
            is_image_storage: false,
            recasts: Vec::new(),
            representation: None,
            path: String::new(),
            collection_coarse: false,
            root: slot,
            segments: vec![facts::PlaceSegment::FixedIndex { index: 0 }],
        };
        let candidate = super::super::RebasedBorrowPlace {
            parameter_referent: None,
            source_place: checked_trees::CapturedPlace {
                root: facts::PlaceRoot::Symbol(slot),
                segments: backing.segments.clone(),
                ..Default::default()
            },
            place: checked_trees::CapturedPlace {
                root: facts::PlaceRoot::Symbol(old_pointee),
                ..Default::default()
            },
            representation_address: None,
            retained_operand: None,
            source_owner_symbol: slot,
            parent_loan: original_loan,
            parent_lineage_is_retained: false,
            source_loan: original_loan,
            lifetime_access: None,
        };
        assert_eq!(
            transferred_place(&backing, None, true, &candidate),
            Some(candidate.place.clone()),
            "the old descriptor's lifetime stays with its captured pointee",
        );
        assert_eq!(
            transferred_place(&backing, None, false, &candidate),
            None,
            "this lifetime association cannot project physical permission",
        );
        for incomplete in [
            super::super::RebasedBorrowPlace {
                source_loan: Handle::invalid(),
                ..candidate.clone()
            },
            super::super::RebasedBorrowPlace {
                parent_loan: Handle::invalid(),
                ..candidate.clone()
            },
            super::super::RebasedBorrowPlace {
                source_place: checked_trees::CapturedPlace {
                    root: facts::PlaceRoot::Symbol(slot),
                    segments: vec![facts::PlaceSegment::FixedIndex { index: 1 }],
                    ..Default::default()
                },
                ..candidate
            },
        ] {
            assert_eq!(transferred_place(&backing, None, true, &incomplete), None);
        }
    }
}
