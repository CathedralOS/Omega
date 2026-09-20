//! Optimizer module role: validation leaf. Independent plan replay and exact custody reconstruction.
//!
//! Validation never trusts the candidate's specialization rows: it re-admits
//! every declared row against the shared admission predicates — the dispatch
//! shape evidence and per-edge admissibility proposal also uses — without
//! re-running the producer's own plan enumeration, requires the rows to be a
//! sorted, distinct, strict subset of the incoming edges (fusing them all
//! would orphan the dispatch state), rebuilds the output itself, and binds
//! the result through the candidate identity. The custody walk then proves
//! the transformed function differs only at the fused predecessor sites and
//! that each fused edge carries the incoming edge's custody followed by the
//! resolved arm edge's — a forged or mismatched edge source changes the
//! reconstructed function and fails the comparison before the transformed
//! unit is re-validated.

use super::{
    AnalysisProduct, DispatchSpecializationPlan, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRealizationSite, StateArgumentSpecializationCandidate,
    StateArgumentSpecializationError, ValidatedStateArgumentSpecialization,
    VerifiedPsiOptimizationSession, admission, apply, candidate_identity, compute_analysis,
};
use optimization_core::AnalysisKind;
use semantic_vocabulary::MachineId;

pub(super) fn candidate(
    session: &VerifiedPsiOptimizationSession,
    candidate: &StateArgumentSpecializationCandidate,
) -> Result<ValidatedStateArgumentSpecialization, StateArgumentSpecializationError> {
    let unit = session.unit();
    if candidate.input != unit.identity {
        return Err(StateArgumentSpecializationError::StaleCandidateRevision {
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
        return Err(StateArgumentSpecializationError::UnknownDispatch);
    }
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine)
        .ok_or(StateArgumentSpecializationError::UnknownDispatch)?;
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        compute_analysis(unit, AnalysisKind::ScalarConstants)
    else {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    };
    let Some(evidence) =
        admission::dispatch_evidence(unit, function, candidate.dispatch, &constants)
    else {
        return Err(StateArgumentSpecializationError::UnknownDispatch);
    };
    if candidate.specializations.is_empty() {
        return Err(StateArgumentSpecializationError::AlreadySpecialized);
    }
    // The declared roster must be strictly ordered by supplying edge — that
    // canonical order is also what rejects a duplicated incoming edge.
    if candidate
        .specializations
        .windows(2)
        .any(|pair| pair[0].incoming_edge() >= pair[1].incoming_edge())
    {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    // Fusing every incoming edge would leave the dispatch state unreachable;
    // a declared set covering the complete incoming roster is refused.
    if candidate.specializations.len()
        >= admission::incoming_edges(function, candidate.dispatch).len()
    {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    for declared in &candidate.specializations {
        if declared.predecessor().machine != candidate.machine {
            return Err(StateArgumentSpecializationError::CandidateMismatch);
        }
        let index = usize::try_from(declared.predecessor().node)
            .map_err(|_| StateArgumentSpecializationError::CandidateMismatch)?;
        let owner_node = function
            .blocks
            .iter()
            .find(|block| block.id == declared.predecessor().block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
        let edge = owner_node
            .successors
            .iter()
            .find(|edge| {
                edge.psi_edge == declared.incoming_edge() && edge.target == candidate.dispatch
            })
            .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
        let admitted = admission::admit_incoming_edge(
            &evidence,
            declared.predecessor().block,
            index,
            owner_node,
            edge,
            &constants,
        )
        .ok_or(StateArgumentSpecializationError::CandidateMismatch)?;
        if admitted != *declared {
            return Err(StateArgumentSpecializationError::CandidateMismatch);
        }
    }
    let plan = DispatchSpecializationPlan {
        machine: candidate.machine,
        dispatch: candidate.dispatch,
        edges: candidate.specializations.clone(),
    };
    let output = apply::realize(unit, &plan)?;
    let expected_identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.dispatch,
        &plan.edges,
    );
    if candidate.identity != expected_identity || candidate.output != output.identity {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    VerifiedPsiOptimizationSession::from_transformed(session.input().clone(), output.clone())
        .map_err(StateArgumentSpecializationError::TransformedValidation)?;
    let provenance = reconstruct_provenance(unit, &output, &plan)?;
    if provenance.is_empty() {
        return Err(StateArgumentSpecializationError::AlreadySpecialized);
    }
    Ok(ValidatedStateArgumentSpecialization {
        candidate: candidate.clone(),
        output,
        provenance,
    })
}

/// Replays the admitted difference: the transformed machine's function must
/// equal the input function except at each specialization's predecessor site,
/// where the fused node is rebuilt independently. Each fused edge must carry
/// the incoming edge's provenance and fuel followed by the resolved arm
/// edge's. The ledger rows then record the incoming edge's own retained
/// occurrence and the resolved arm edge's fan-out — realized once inside the
/// fused edge and once at its surviving dispatch occurrence.
fn reconstruct_provenance(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    plan: &DispatchSpecializationPlan,
) -> Result<Vec<ProvenanceRewrite>, StateArgumentSpecializationError> {
    let machine = plan.machine;
    let input_function = input
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(StateArgumentSpecializationError::UnknownDispatch)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(StateArgumentSpecializationError::UnknownDispatch)?;
    let mut expected_function = input_function.clone();
    for row in &plan.edges {
        let index = usize::try_from(row.predecessor.node)
            .map_err(|_| StateArgumentSpecializationError::CoordinateOverflow)?;
        let Some(slot) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == row.predecessor.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(StateArgumentSpecializationError::MissingSite {
                machine,
                block: row.predecessor.block,
                node: row.predecessor.node,
            });
        };
        *slot = apply::fused_node(row, plan.dispatch, input_function)?;
    }
    if *output_function != expected_function {
        return Err(StateArgumentSpecializationError::CandidateMismatch);
    }
    provenance_rows(input_function, machine, plan)
}

/// The accepted custody ledger for one specialization plan: each fused edge
/// records the incoming edge's retained occurrence, the resolved arm edge's
/// fan-out onto the fused edge, and the resolved edge's surviving dispatch
/// occurrence. Shared between bespoke validation and the pass rule's candidate
/// construction so both publish identical provenance.
pub(crate) fn provenance_rows(
    input_function: &optimization_unit::PsiOptimizationFunction,
    machine: MachineId,
    plan: &DispatchSpecializationPlan,
) -> Result<Vec<ProvenanceRewrite>, StateArgumentSpecializationError> {
    let mut rows = Vec::new();
    for row in &plan.edges {
        let incoming = find_edge(input_function, row.incoming_edge)?;
        let resolved = find_edge(input_function, row.taken_edge)?;
        let incoming_site = PsiRealizationSite::Edge {
            machine,
            edge: row.incoming_edge,
        };
        let resolved_site = PsiRealizationSite::Edge {
            machine,
            edge: row.taken_edge,
        };
        rows.push(ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        });
        rows.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(incoming_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
        });
        rows.push(ProvenanceRewrite {
            input: resolved_site,
            disposition: ProvenanceDisposition::RealizedAt(resolved_site),
            sources: resolved.provenance.clone(),
            fuel: resolved.fuel.clone(),
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

fn find_edge(
    function: &optimization_unit::PsiOptimizationFunction,
    edge: semantic_vocabulary::EdgeId,
) -> Result<&optimization_unit::OptimizationEdge, StateArgumentSpecializationError> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .find(|candidate| candidate.psi_edge == edge)
        .ok_or(StateArgumentSpecializationError::CandidateMismatch)
}
