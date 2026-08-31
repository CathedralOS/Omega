//! Exact checked state-entry claims and their machine-local Terminal identities.

use super::*;

pub(super) struct LoweredUnitClaims {
    pub(super) entry_claims: Vec<EntryClaim>,
    pub(super) source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
}

pub(super) fn lower_unit_entry_claims(
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
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
            source: psi_language_semantics::PermissionEventSource::StateEntry,
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
