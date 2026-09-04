use psi_checked_trees::{
    BorrowAccessKind, BorrowLoanOwnerSegment, CheckedBorrowResourceDispositionTarget,
    CheckedBorrowResourceLifecyclePhase, CheckedParentBorrowResource, CheckedReborrowAccessEffect,
    CheckedReborrowContainmentKind, CheckedReborrowResourceDisposition, FlowBorrowWeakeningReason,
    FlowConstraintKind, FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
};
use psi_core::MachineId;
use psi_terminal::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRootHandoff,
    TerminalReborrowRootHandoffStep,
};
use sha2::{Digest, Sha256};

use crate::{CheckedTrees, LoweringError, unsupported};

pub(crate) fn identity(
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

pub(crate) fn boundary(
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

pub(crate) fn owner_path(
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

pub(crate) fn place_segments(
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

pub(crate) fn place(
    checked: &CheckedTrees,
    place: &psi_checked_trees::CapturedPlace,
) -> Result<TerminalBorrowPlace, LoweringError> {
    Ok(TerminalBorrowPlace {
        root_identity: identity(checked, place.root_symbol)?,
        segments: place_segments(checked, &place.segments)?,
    })
}

pub(crate) fn access(kind: BorrowAccessKind) -> StructuralAccess {
    match kind {
        BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
        BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
        BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    }
}

fn retained_parent_status(
    checked: &CheckedTrees,
    child: &psi_checked_trees::CheckedReborrowLoanResource,
) -> Result<ParentLexicalStatusAtChildEnd, LoweringError> {
    let parent = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.parent_weakening);
    let child_weakening = checked
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .get(child.parent_end_status.child_weakening);
    let coordinate = |source, reason| {
        let FlowInvalidationSource::Statement { statement_index } = source else {
            return None;
        };
        let phase = match reason {
            FlowBorrowWeakeningReason::LastUseExpired => 0_u8,
            FlowBorrowWeakeningReason::LocalReassigned => 1,
            FlowBorrowWeakeningReason::StateExit => 2,
        };
        Some((statement_index, phase))
    };
    let parent_coordinate = coordinate(parent.source, parent.reason).ok_or(
        LoweringError::Unsupported("reborrow custody weakening is not state-local"),
    )?;
    let child_coordinate = coordinate(child_weakening.source, child_weakening.reason).ok_or(
        LoweringError::Unsupported("reborrow custody weakening is not state-local"),
    )?;
    Ok(match parent_coordinate.cmp(&child_coordinate) {
        std::cmp::Ordering::Less => ParentLexicalStatusAtChildEnd::RetiredBeforeChild,
        std::cmp::Ordering::Equal => ParentLexicalStatusAtChildEnd::RetiredWithChild,
        std::cmp::Ordering::Greater => ParentLexicalStatusAtChildEnd::LivePastChild,
    })
}

pub(crate) fn retain_selected_reborrow_root_handoffs(
    checked: &CheckedTrees,
    source_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    rows: &mut Vec<TerminalReborrowRootHandoff>,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let mut retained_children = Vec::new();
    for (_, event) in borrow.reborrow_disposition_events.iter() {
        if event.machine_symbol != source_machine
            || event.disposition != CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
        {
            continue;
        }
        let mut lineage = Vec::new();
        let mut next = event.child_resource;
        let parent_handle = loop {
            if lineage.iter().any(|(handle, _)| *handle == next) {
                return unsupported("reborrow root custody lineage contains a cycle");
            }
            let child = borrow.reborrow_loan_resources.get(next);
            if child.machine_symbol != source_machine || child.state_symbol != event.state_symbol {
                return unsupported("reborrow root custody crosses a machine or state");
            }
            lineage.push((next, child));
            match child.parent_resource {
                CheckedParentBorrowResource::DirectRoot { resource } => break resource,
                CheckedParentBorrowResource::Reborrow { resource } => next = resource,
            }
        };
        lineage.reverse();
        let parent = borrow.direct_loan_resources.get(parent_handle);
        if parent.machine_symbol != source_machine
            || parent.state_symbol != event.state_symbol
            || parent.access != BorrowAccessKind::Mutable
            || parent.parent_lifetime.machine_symbol != source_machine
            || parent.parent_lifetime.state_symbol != event.state_symbol
            || parent.parent_lifetime.root_symbol != parent.captured_place.root_symbol
        {
            return unsupported("reborrow root custody direct-root identity disagrees");
        }

        let mut expected_parent = CheckedParentBorrowResource::DirectRoot {
            resource: parent_handle,
        };
        let mut expected_parent_loan = parent.loan;
        let mut expected_parent_access = parent.access.clone();
        let mut expected_parent_place = &parent.captured_place;
        let mut terminal_lineage = Vec::with_capacity(lineage.len());
        for (child_handle, child) in &lineage {
            let children = borrow
                .reborrow_loan_resources
                .iter()
                .filter(|(_, candidate)| {
                    candidate.machine_symbol == source_machine
                        && candidate.state_symbol == event.state_symbol
                        && candidate.parent_resource == expected_parent
                })
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            if children.as_slice() != [*child_handle] {
                return unsupported("reborrow root custody requires one linear exclusive branch");
            }
            if child.parent_resource != expected_parent
                || child.parent_loan != expected_parent_loan
                || child.parent_access != expected_parent_access
                || child.access_effect != CheckedReborrowAccessEffect::ExclusiveSuspension
                || expected_parent_access.direct_reborrow_effect(&child.access)
                    != Some(CheckedReborrowAccessEffect::ExclusiveSuspension)
                || child.parent_end_status.status != retained_parent_status(checked, child)?
            {
                return unsupported("reborrow root custody lineage access or lifecycle disagrees");
            }

            let certificates = borrow
                .reborrow_containment_certificates
                .iter()
                .filter(|(_, certificate)| certificate.child_resource == *child_handle)
                .map(|(_, certificate)| certificate)
                .collect::<Vec<_>>();
            let [certificate] = certificates.as_slice() else {
                return unsupported(
                    "reborrow root custody requires one containment certificate per edge",
                );
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
            let exact_certificate = certificate.machine_symbol == child.machine_symbol
                && certificate.state_symbol == child.state_symbol
                && certificate.child_loan == child.loan
                && certificate.parent_loan == child.parent_loan
                && certificate.parent_resource == child.parent_resource
                && certificate.parent_access == expected_parent_access
                && certificate.child_access == child.access
                && certificate.access_effect == child.access_effect
                && certificate.child_activation == suspension.child_activation
                && certificate.parent_entry_constraint == suspension.parent_entry_constraint
                && certificate.formation_source == suspension.source
                && certificate.child_weakening == child.parent_end_status.child_weakening
                && certificate.parent_weakening == child.parent_end_status.parent_weakening
                && certificate.child_weakening_source == child.weakening_source
                && certificate.child_weakening_reason == child.weakening_reason
                && certificate.parent_place == *expected_parent_place
                && certificate.child_place == child.captured_place
                && child
                    .captured_place
                    .segments
                    .starts_with(&expected_parent_place.segments)
                && certificate.projection_remainder
                    == child.captured_place.segments[expected_parent_place.segments.len()..]
                && certificate.containment == CheckedReborrowContainmentKind::ExclusiveSuspension;
            let exact_resources = child.loan == child_activation.loan
                && child.loan == child_weakening.loan
                && child.parent_loan == parent_weakening.loan
                && suspension.child_loan == child.loan
                && suspension.parent_loan == child.parent_loan
                && suspension.parent_resource == child.parent_resource
                && suspension.source == child.activation_source
                && child.parent_end_status.child_loan == child.loan
                && child.parent_end_status.parent_loan == child.parent_loan
                && child.parent_end_status.parent_resource == child.parent_resource
                && child.restoration.child_loan == child.loan
                && child.restoration.parent_loan == child.parent_loan
                && child.restoration.parent_resource == child.parent_resource
                && child.restoration.child_weakening_source == child.weakening_source
                && child.restoration.child_weakening_reason == child.weakening_reason
                && matches!(
                    parent_constraint.kind,
                    FlowConstraintKind::BorrowLoan { loan } if loan == child.parent_loan
                );
            if !exact_certificate || !exact_resources {
                return unsupported("reborrow root custody checked replay disagrees");
            }

            terminal_lineage.push(TerminalReborrowRootHandoffStep {
                child_owner_identity: identity(checked, child.owner_symbol)?,
                child_owner_path: owner_path(checked, &child.owner_path)?,
                child_place: place(checked, &child.captured_place)?,
                projection_remainder: place_segments(checked, &certificate.projection_remainder)?,
                child_access: access(child.access.clone()),
                child_activation: boundary(checked, child_activation.source)?,
                formation_boundary: boundary(checked, suspension.source)?,
                child_weakening: boundary(checked, child_weakening.source)?,
            });
            expected_parent = CheckedParentBorrowResource::Reborrow {
                resource: *child_handle,
            };
            expected_parent_loan = child.loan;
            expected_parent_access = child.access.clone();
            expected_parent_place = &child.captured_place;
        }

        let (leaf_handle, leaf) = lineage.last().copied().ok_or(LoweringError::Unsupported(
            "reborrow root custody lineage is empty",
        ))?;
        if borrow.reborrow_loan_resources.iter().any(|(_, candidate)| {
            candidate.parent_resource
                == CheckedParentBorrowResource::Reborrow {
                    resource: leaf_handle,
                }
        }) {
            return unsupported("reborrow root custody leaf has an unretained branch");
        }
        let events = borrow
            .reborrow_disposition_events
            .iter()
            .filter(|(_, candidate)| candidate.child_resource == leaf_handle)
            .map(|(_, candidate)| candidate)
            .collect::<Vec<_>>();
        let [exact_leaf_event] = events.as_slice() else {
            return unsupported("reborrow root custody requires one leaf disposition event");
        };
        let exact_path = event.retired_parent_path.len() == lineage.len()
            && event
                .retired_parent_path
                .iter()
                .zip(lineage.iter().rev())
                .all(|(step, (_, child))| {
                    step.resource == child.parent_resource
                        && step.weakening == child.parent_end_status.parent_weakening
                });
        let exact_event = std::ptr::eq(*exact_leaf_event, event)
            && event.child_loan == leaf.loan
            && event.child_activation == leaf.parent_suspension.child_activation
            && event.child_weakening == leaf.parent_end_status.child_weakening
            && event.parent_loan == leaf.parent_loan
            && event.parent_resource == leaf.parent_resource
            && event.boundary_source == leaf.weakening_source
            && leaf.weakening_reason == FlowBorrowWeakeningReason::StateExit
            && event.boundary_phase == CheckedBorrowResourceLifecyclePhase::StateExit
            && event.shared_cohort.is_empty()
            && exact_path
            && event.final_target
                == CheckedBorrowResourceDispositionTarget::DirectRootLifetime(
                    parent.parent_lifetime.clone(),
                );
        if !exact_event {
            return unsupported("reborrow root custody terminal disposition disagrees");
        }
        retained_children.extend(lineage.iter().map(|(handle, _)| *handle));
        let root_weakening = checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .get(lineage[0].1.parent_end_status.parent_weakening);
        rows.push(TerminalReborrowRootHandoff {
            machine: terminal_machine,
            source_machine_identity: identity(checked, leaf.machine_symbol)?,
            source_state_identity: identity(checked, leaf.state_symbol)?,
            direct_root_owner_identity: identity(checked, parent.owner_symbol)?,
            direct_root_owner_path: owner_path(checked, &parent.owner_path)?,
            direct_root_place: place(checked, &parent.captured_place)?,
            direct_root_access: access(parent.access.clone()),
            direct_root_activation: boundary(checked, parent.activation_source)?,
            direct_root_weakening: boundary(checked, root_weakening.source)?,
            direct_root_lifetime_identity: identity(checked, parent.parent_lifetime.root_symbol)?,
            lineage: terminal_lineage,
        });
    }
    if borrow
        .reborrow_loan_resources
        .iter()
        .any(|(handle, child)| {
            child.machine_symbol == source_machine
                && child.weakening_reason == FlowBorrowWeakeningReason::StateExit
                && !retained_children.contains(&handle)
        })
    {
        return unsupported("state-exit reborrow lineage cannot publish exact root custody");
    }
    rows.sort();
    if !rows.windows(2).all(|pair| pair[0] < pair[1]) {
        return unsupported("duplicate reborrow root custody publication");
    }
    Ok(())
}
