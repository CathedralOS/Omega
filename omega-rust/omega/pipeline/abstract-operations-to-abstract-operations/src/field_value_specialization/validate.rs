//! Optimizer module role: validation leaf. Independent plan replay and exact custody reconstruction.
//!
//! Validation never trusts the candidate's field rows: it re-admits every
//! declared row against the shared admission predicates — the place's
//! declaration/type/producer evidence and per-node admissibility proposal
//! also uses — without re-running the producer's own plan enumeration,
//! requires the rows to be sorted and distinct by site, rebuilds the output
//! itself, and binds the result through the candidate identity. The custody
//! walk then proves the transformed function differs only at the folded
//! observation sites — a forged or mismatched row changes the reconstructed
//! function and fails the comparison before the transformed unit is
//! re-validated.

use super::{
    FieldValuePlan, FieldValueSpecializationCandidate, FieldValueSpecializationError,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    ValidatedFieldValueSpecialization, VerifiedPsiOptimizationSession, admission, apply,
    candidate_identity,
};

pub(super) fn candidate(
    session: &VerifiedPsiOptimizationSession,
    candidate: &FieldValueSpecializationCandidate,
) -> Result<ValidatedFieldValueSpecialization, FieldValueSpecializationError> {
    let unit = session.unit();
    if candidate.input != unit.identity {
        return Err(FieldValueSpecializationError::StaleCandidateRevision {
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
        return Err(FieldValueSpecializationError::UnknownPlace);
    }
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    let Some(evidence) = admission::field_evidence(function, candidate.place) else {
        return Err(FieldValueSpecializationError::UnknownPlace);
    };
    if candidate.reads.is_empty() {
        return Err(FieldValueSpecializationError::AlreadySpecialized);
    }
    if candidate.producer != evidence.root_producer.operation() {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    // The declared roster must be strictly ordered by site — that canonical
    // order is also what rejects a duplicated observation site.
    if candidate.reads.windows(2).any(|pair| {
        (pair[0].site().block, pair[0].site().node) >= (pair[1].site().block, pair[1].site().node)
    }) {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    for declared in &candidate.reads {
        if declared.site().machine != candidate.machine {
            return Err(FieldValueSpecializationError::CandidateMismatch);
        }
        let index = usize::try_from(declared.site().node)
            .map_err(|_| FieldValueSpecializationError::CandidateMismatch)?;
        let node = function
            .blocks
            .iter()
            .find(|block| block.id == declared.site().block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
        let admitted = admission::admit_field_node(
            unit,
            &evidence,
            function,
            declared.site().block,
            index,
            node,
        )
        .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
        if admitted != *declared {
            return Err(FieldValueSpecializationError::CandidateMismatch);
        }
    }
    let plan = FieldValuePlan {
        machine: candidate.machine,
        place: candidate.place,
        producer: candidate.producer,
        reads: candidate.reads.clone(),
    };
    let output = apply::realize(unit, &plan)?;
    let expected_identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.place,
        &plan.reads,
    );
    if candidate.identity != expected_identity || candidate.output != output.identity {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    VerifiedPsiOptimizationSession::from_transformed(session.input().clone(), output.clone())
        .map_err(FieldValueSpecializationError::TransformedValidation)?;
    let provenance = reconstruct_provenance(unit, &output, &plan)?;
    if provenance.is_empty() {
        return Err(FieldValueSpecializationError::AlreadySpecialized);
    }
    Ok(ValidatedFieldValueSpecialization {
        candidate: candidate.clone(),
        output,
        provenance,
    })
}

/// Replays the admitted difference: the transformed machine's function must
/// equal the input function except at each folded observation site, where the
/// replacement node is rebuilt independently. The ledger rows then record
/// each folded site's retained custody — the read's operation provenance and
/// fuel settlement realized at the same node.
fn reconstruct_provenance(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    plan: &FieldValuePlan,
) -> Result<Vec<ProvenanceRewrite>, FieldValueSpecializationError> {
    let machine = plan.machine;
    let input_function = input
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(FieldValueSpecializationError::UnknownPlace)?;
    let mut expected_function = input_function.clone();
    for row in &plan.reads {
        let index = usize::try_from(row.site.node)
            .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
        let Some(slot) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(FieldValueSpecializationError::MissingSite {
                machine,
                block: row.site.block,
                node: row.site.node,
            });
        };
        *slot = apply::folded_node(row, input_function, input)?;
    }
    apply::refresh_facts(&mut expected_function, plan)?;
    if *output_function != expected_function {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    provenance_rows(input_function, plan)
}

/// The exact node custody a plan's folded observations carry: each folded
/// site retains the read's own provenance and fuel settlement, realized at
/// the same node. Proposal and validation share this reconstruction so a
/// published candidate names exactly the custody the walk independently
/// derives.
pub(crate) fn provenance_rows(
    input_function: &optimization_unit::PsiOptimizationFunction,
    plan: &FieldValuePlan,
) -> Result<Vec<ProvenanceRewrite>, FieldValueSpecializationError> {
    let machine = plan.machine;
    let mut rows = Vec::new();
    for row in &plan.reads {
        let index = usize::try_from(row.site.node)
            .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
        let node = input_function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(FieldValueSpecializationError::MissingSite {
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
