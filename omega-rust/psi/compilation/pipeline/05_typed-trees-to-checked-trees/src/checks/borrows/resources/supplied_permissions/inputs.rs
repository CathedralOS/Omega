//! A saved output keeps the references selected before its helper writes.
//! The short borrow used to reach a slot is separate from a stored reference's
//! own backing. Preserve every frozen member and its declared access ceiling.
use super::super::call_operands::ResourceDraft;
use super::super::permission_sources;
use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, CapturedPlace, CheckFacts, CheckedCallBorrowLoanResource,
    CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime, FlowCallFact,
    FlowOperandRetainedSupplier, FlowOperandStoredSource, FlowStateFact,
};
use language_semantics::ReferenceAccess;

pub(super) enum InputPermissionDraft {
    Borrowed(ResourceDraft),
    Incoming {
        place: CapturedPlace,
        access: ReferenceAccess,
        owner_path: Vec<checked_trees::BorrowLoanOwnerSegment>,
    },
}
pub(super) struct InputResourceDraft {
    pub(super) source: Handle<FlowOperandRetainedSupplier>,
    pub(super) stored: Handle<FlowOperandStoredSource>,
    pub(super) permission: InputPermissionDraft,
}
pub(super) struct InputPlan {
    pub(super) resources: Vec<InputResourceDraft>,
    pub(super) parents: Arena<Handle<BorrowLoanFact>>,
}

pub(super) fn plan(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    sources: HandleSpan<FlowOperandRetainedSupplier>,
    ceilings: &[ReferenceAccess],
) -> Result<InputPlan, &'static str> {
    let mut resources = Vec::new();
    let mut parents = Arena::new();
    let control = &facts.flow.control;
    let recorded = control
        .operand_retained_suppliers
        .span(sources)
        .ok_or("completed output lifetime inputs are unavailable")?;
    if recorded.len() != ceilings.len() {
        return Err("completed output input access roster is incomplete");
    }
    for ((handle, supplier), ceiling) in sources.handles().zip(recorded).zip(ceilings) {
        let contract = supplier
            .contract
            .ok_or("completed output lifetime input has no original applied contract")?;
        let source_type = crate::borrow::lifetime_contract::source_type(
            program,
            &facts.borrow,
            control,
            supplier,
        )
        .ok_or("completed output lifetime input does not match its captured call argument")?;
        let source_path = control
            .operand_retained_paths
            .span(contract.source_path)
            .ok_or("completed output input lost its declared source path")?;
        let leaves = if let Some(shell) = validation::reference_shell(program, source_type) {
            vec![(
                Vec::new(),
                checked_trees::BorrowAccessKind::from(shell.access),
            )]
        } else {
            validation::borrow_contracts::carried_reference_paths(program, source_type)
                .ok_or("completed output input has an unresolved reference frontier")?
                .into_iter()
                .map(|(path, access)| (crate::borrow::owner_place_segments(&path), access))
                .collect()
        };
        for (leaf, access) in leaves {
            let access = bounded_access(*ceiling, access)?;
            let mut owner_path =
                crate::borrow::owner_path_from_place_segments(program, source_path);
            owner_path.extend(crate::borrow::owner_path_from_place_segments(
                program, &leaf,
            ));
            if contract.source == checked_trees::ViewReturnInputSource::Referent {
                let mut captured_path = control
                    .operand_retained_paths
                    .span(supplier.projection)
                    .ok_or("completed output input lost its captured projection")?
                    .to_vec();
                captured_path.extend_from_slice(&leaf);
                append_borrowed(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    handle,
                    Handle::invalid(),
                    contract.input,
                    supplier.operand,
                    &captured_path,
                    &owner_path,
                    access,
                    &mut parents,
                    &mut resources,
                )?;
                continue;
            }
            let selected = control
                .operand_stored_sources
                .span(contract.stored_sources)
                .ok_or("completed output input lost its frozen stored frontier")?;
            let mut count = 0;
            for (stored, member) in contract.stored_sources.handles().zip(selected) {
                let (FlowOperandStoredSource::Value { path, .. }
                | FlowOperandStoredSource::Incoming { path, .. }) = member;
                let path = control
                    .operand_retained_paths
                    .span(*path)
                    .ok_or("stored input lost its selected leaf path")?;
                let corresponding =
                    validation::corresponding_place_projection(program, source_type, path)
                        .ok_or("stored input leaf has no applied member correspondence")?;
                let expected =
                    validation::corresponding_place_projection(program, source_type, &leaf)
                        .ok_or("declared input leaf has no applied member correspondence")?;
                if corresponding != expected {
                    continue;
                }
                count += 1;
                match *member {
                    FlowOperandStoredSource::Value {
                        operand,
                        projection,
                        ..
                    } => {
                        let projection = control
                            .operand_retained_paths
                            .span(projection)
                            .ok_or("stored input lost its original captured projection")?;
                        append_borrowed(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            call,
                            handle,
                            stored,
                            contract.input,
                            operand,
                            projection,
                            &owner_path,
                            access.clone(),
                            &mut parents,
                            &mut resources,
                        )?;
                    }
                    FlowOperandStoredSource::Incoming { slot, .. } => {
                        let call_row = control.calls.get(call);
                        let aliases = crate::checks::borrows::completed_backings::incoming_sources(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            call_row.statement_index,
                            call_row.entry_constraints,
                            validation::CanonicalPlace {
                                root: slot.root,
                                segments: control
                                    .operand_referent_segments
                                    .span(slot.segments)
                                    .ok_or("stored input lost its original incoming slot")?
                                    .to_vec(),
                            },
                        )?;
                        if aliases.is_empty() {
                            return Err("stored input has no incoming authority alternatives");
                        }
                        for alias in aliases {
                            let identity = alias.place.incoming_reference;
                            let (_, owner) = facts
                                .borrow
                                .first_state_borrow(state.machine_symbol, state.state_symbol)
                                .ok_or("stored input has no owning borrow state")?;
                            if !facts.borrow.state_owns_incoming_reference(owner, identity) {
                                return Err("stored input has no exact incoming entry permission");
                            }
                            let incoming = facts.borrow.incoming_references.get(identity);
                            let owner = facts
                                .borrow
                                .owner_segments
                                .span(incoming.owner_path)
                                .ok_or("stored input has no declared incoming path")?;
                            let reference = validation::owned_projection_type_reference(
                                program,
                                incoming.carrier_type,
                                &crate::borrow::owner_place_segments(owner),
                            )
                            .and_then(|reference| validation::reference_shell(program, reference))
                            .ok_or("stored input's incoming identity is not a reference")?;
                            let expected = validation::project_type_reference_from_segments(
                                program,
                                source_type,
                                &leaf,
                            )
                            .and_then(|reference| validation::reference_shell(program, reference))
                            .ok_or("stored input's declared leaf is not a reference")?;
                            if !validation::storage_origin_types_match(
                                program,
                                reference.referee,
                                expected.referee,
                            ) {
                                return Err(
                                    "stored input's incoming descriptor has another referent type",
                                );
                            }
                            if reference.access != ReferenceAccess::Mutable
                                && reference.access != reference_access(&access)
                            {
                                return Err(
                                    "stored input's incoming permission cannot provide the declared access",
                                );
                            }
                            resources.push(InputResourceDraft {
                                source: handle,
                                stored,
                                permission: InputPermissionDraft::Incoming {
                                    place: alias.place,
                                    access: reference_access(&access),
                                    owner_path: owner_path.clone(),
                                },
                            });
                        }
                    }
                }
            }
            if count == 0 {
                return Err("stored input does not cover a declared reference leaf");
            }
        }
    }
    Ok(InputPlan { resources, parents })
}

#[allow(clippy::too_many_arguments)]
fn append_borrowed(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    source: Handle<FlowOperandRetainedSupplier>,
    stored: Handle<FlowOperandStoredSource>,
    owner: Handle<checked_trees::FlowOperandFact>,
    operand: Handle<checked_trees::FlowOperandFact>,
    path: &[facts::PlaceSegment],
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    access: checked_trees::BorrowAccessKind,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
    resources: &mut Vec<InputResourceDraft>,
) -> Result<(), &'static str> {
    // A selected value may be an actual of an earlier helper. Its own call
    // supplies the access; the later installer cannot lend it a second time.
    let original = if stored.is_valid() {
        crate::checks::borrows::formation_captures::actual_invocation(
            program, facts, state, operand,
        )?
    } else {
        None
    };
    let lending_call = original.unwrap_or(call);
    let (suppliers, captured_parents) = permission_sources::lending_sources(
        program,
        facts, loan_occurrences,
        state,
        lending_call,
        operand,
        path,
        &access,
    ).inspect_err(|&reason| {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!("CHECKSITE completed-output-input-refused source={source:?} stored={stored:?} owner={owner:?} operand={operand:?} original_call={original:?} lending_call={lending_call:?} path={path:?} access={access:?} reason={reason}");
            let value = facts.flow.control.operands.get(operand);
            if facts.flow.control.operand_retentions.is_valid(value.retention) {
                let profile = facts.flow.control.operand_retentions.get(value.retention);
                eprintln!("CHECKSITE completed-output-input-profile operand={operand:?} retention={:?} profile={profile:?} suppliers={:?} reborrows={:?}", value.retention,
                    facts.flow.control.operand_retained_suppliers.span_or_empty(profile.suppliers),
                    facts.flow.control.operand_reborrow_sources.span_or_empty(profile.reborrow_sources));
            }
        }
    })?;
    if suppliers.is_empty() {
        return Err("completed input has no selected lending permission");
    }
    for mut selected in suppliers {
        if let CheckedCallBorrowLoanSource::CallAccess { parents: old, .. } = &mut selected.source {
            let retained = captured_parents
                .span(*old)
                .ok_or("completed input permission has an unavailable parent roster")?;
            *old = parents.insert_many(retained.iter().copied());
        }
        resources.push(InputResourceDraft {
            source,
            stored,
            permission: InputPermissionDraft::Borrowed(ResourceDraft {
                resource: CheckedCallBorrowLoanResource {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    call,
                    owner_operand: owner,
                    operand,
                    source: selected.source,
                    access: access.clone(),
                    parent_lifetime: CheckedDirectBorrowParentLifetime {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        root: selected.backing.root,
                    },
                    ..Default::default()
                },
                path: owner_path.to_vec(),
                backing: selected.backing,
            }),
        });
    }
    Ok(())
}
fn reference_access(access: &checked_trees::BorrowAccessKind) -> ReferenceAccess {
    match access {
        checked_trees::BorrowAccessKind::Read => ReferenceAccess::Shared,
        checked_trees::BorrowAccessKind::Mutable => ReferenceAccess::Mutable,
        checked_trees::BorrowAccessKind::WriteOnly => ReferenceAccess::WriteOnly,
    }
}
fn bounded_access(
    ceiling: ReferenceAccess,
    access: checked_trees::BorrowAccessKind,
) -> Result<checked_trees::BorrowAccessKind, &'static str> {
    use checked_trees::BorrowAccessKind;
    Ok(match (ceiling, access) {
        (ReferenceAccess::Shared, BorrowAccessKind::WriteOnly)
        | (ReferenceAccess::WriteOnly, BorrowAccessKind::Read) => {
            return Err("completed input's access ceiling permits no requested leaf access");
        }
        (ReferenceAccess::Shared, _) | (_, BorrowAccessKind::Read) => BorrowAccessKind::Read,
        (ReferenceAccess::WriteOnly, _) | (_, BorrowAccessKind::WriteOnly) => {
            BorrowAccessKind::WriteOnly
        }
        _ => BorrowAccessKind::Mutable,
    })
}
