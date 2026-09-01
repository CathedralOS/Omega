//! Optimizer module role: proposal leaf. Placement-custody-derived exact relocation candidates.

use super::*;

pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<
    Vec<CountdownInvariantConstantRelocationCandidate>,
    CountdownInvariantConstantRelocationError,
> {
    let placements = session
        .countdown_invariant_constant_placement_analysis()
        .map_err(CountdownInvariantConstantRelocationError::Placement)?;
    let mut candidates = Vec::new();
    for placement in placements.loops() {
        let preheader = placement
            .placements
            .first()
            .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?
            .destination
            .before
            .block;
        if placement
            .placements
            .iter()
            .all(|row| row.constant.location.block == preheader)
        {
            continue;
        }
        candidates.push(from_placement(session.unit(), placement)?);
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| CountdownInvariantConstantRelocationError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(
            CountdownInvariantConstantRelocationError::CandidateBudgetExhausted {
                required,
                limit: candidate_limit,
            },
        );
    }
    Ok(candidates)
}

pub(super) fn from_placement(
    unit: &PsiOptimizationUnit,
    placement: &UnsignedCountdownInvariantConstantPlacements,
) -> Result<CountdownInvariantConstantRelocationCandidate, CountdownInvariantConstantRelocationError>
{
    let output = apply::realize(unit, placement)?;
    let mut relocations = placement
        .placements
        .iter()
        .map(|row| {
            Ok(CountdownInvariantConstantRelocation {
                destination: apply::operation_location(&output, row.constant.psi_operation)
                    .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?,
                constant: row.constant.clone(),
            })
        })
        .collect::<Result<Vec<_>, CountdownInvariantConstantRelocationError>>()?;
    relocations.sort_by_key(|row| match row.constant.role {
        CountdownInvariantConstantRole::PositiveGuardZero => 0,
        CountdownInvariantConstantRole::BackedgeDecrementOne => 1,
    });
    if relocations.len() != 2
        || relocations[0].constant.role != CountdownInvariantConstantRole::PositiveGuardZero
        || relocations[1].constant.role != CountdownInvariantConstantRole::BackedgeDecrementOne
    {
        return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
    }
    let identity = candidate_identity(
        unit.identity,
        output.identity,
        &placement.component,
        &relocations,
    );
    Ok(CountdownInvariantConstantRelocationCandidate {
        identity,
        input: unit.identity,
        output: output.identity,
        component: placement.component.clone(),
        relocations,
    })
}
