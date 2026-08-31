use psi_checked_trees::{
    BorrowAccessKind, BorrowLoanOwnerSegment, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedParentBorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowContainmentKind, CheckedReborrowResourceDisposition, FlowBorrowWeakeningReason,
    FlowConstraintKind, FlowInvalidationSource,
};
use psi_core::MachineId;
use psi_terminal::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRootHandoff,
};
use sha2::{Digest, Sha256};

use crate::{CheckedTrees, LoweringError, unsupported};

fn identity(
    checked: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    if let Ok(identity) = checked.typed.normalized_hermetic_symbol_identity(symbol) {
        return Ok(identity);
    }
    if !symbol.is_valid() {
        return unsupported("reborrow custody contains an unresolved symbol");
    }
    let mut digest = Sha256::new();
    digest.update(b"omega-terminal-reborrow-symbol-v1");
    digest.update(checked.typed.symbols.display_path(symbol, "::").as_bytes());
    digest.update(symbol.arena_index().to_le_bytes());
    digest.update(symbol.generation().to_le_bytes());
    Ok(format!("terminal-borrow:{:x}", digest.finalize()))
}

fn ordinal(value: usize) -> Result<u64, LoweringError> {
    value
        .try_into()
        .map_err(|_| LoweringError::Unsupported("reborrow custody coordinate is too large"))
}

fn boundary(
    checked: &CheckedTrees,
    source: FlowInvalidationSource,
) -> Result<TerminalBorrowBoundarySource, LoweringError> {
    Ok(match source {
        FlowInvalidationSource::Statement { statement_index } => {
            TerminalBorrowBoundarySource::Statement {
                statement_index: ordinal(statement_index)?,
            }
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => TerminalBorrowBoundarySource::Call {
            statement_index: ordinal(statement_index)?,
            call_ordinal: ordinal(call_ordinal)?,
            target_identity: identity(checked, target_symbol)?,
        },
    })
}

fn owner_path(
    checked: &CheckedTrees,
    path: &[BorrowLoanOwnerSegment],
) -> Result<Vec<TerminalBorrowOwnerSegment>, LoweringError> {
    path.iter()
        .map(|segment| {
            Ok(match segment {
                BorrowLoanOwnerSegment::Field(symbol) => {
                    TerminalBorrowOwnerSegment::Field(identity(checked, *symbol)?)
                }
                BorrowLoanOwnerSegment::Case(symbol) => {
                    TerminalBorrowOwnerSegment::Case(identity(checked, *symbol)?)
                }
                BorrowLoanOwnerSegment::FixedIndex(index) => {
                    TerminalBorrowOwnerSegment::FixedIndex(ordinal(*index)?)
                }
                BorrowLoanOwnerSegment::DynamicIndex => TerminalBorrowOwnerSegment::DynamicIndex,
            })
        })
        .collect()
}

fn place_segments(
    checked: &CheckedTrees,
    segments: &[psi_facts::PlaceSegment],
) -> Result<Vec<TerminalBorrowPlaceSegment>, LoweringError> {
    segments
        .iter()
        .map(|segment| {
            Ok(match segment {
                psi_facts::PlaceSegment::Field { symbol } => {
                    TerminalBorrowPlaceSegment::Field(identity(checked, *symbol)?)
                }
                psi_facts::PlaceSegment::Case { variant } => {
                    TerminalBorrowPlaceSegment::Case(identity(checked, *variant)?)
                }
                psi_facts::PlaceSegment::FixedIndex { index } => {
                    TerminalBorrowPlaceSegment::FixedIndex(ordinal(*index)?)
                }
                psi_facts::PlaceSegment::FixedRange { start, end } => {
                    TerminalBorrowPlaceSegment::FixedRange {
                        start: ordinal(*start)?,
                        end: ordinal(*end)?,
                    }
                }
                psi_facts::PlaceSegment::Index { .. } => {
                    return unsupported("dynamic projection cannot publish reborrow root custody");
                }
            })
        })
        .collect()
}

fn place(
    checked: &CheckedTrees,
    place: &psi_checked_trees::CapturedPlace,
) -> Result<TerminalBorrowPlace, LoweringError> {
    Ok(TerminalBorrowPlace {
        root_identity: identity(checked, place.root_symbol)?,
        segments: place_segments(checked, &place.segments)?,
    })
}

fn access(kind: BorrowAccessKind) -> StructuralAccess {
    match kind {
        BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
        BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
        BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    }
}

pub(crate) fn retain_selected_reborrow_root_handoffs(
    checked: &CheckedTrees,
    source_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    rows: &mut Vec<TerminalReborrowRootHandoff>,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    for (child_handle, child) in borrow.reborrow_loan_resources.iter() {
        if child.machine_symbol != source_machine
            || child.weakening_reason != FlowBorrowWeakeningReason::StateExit
        {
            continue;
        }
        let CheckedParentBorrowResource::DirectRoot {
            resource: parent_handle,
        } = child.parent_resource
        else {
            continue;
        };
        let parent = borrow.direct_loan_resources.get(parent_handle);
        if child.access_effect != CheckedReborrowAccessEffect::ExclusiveSuspension
            || parent.weakening_reason != FlowBorrowWeakeningReason::StateExit
            || parent.access != BorrowAccessKind::Mutable
            || !matches!(
                child.access,
                BorrowAccessKind::Mutable | BorrowAccessKind::WriteOnly
            )
        {
            return unsupported("reborrow root custody access or lifecycle is not permitted");
        }

        let events = borrow
            .reborrow_disposition_events
            .iter()
            .filter(|(_, event)| event.child_resource == child_handle)
            .map(|(_, event)| event)
            .collect::<Vec<_>>();
        let [event] = events.as_slice() else {
            return unsupported("reborrow root custody requires one disposition event");
        };
        let certificates = borrow
            .reborrow_containment_certificates
            .iter()
            .filter(|(_, certificate)| certificate.child_resource == child_handle)
            .map(|(_, certificate)| certificate)
            .collect::<Vec<_>>();
        let [certificate] = certificates.as_slice() else {
            return unsupported("reborrow root custody requires one containment certificate");
        };

        let suspension = &child.parent_suspension;
        let child_activation = checked
            .facts
            .flow
            .borrow_lifetimes
            .activations
            .get(suspension.child_activation);
        let child_weakening = checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(child.parent_end_status.child_weakening);
        let parent_weakening = checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(child.parent_end_status.parent_weakening);
        let parent_constraint = checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .get(suspension.parent_entry_constraint);
        let exact_event = event.machine_symbol == child.machine_symbol
            && event.state_symbol == child.state_symbol
            && event.child_loan == child.loan
            && event.child_activation == suspension.child_activation
            && event.child_weakening == child.parent_end_status.child_weakening
            && event.parent_loan == child.parent_loan
            && event.parent_resource == child.parent_resource
            && event.boundary_source == child.weakening_source
            && event.boundary_phase == CheckedBorrowResourceLifecyclePhase::StateExit
            && event.shared_cohort.is_empty()
            && event.retired_parent_path.len() == 1
            && event.retired_parent_path[0].resource == child.parent_resource
            && event.retired_parent_path[0].weakening == child.parent_end_status.parent_weakening
            && event.final_target
                == CheckedBorrowResourceDispositionTarget::DirectRootLifetime(
                    parent.parent_lifetime.clone(),
                )
            && event.disposition == CheckedReborrowResourceDisposition::StateExitDirectRootHandoff;
        let exact_certificate = certificate.machine_symbol == child.machine_symbol
            && certificate.state_symbol == child.state_symbol
            && certificate.child_loan == child.loan
            && certificate.parent_loan == child.parent_loan
            && certificate.parent_resource == child.parent_resource
            && certificate.parent_access == parent.access
            && certificate.child_access == child.access
            && certificate.access_effect == child.access_effect
            && certificate.child_activation == suspension.child_activation
            && certificate.parent_entry_constraint == suspension.parent_entry_constraint
            && certificate.formation_source == suspension.source
            && certificate.child_weakening == child.parent_end_status.child_weakening
            && certificate.parent_weakening == child.parent_end_status.parent_weakening
            && certificate.child_weakening_source == child.weakening_source
            && certificate.child_weakening_reason == child.weakening_reason
            && certificate.parent_place == parent.captured_place
            && certificate.child_place == child.captured_place
            && certificate.projection_remainder
                == child.captured_place.segments[parent.captured_place.segments.len()..]
            && certificate.containment == CheckedReborrowContainmentKind::ExclusiveSuspension;
        let exact_resources = child.loan == child_activation.loan
            && child.loan == child_weakening.loan
            && child.parent_loan == parent.loan
            && child.parent_loan == parent_weakening.loan
            && suspension.child_loan == child.loan
            && suspension.parent_loan == child.parent_loan
            && suspension.parent_resource == child.parent_resource
            && suspension.source == child.activation_source
            && matches!(
                parent_constraint.kind,
                FlowConstraintKind::BorrowLoan { loan } if loan == child.parent_loan
            )
            && parent.machine_symbol == child.machine_symbol
            && parent.state_symbol == child.state_symbol
            && parent.parent_lifetime.machine_symbol == child.machine_symbol
            && parent.parent_lifetime.state_symbol == child.state_symbol
            && parent.parent_lifetime.root_symbol == parent.captured_place.root_symbol;
        if !exact_event || !exact_certificate || !exact_resources {
            return unsupported("reborrow root custody checked replay disagrees");
        }

        rows.push(TerminalReborrowRootHandoff {
            machine: terminal_machine,
            source_machine_identity: identity(checked, child.machine_symbol)?,
            source_state_identity: identity(checked, child.state_symbol)?,
            direct_root_owner_identity: identity(checked, parent.owner_symbol)?,
            direct_root_owner_path: owner_path(checked, &parent.owner_path)?,
            child_owner_identity: identity(checked, child.owner_symbol)?,
            child_owner_path: owner_path(checked, &child.owner_path)?,
            direct_root_place: place(checked, &parent.captured_place)?,
            child_place: place(checked, &child.captured_place)?,
            projection_remainder: place_segments(checked, &certificate.projection_remainder)?,
            direct_root_access: access(parent.access.clone()),
            child_access: access(child.access.clone()),
            direct_root_activation: boundary(checked, parent.activation_source)?,
            child_activation: boundary(checked, child_activation.source)?,
            formation_boundary: boundary(checked, suspension.source)?,
            child_weakening: boundary(checked, child_weakening.source)?,
            direct_root_weakening: boundary(checked, parent_weakening.source)?,
            direct_root_lifetime_identity: identity(checked, parent.parent_lifetime.root_symbol)?,
        });
    }
    rows.sort();
    if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
        return unsupported("duplicate reborrow root custody publication");
    }
    Ok(())
}
