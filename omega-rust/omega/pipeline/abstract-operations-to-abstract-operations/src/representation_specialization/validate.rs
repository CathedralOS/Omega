//! Optimizer module role: validation leaf. Independent plan replay and exact custody reconstruction.
//!
//! Validation never trusts the candidate's membership rows: it re-derives the
//! specialization plan from the place roster and the producer operation,
//! requires the claimed rows to equal the replayed rows exactly, rebuilds the
//! output itself, and binds the result through the candidate identity. The
//! custody walk then proves the transformed function differs only at the
//! folded observation sites — a forged or mismatched row changes the
//! reconstructed function and fails the comparison before the transformed
//! unit is re-validated.

use super::{
    CaseMembershipPlan, CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    ValidatedCaseMembershipSpecialization, VerifiedPsiOptimizationSession, apply,
    candidate_identity, propose,
};

pub(super) fn candidate(
    session: &VerifiedPsiOptimizationSession,
    candidate: &CaseMembershipSpecializationCandidate,
) -> Result<ValidatedCaseMembershipSpecialization, CaseMembershipSpecializationError> {
    let unit = session.unit();
    if candidate.input != unit.identity {
        return Err(CaseMembershipSpecializationError::StaleCandidateRevision {
            candidate: candidate.input,
            current: unit.identity,
        });
    }
    if session
        .cycle_components()
        .components()
        .iter()
        .any(|component| component.id.machine == candidate.machine)
    {
        return Err(CaseMembershipSpecializationError::UnknownPlace);
    }
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine)
        .ok_or(CaseMembershipSpecializationError::UnknownPlace)?;
    let Some(plan) = propose::plan(unit, function, candidate.place) else {
        return Err(CaseMembershipSpecializationError::UnknownPlace);
    };
    if plan.memberships.is_empty() {
        return Err(CaseMembershipSpecializationError::AlreadySpecialized);
    }
    if plan.machine != candidate.machine
        || plan.place != candidate.place
        || plan.producer != candidate.producer
        || plan.memberships != candidate.memberships
    {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    let output = apply::realize(unit, &plan)?;
    let expected_identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.place,
        &plan.memberships,
    );
    if candidate.identity != expected_identity || candidate.output != output.identity {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    VerifiedPsiOptimizationSession::from_transformed(session.input().clone(), output.clone())
        .map_err(CaseMembershipSpecializationError::TransformedValidation)?;
    let provenance = reconstruct_provenance(unit, &output, &plan)?;
    if provenance.is_empty() {
        return Err(CaseMembershipSpecializationError::AlreadySpecialized);
    }
    Ok(ValidatedCaseMembershipSpecialization {
        candidate: candidate.clone(),
        output,
        provenance,
    })
}

/// Replays the admitted difference: the transformed machine's function must
/// equal the input function except at each folded observation site, where the
/// replacement node is rebuilt independently. The ledger rows then record
/// each folded site's retained custody — the membership's operation
/// provenance and fuel settlement realized at the same node.
fn reconstruct_provenance(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    plan: &CaseMembershipPlan,
) -> Result<Vec<ProvenanceRewrite>, CaseMembershipSpecializationError> {
    let machine = plan.machine;
    let input_function = input
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(CaseMembershipSpecializationError::UnknownPlace)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(CaseMembershipSpecializationError::UnknownPlace)?;
    let mut expected_function = input_function.clone();
    for row in &plan.memberships {
        let index = usize::try_from(row.site.node)
            .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?;
        let Some(slot) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(CaseMembershipSpecializationError::MissingSite {
                machine,
                block: row.site.block,
                node: row.site.node,
            });
        };
        *slot = apply::folded_node(row, input_function, input)?;
    }
    apply::refresh_facts(&mut expected_function, plan)?;
    if *output_function != expected_function {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    let mut rows = Vec::new();
    for row in &plan.memberships {
        let index = usize::try_from(row.site.node)
            .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?;
        let node = input_function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(CaseMembershipSpecializationError::MissingSite {
                machine,
                block: row.site.block,
                node: row.site.node,
            })?;
        let site = PsiRealizationSite::Node(row.site);
        rows.push(ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok(rows)
}
