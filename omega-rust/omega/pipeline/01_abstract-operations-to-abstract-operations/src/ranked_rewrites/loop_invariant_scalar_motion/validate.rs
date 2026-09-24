//! Optimizer module role: validation leaf. Independent plan replay and transformed custody proof.

use super::{
    LoopInvariantScalarMotionCandidate, LoopInvariantScalarMotionError,
    LoopInvariantScalarRelocation, MachineId, NodeLocation, OptimizationNode,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, ValidatedLoopInvariantScalarMotion, VerifiedPsiOptimizationSession, apply,
    candidate_identity, propose,
};
pub(super) fn candidate(
    session: &VerifiedPsiOptimizationSession,
    candidate: &LoopInvariantScalarMotionCandidate,
) -> Result<ValidatedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
    if candidate.input != session.unit().identity {
        return Err(LoopInvariantScalarMotionError::StaleCandidateRevision {
            candidate: candidate.input,
            current: session.unit().identity,
        });
    }
    let component = session
        .cycle_components()
        .components()
        .iter()
        .find(|component| component.id == candidate.component)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    let Some(plan) = propose::component_plan(session, component, &effects)? else {
        return Err(LoopInvariantScalarMotionError::AlreadyRelocated);
    };

    let output = apply::realize(
        session.unit(),
        component,
        &plan.nodes,
        plan.certificate_tail,
    )?;
    let relocations = plan
        .nodes
        .iter()
        .map(|node| {
            Ok(LoopInvariantScalarRelocation {
                node: node.clone(),
                destination: apply::operation_location(&output, node.psi_operation)
                    .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
            })
        })
        .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
    let expected_identity = candidate_identity(
        session.unit().identity,
        output.identity,
        &component.id,
        &relocations,
    );
    if candidate.identity != expected_identity
        || candidate.output != output.identity
        || candidate.component != component.id
        || candidate.relocations != relocations
    {
        return Err(LoopInvariantScalarMotionError::CandidateMismatch);
    }

    let reconstructed =
        VerifiedPsiOptimizationSession::from_transformed(session.input().clone(), output.clone())
            .map_err(LoopInvariantScalarMotionError::TransformedValidation)?;
    apply::reconstruct_custody(&reconstructed)?;
    let provenance = reconstruct_provenance(session.unit(), &output, candidate)?;
    if provenance.is_empty() {
        return Err(LoopInvariantScalarMotionError::AlreadyRelocated);
    }
    Ok(ValidatedLoopInvariantScalarMotion {
        candidate: candidate.clone(),
        output,
        provenance,
    })
}

fn reconstruct_provenance(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    candidate: &LoopInvariantScalarMotionCandidate,
) -> Result<Vec<ProvenanceRewrite>, LoopInvariantScalarMotionError> {
    let machine = candidate.component.machine;
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let mut rows = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(source) = node.provenance.first().copied() else {
                continue;
            };
            let input_location = NodeLocation {
                machine,
                block: block.id,
                node: u32::try_from(node_index)
                    .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?,
            };
            let (output_location, output_node) = unique_source(output, machine, source)
                .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
            if output_node.provenance != node.provenance || output_node.fuel != node.fuel {
                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
            }
            if input_location == output_location
                && node.effect == output_node.effect
                && node.definitions == output_node.definitions
                && node.uses == output_node.uses
            {
                continue;
            }
            let (sources, fuel) = candidate
                .relocations
                .iter()
                .find(|relocation| relocation.node.location == input_location)
                .map(|relocation| {
                    (
                        relocation.node.provenance.clone(),
                        relocation.node.fuel.clone(),
                    )
                })
                .unwrap_or_else(|| (node.provenance.clone(), node.fuel.clone()));
            if sources != node.provenance || fuel != node.fuel {
                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
            }
            rows.push(ProvenanceRewrite {
                input: PsiRealizationSite::Node(input_location),
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                    output_location,
                )),
                sources,
                fuel,
            });
        }
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

fn unique_source(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    source: PsiProvenance,
) -> Option<(NodeLocation, &OptimizationNode)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    let mut matches = function.blocks.iter().flat_map(|block| {
        block
            .nodes
            .iter()
            .enumerate()
            .filter_map(move |(node, value)| {
                (value.provenance.first() == Some(&source)).then_some((
                    NodeLocation {
                        machine,
                        block: block.id,
                        node: u32::try_from(node).ok()?,
                    },
                    value,
                ))
            })
    });
    let row = matches.next()?;
    matches.next().is_none().then_some(row)
}
