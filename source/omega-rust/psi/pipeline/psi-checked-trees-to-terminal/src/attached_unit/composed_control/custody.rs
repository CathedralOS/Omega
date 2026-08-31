//! Exact empty or whole-root linear custody through both conditional arms.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum ComposedCustody {
    Empty,
    WholeRootLinear {
        entry_claim: PermissionClaimIdentity,
        leaf_claims: [PermissionClaimIdentity; 2],
    },
}

pub(super) fn admit(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    entry: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    leaves: [&psi_checked_trees::CheckedComposedUnitControlStatePlan; 2],
    successors: [&psi_checked_trees::CheckedStructuralControlSuccessorPlan; 2],
) -> Result<ComposedCustody, LoweringError> {
    if entry.structural_parameters.is_empty()
        && entry.entry_claims.is_empty()
        && leaves
            .iter()
            .all(|leaf| leaf.structural_parameters.is_empty() && leaf.entry_claims.is_empty())
    {
        if successors.iter().any(|successor| {
            !successor.transfers.is_empty()
                || !successor.scalar_arguments.is_empty()
                || !successor
                    .trivial_affine_discard_parameter_positions
                    .is_empty()
        }) || leaves.iter().any(|leaf| {
            !matches!(
                leaf.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::BoundaryCall {
                    structural_arguments,
                    completion_receipts,
                    ..
                }] if structural_arguments.is_empty() && completion_receipts.is_empty()
            )
        }) {
            return unsupported("composed Unit empty custody drifted across an edge or leaf");
        }
        return Ok(ComposedCustody::Empty);
    }

    let ([entry_parameter], [entry_claim]) = (
        entry.structural_parameters.as_slice(),
        entry.entry_claims.as_slice(),
    ) else {
        return unsupported("composed Unit entry is outside the exact whole-root linear slice");
    };
    validate_parameter(entry_parameter, &entry_parameter.type_identity)?;
    validate_claim(entry_claim, plan.machine, entry.state)?;

    let mut leaf_claims = [PermissionClaimIdentity::Unknown; 2];
    for (index, (leaf, successor)) in leaves.into_iter().zip(successors).enumerate() {
        let ([leaf_parameter], [leaf_claim], [transfer]) = (
            leaf.structural_parameters.as_slice(),
            leaf.entry_claims.as_slice(),
            successor.transfers.as_slice(),
        ) else {
            return unsupported("composed Unit leaf lost its exact whole-root transfer");
        };
        validate_parameter(leaf_parameter, &entry_parameter.type_identity)?;
        validate_claim(leaf_claim, plan.machine, leaf.state)?;
        if transfer.source_parameter_index != 0
            || transfer.target_parameter_index != 0
            || !successor.scalar_arguments.is_empty()
            || !successor
                .trivial_affine_discard_parameter_positions
                .is_empty()
        {
            return unsupported("composed Unit whole-root edge map drifted");
        }
        let [
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                target_state,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            },
        ] = leaf.operations.as_slice()
        else {
            return unsupported("composed Unit linear leaf is not one boundary call");
        };
        if !scalar_arguments.is_empty()
            || !matches!(structural_arguments.as_slice(), [argument]
                if argument.source_parameter_index == 0
                    && argument.path.is_empty()
                    && argument.type_identity == entry_parameter.type_identity
                    && argument.access == psi_checked_trees::CheckedStructuralAccess::Owned
                    && argument.byte_sequence_literal.is_none())
            || !matches!(completion_receipts.as_slice(), [receipt]
                if receipt.claim_identity == leaf_claim.claim_identity
                    && receipt.argument_index == 0)
        {
            return unsupported("composed Unit linear boundary custody drifted");
        }
        retain_edge_alias(
            checked,
            plan.machine,
            entry.state,
            successor,
            entry_claim.claim_identity,
            leaf_claim.claim_identity,
        )?;
        retain_call_consumption(
            checked,
            plan.machine,
            leaf.state,
            *coordinate,
            *target_state,
            leaf_claim.claim_identity,
        )?;
        leaf_claims[index] = leaf_claim.claim_identity;
    }
    if leaf_claims[0] == leaf_claims[1] || leaf_claims.contains(&entry_claim.claim_identity) {
        return unsupported("composed Unit claim aliases are not state-distinct");
    }
    Ok(ComposedCustody::WholeRootLinear {
        entry_claim: entry_claim.claim_identity,
        leaf_claims,
    })
}

pub(super) fn validate_boundary(
    custody: ComposedCustody,
    boundary: &CheckedBoundaryMachinePlan,
) -> Result<(), LoweringError> {
    match custody {
        ComposedCustody::Empty => {
            if boundary.attachment_type_identity.is_some()
                || !boundary.structural_parameters.is_empty()
                || !boundary.domain_requirements.is_empty()
                || boundary.result_type.is_some()
            {
                return unsupported("composed Unit boundary is not scalar-only Unit");
            }
        }
        ComposedCustody::WholeRootLinear { .. } => {
            let ([parameter], Some(attachment)) = (
                boundary.structural_parameters.as_slice(),
                boundary.attachment_type_identity.as_ref(),
            ) else {
                return unsupported("composed Unit settlement is not one attached parameter");
            };
            validate_parameter(parameter, attachment)?;
            if !parameter.is_self
                || !boundary.scalar_parameters.is_empty()
                || !boundary.domain_requirements.is_empty()
                || boundary.result_type.is_some()
            {
                return unsupported("composed Unit settlement escaped the exact linear boundary");
            }
        }
    }
    Ok(())
}

fn validate_parameter(
    parameter: &psi_checked_trees::CheckedUnitStructuralParameterPlan,
    expected_type: &str,
) -> Result<(), LoweringError> {
    if parameter.type_identity != expected_type
        || parameter.multiplicity != Multiplicity::Linear
        || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
        || !parameter.qualifications.is_empty()
    {
        return unsupported("composed Unit parameter is not qualification-free owned linear");
    }
    Ok(())
}

fn validate_claim(
    claim: &CheckedUnitEntryClaimPlan,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: psi_language_semantics::PermissionEventSource::StateEntry,
        ..
    } = claim.claim_identity
    else {
        return unsupported("composed Unit claim has no exact state-entry identity");
    };
    if machine_symbol != machine
        || state_symbol != state
        || claim.parameter_index != 0
        || !claim.path.is_empty()
        || claim.carry != CarryPolicy::STRICT
    {
        return unsupported("composed Unit claim is not the exact whole-root entry claim");
    }
    Ok(())
}

fn retain_edge_alias(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    entry_state: psi_symbols::SymbolHandle,
    successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
    source_claim: PermissionClaimIdentity,
    target_claim: PermissionClaimIdentity,
) -> Result<(), LoweringError> {
    let statement_index = usize::try_from(successor.statement_ordinal)
        .map_err(|_| LoweringError::Unsupported("composed Unit edge ordinal exceeds usize"))?;
    let events = &checked.facts.flow.ownership;
    let transferred = events
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == entry_state
                && event.source
                    == psi_language_semantics::PermissionEventSource::Call {
                        statement_index,
                        call_ordinal: 0,
                        target_symbol: successor.target_state,
                    }
                && event.kind == psi_language_semantics::PermissionEventKind::Transfer
                && event.access == psi_language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity == source_claim
                && events.segments.span_or_empty(event.segments).is_empty()
        })
        .count();
    let established = events
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == successor.target_state
                && event.source == psi_language_semantics::PermissionEventSource::StateEntry
                && event.kind == psi_language_semantics::PermissionEventKind::Establish
                && event.access == psi_language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity == target_claim
                && events.segments.span_or_empty(event.segments).is_empty()
        })
        .count();
    if transferred != 1 || established != 1 {
        return unsupported("composed Unit edge claim alias does not replay checked ownership");
    }
    Ok(())
}

fn retain_call_consumption(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    target: psi_symbols::SymbolHandle,
    claim: PermissionClaimIdentity,
) -> Result<(), LoweringError> {
    let statement_index = usize::try_from(coordinate.statement_index)
        .map_err(|_| LoweringError::Unsupported("composed Unit call coordinate exceeds usize"))?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("composed Unit call coordinate exceeds usize"))?;
    let events = &checked.facts.flow.ownership;
    let matches = events
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source
                    == psi_language_semantics::PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        target_symbol: target,
                    }
                && event.kind == psi_language_semantics::PermissionEventKind::Consume
                && event.access == psi_language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity == claim
                && events.segments.span_or_empty(event.segments).is_empty()
        })
        .count();
    if matches != 1 {
        return unsupported("composed Unit boundary claim consumption does not replay");
    }
    Ok(())
}
