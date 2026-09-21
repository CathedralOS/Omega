//! Optimizer module role: validation leaf. Independent plan replay and exact custody reconstruction.
//!
//! Validation never trusts the candidate's membership rows: it re-admits
//! every declared row against the shared admission predicates — the place's
//! declaration/type/proof evidence and per-node admissibility proposal also
//! uses — without re-running the producer's own plan enumeration, requires
//! the rows to be sorted and distinct by site, rebuilds the output itself,
//! and binds the result through the candidate identity. The custody walk
//! then proves the transformed function differs only at the folded
//! observation sites — a forged or mismatched row changes the reconstructed
//! function and fails the comparison before the transformed unit is
//! re-validated.

use super::{
    CaseMembershipPlan, CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    ValidatedCaseMembershipSpecialization, VerifiedPsiOptimizationSession, admission, apply,
    candidate_identity,
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
    let Some(evidence) = admission::membership_evidence(unit, function, candidate.place) else {
        return Err(CaseMembershipSpecializationError::UnknownPlace);
    };
    if candidate.memberships.is_empty() {
        return Err(CaseMembershipSpecializationError::AlreadySpecialized);
    }
    if candidate.producer != evidence.root_basis.and_then(|(_, producer)| producer) {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    // The declared roster must be strictly ordered by site — that canonical
    // order is also what rejects a duplicated observation site.
    if candidate.memberships.windows(2).any(|pair| {
        (pair[0].site().block, pair[0].site().node) >= (pair[1].site().block, pair[1].site().node)
    }) {
        return Err(CaseMembershipSpecializationError::CandidateMismatch);
    }
    for declared in &candidate.memberships {
        if declared.site().machine != candidate.machine {
            return Err(CaseMembershipSpecializationError::CandidateMismatch);
        }
        let index = usize::try_from(declared.site().node)
            .map_err(|_| CaseMembershipSpecializationError::CandidateMismatch)?;
        let node = function
            .blocks
            .iter()
            .find(|block| block.id == declared.site().block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(CaseMembershipSpecializationError::CandidateMismatch)?;
        let admitted = admission::admit_membership_node(
            unit,
            &evidence,
            candidate.machine,
            declared.site().block,
            index,
            node,
        )
        .ok_or(CaseMembershipSpecializationError::CandidateMismatch)?;
        if admitted != *declared {
            return Err(CaseMembershipSpecializationError::CandidateMismatch);
        }
    }
    let plan = CaseMembershipPlan {
        machine: candidate.machine,
        place: candidate.place,
        producer: candidate.producer,
        memberships: candidate.memberships.clone(),
    };
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
    provenance_rows(input_function, plan)
}

/// The exact node custody a plan's folded observations carry: each folded
/// site retains the membership's own provenance and fuel settlement, realized
/// at the same node. Proposal and validation share this reconstruction so a
/// published candidate names exactly the custody the walk independently
/// derives.
pub(crate) fn provenance_rows(
    input_function: &optimization_unit::PsiOptimizationFunction,
    plan: &CaseMembershipPlan,
) -> Result<Vec<ProvenanceRewrite>, CaseMembershipSpecializationError> {
    let machine = plan.machine;
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
