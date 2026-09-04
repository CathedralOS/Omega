//! Optimizer module role: validation leaf. Independent placement replay and transformed custody proof.

use super::*;

pub(super) fn candidate(
    session: &VerifiedPsiOptimizationSession,
    candidate: &CountdownInvariantConstantRelocationCandidate,
) -> Result<ValidatedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationError>
{
    if candidate.input != session.unit().identity {
        return Err(
            CountdownInvariantConstantRelocationError::StaleCandidateRevision {
                candidate: candidate.input,
                current: session.unit().identity,
            },
        );
    }
    let placements = session
        .countdown_invariant_constant_placement_analysis()
        .map_err(CountdownInvariantConstantRelocationError::Placement)?;
    let placement = placements
        .loops()
        .iter()
        .find(|placement| placement.component == candidate.component)
        .ok_or(CountdownInvariantConstantRelocationError::UnknownComponent)?;
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
        return Err(CountdownInvariantConstantRelocationError::AlreadyRelocated);
    }

    let output = apply::realize(session.unit(), placement)?;
    let mut relocations = placement
        .placements
        .iter()
        .map(|row| {
            Ok(CountdownInvariantConstantRelocation {
                constant: row.constant.clone(),
                destination: apply::operation_location(&output, row.constant.psi_operation)
                    .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?,
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
    let expected_identity = candidate_identity(
        session.unit().identity,
        output.identity,
        &placement.component,
        &relocations,
    );
    if candidate.identity != expected_identity
        || candidate.output != output.identity
        || candidate.component != placement.component
        || candidate.relocations != relocations
    {
        return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
    }

    let reconstructed =
        VerifiedPsiOptimizationSession::from_transformed(session.input().clone(), output.clone())
            .map_err(CountdownInvariantConstantRelocationError::TransformedValidation)?;
    apply::reconstruct_custody(&reconstructed)?;
    let provenance = reconstruct_provenance(session.unit(), &output, candidate)?;
    if provenance.is_empty() {
        return Err(CountdownInvariantConstantRelocationError::AlreadyRelocated);
    }
    Ok(ValidatedCountdownInvariantConstantRelocation {
        candidate: candidate.clone(),
        output,
        provenance,
    })
}

fn reconstruct_provenance(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    candidate: &CountdownInvariantConstantRelocationCandidate,
) -> Result<Vec<ProvenanceRewrite>, CountdownInvariantConstantRelocationError> {
    let machine = candidate.component.machine;
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(CountdownInvariantConstantRelocationError::UnknownComponent)?;
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
                    .map_err(|_| CountdownInvariantConstantRelocationError::CoordinateOverflow)?,
            };
            let (output_location, output_node) = unique_source(output, machine, source)
                .ok_or(CountdownInvariantConstantRelocationError::CandidateMismatch)?;
            if output_node.provenance != node.provenance || output_node.fuel != node.fuel {
                return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
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
                .find(|relocation| relocation.constant.location == input_location)
                .map(|relocation| {
                    (
                        relocation.constant.provenance.clone(),
                        relocation.constant.fuel.clone(),
                    )
                })
                .unwrap_or_else(|| (node.provenance.clone(), node.fuel.clone()));
            if sources != node.provenance || fuel != node.fuel {
                return Err(CountdownInvariantConstantRelocationError::CandidateMismatch);
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
