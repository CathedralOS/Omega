//! Exact checked state-entry claims and their machine-local Terminal identities.

use super::*;

pub(super) struct LoweredUnitClaims {
    pub(super) entry_claims: Vec<EntryClaim>,
    pub(super) source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
}

pub(super) fn validate_whole_entry_claims(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    claims: &[CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == language_semantics::PermissionEventSource::StateEntry
                && event.kind == language_semantics::PermissionEventKind::Establish
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    if events.len() != claims.len() {
        return unsupported("structural graph entry claim roster disagrees with checked ownership");
    }
    for claim in claims {
        let parameter =
            parameters
                .get(claim.parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural graph entry claim parameter missing",
                ))?;
        let source = checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "structural graph entry claim source missing",
            ))?;
        if parameter.multiplicity != Multiplicity::Linear
            || !claim.path.is_empty()
            || claim.carry != CarryPolicy::STRICT
            || claims
                .iter()
                .filter(|candidate| candidate.claim_identity == claim.claim_identity)
                .count()
                != 1
            || events
                .iter()
                .filter(|event| {
                    event.claim_identity == claim.claim_identity
                        && event.root == facts::PlaceRoot::Symbol(source.symbol)
                        && event.segments.is_empty()
                })
                .count()
                != 1
            || checked.facts.carry.claim_policies.iter().any(|policy| {
                policy.claim_identity == claim.claim_identity && policy.effective != claim.carry
            })
        {
            return unsupported("structural graph entry claim changed its whole source custody");
        }
    }
    Ok(())
}

pub(super) fn lower_unit_entry_claims(
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    claims: &[CheckedUnitEntryClaimPlan],
    parameters: &[StructuralParameterDeclaration],
) -> Result<LoweredUnitClaims, LoweringError> {
    let mut entry_claims = Vec::with_capacity(claims.len());
    let mut source_claims = Vec::with_capacity(claims.len());
    let mut next_claim = 1_u64;
    for claim in claims {
        if claim.carry != CarryPolicy::STRICT {
            return unsupported("Unit entry claim has a non-default carry policy");
        }
        let parameter = parameters
            .get(usize::try_from(claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("Unit entry claim parameter index exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "Unit entry claim has an invalid parameter index",
            ))?;
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: language_semantics::PermissionEventSource::StateEntry,
            ..
        } = claim.claim_identity
        else {
            return unsupported("Unit entry claim is not an exact checked state-entry claim");
        };
        if machine_symbol != machine || state_symbol != state {
            return unsupported("Unit entry claim belongs to another checked state");
        }
        if source_claims
            .iter()
            .any(|(identity, _)| *identity == claim.claim_identity)
        {
            return unsupported("Unit entry claim identity is duplicated");
        }
        let id = claim_id(allocate_dense(&mut next_claim)?);
        entry_claims.push(EntryClaim {
            claim: id,
            input: parameter.place,
            path: lower_structural_path(&claim.path),
        });
        source_claims.push((claim.claim_identity, id));
    }
    Ok(LoweredUnitClaims {
        entry_claims,
        source_claims,
    })
}

/// Rebase checked identity conservation onto the ordinary invocation and result
/// places. The semantic claim identity, rather than a coincident dense ordinal,
/// joins the content proof to the entry custody namespace.
pub(super) fn lower_result_identity(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    checked_parameters: &[CheckedUnitStructuralParameterPlan],
    parameters: &[StructuralParameterDeclaration],
    result: &TerminalMachineResult,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<terminal_psi::ContentIdentityReshuffle>, LoweringError> {
    let facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut lowered = crate::content_conservation::lower_content_identity_reshuffles(&facts)?;
    let remap =
        |place: PlaceId| -> Result<PlaceId, LoweringError> {
            let declaration = lowered
                .structural_places
                .iter()
                .find(|declaration| declaration.id == place)
                .ok_or(LoweringError::Unsupported(
                    "content result has an absent structural root",
                ))?;
            match declaration.kind {
                StructuralPlaceKind::Parameter { position, is_self } => checked_parameters
                    .iter()
                    .zip(parameters)
                    .find(|(source, _)| source.position == position && source.is_self == is_self)
                    .map(|(_, parameter)| parameter.place)
                    .ok_or(LoweringError::Unsupported(
                        "content result lost its exact input parameter",
                    )),
                StructuralPlaceKind::Result => {
                    result.structural().map(|result| result.place).ok_or(
                        LoweringError::Unsupported("content identity has no structural result"),
                    )
                }
                _ => unsupported("content identity requires whole invocation roots"),
            }
        };
    for reshuffle in &mut lowered.reshuffles {
        let source = lowered
            .source_claims
            .iter()
            .find(|(_, claim)| *claim == reshuffle.claim)
            .ok_or(LoweringError::Unsupported(
                "content result lost its semantic claim",
            ))?
            .0;
        reshuffle.claim = lookup_claim_id(source_claims, source)?;
        reshuffle.input.root = remap(reshuffle.input.root)?;
        reshuffle.output.root = remap(reshuffle.output.root)?;
    }
    Ok(lowered.reshuffles)
}
