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
) -> Option<Vec<super::StateLoanTracker>> {
    let effects = frames.stored_reference_effects_at_expression(
        machine,
        state,
        statement_index,
        expression,
    )?;
    let produced = from_effects(program, effects.clone(), incoming);
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
        call_result: loan.call_result,
        owner_path: loan.owner_path,
        place: loan.place,
        retained_operand: loan.retained_operand,
    }));
    Some(current)
}

pub(super) fn loans(
    program: &dyn ::validation::CheckingInput,
    _state: &typed_trees::state::State,
    machine_symbol: symbols::SymbolHandle,
    statement: &checked_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    loan_trackers: &[super::StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(machine) = program.machine(machine_symbol) else {
        return Vec::new();
    };
    let Some(effects) = call_frames
        .and_then(|frames| frames.stored_reference_fallthrough_effects(machine, statement))
    else {
        return Vec::new();
    };
    from_effects(program, effects, loan_trackers)
}

pub(in crate::borrow) fn from_effects(
    program: &dyn ::validation::CheckingInput,
    effects: Vec<validation::StoredReferenceEffect>,
    loan_trackers: &[super::StateLoanTracker],
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
                    call_result: true,
                });
            }
        }
        for source in &effect.sources {
            let backing = &source.backing;
            if let Some(identity) = backing.invocation_output {
                // Both a supplied pointer and an already captured output
                // retained as a lifetime debt have this exact opaque namespace.
                // A debt does not become the current descriptor slot's address
                // or acquire a supplied permission. All other input debts and
                // alternatives remain independent rows of the complete relation.
                let access = source.lifetime_access.unwrap_or(effect.access);
                let supplied = (identity.is_valid()
                    && !backing.is_image_storage
                    && backing.recasts.is_empty()
                    && backing.representation.is_none())
                .then(|| validation::find_state(program, identity.state_symbol))
                .flatten()
                .and_then(|(machine, state)| {
                    let parameter = validation::invocation_output_parameter(program, identity)?;
                    let reference = validation::project_type_reference_from_segments(
                        program,
                        parameter.type_reference,
                        &backing.segments,
                    )?;
                    let shell = validation::reference_shell(program, reference)?;
                    (shell.access == access).then_some((machine, state, parameter.type_reference))
                });
                if let Some((machine, state, carrier_type)) = supplied {
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
                        call_result: true,
                    });
                    continue;
                }
                // The explicit output alternative is malformed/unavailable.
                // Keep its zero-root unknown alternative below; deleting it
                // would let a sibling source falsely prove a complete result.
            }
            let captured = super::call_views::rebase_borrow_places_through_local_loans(
                program,
                checked_trees::CapturedPlace {
                    incoming_reference: Default::default(),
                    root: facts::PlaceRoot::Symbol(source.carrier_symbol),
                    segments: source.carrier_segments.clone(),
                },
                loan_trackers,
            );
            // A completed source records its input referent before deriving
            // the output view. Account for every actual carrier alternative
            // across the complete source roster, never a convenient subset.
            // Lifetime debts are separate obligations, not spatial choices.
            let relevant = |candidate: &&super::RebasedBorrowPlace| {
                source.lifetime_access.is_some() || !candidate.lifetime_only
            };
            let complete_carrier = captured.iter().filter(relevant).next().is_some()
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
                        || supplier.is_some_and(|source| source.lifetime_only)
                    {
                        checked_trees::BorrowLoanLineage::LifetimeDependency
                    } else {
                        checked_trees::BorrowLoanLineage::UnretainedDerived
                    },
                    kind: checked_trees::BorrowAccessKind::from(
                        source.lifetime_access.unwrap_or(effect.access),
                    ),
                    call_result: true,
                });
            }
        }
    }
    loans
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
    if candidate.lifetime_only && !lifetime_only {
        return None;
    }
    // Preserve the existing exact join, including its presentation metadata
    // consumers. Only a genuinely derived output needs the additional base.
    if (candidate.place.root == facts::PlaceRoot::Symbol(backing.root)
        && candidate.place.segments == backing.segments)
        || (candidate.place.incoming_reference.is_valid()
            && candidate.source_place.root == facts::PlaceRoot::Symbol(backing.root)
            && candidate.source_place.segments == backing.segments)
    {
        return Some(candidate.place.clone());
    }
    let base = carrier_backing?;
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
    let projection = &backing.segments[base.segments.len()..];
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
                root: facts::PlaceRoot::Symbol(backing.root),
                segments: backing.segments.clone(),
            })
    }
}
