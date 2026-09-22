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
    let analysis = admission::function_analysis(function);
    for declared in &candidate.reads {
        if declared.site().machine != candidate.machine {
            return Err(FieldValueSpecializationError::CandidateMismatch);
        }
        let index = usize::try_from(declared.site().node)
            .map_err(|_| FieldValueSpecializationError::CandidateMismatch)?;
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == declared.site().block)
            .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
        let node = block
            .nodes
            .get(index)
            .ok_or(FieldValueSpecializationError::CandidateMismatch)?;
        let admitted =
            admission::admit_field_node(unit, &evidence, function, &analysis, block, index, node)
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
/// equal the input function transformed by exactly the plan's admitted rows —
/// constant folds in place, forwarded substitutions, retired observation
/// nodes, custody fusion, and restamped derived metadata. The ledger rows
/// then record the exact node custody the roster carries.
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
    apply::transform_function(input, &mut expected_function, plan)?;
    if *output_function != expected_function {
        return Err(FieldValueSpecializationError::CandidateMismatch);
    }
    let (_, provenance) = plan_accounting(input_function, plan)?;
    Ok(provenance)
}

/// The exact block region and node custody a plan's rows carry, computed from
/// the input function's coordinates. A `Constant` row folds its observation
/// in place: the read's own provenance and fuel stay realized at the same
/// node. A `Forward` row retires its observation node: the read's custody
/// lands at the node that inherits the vacated index — the next surviving
/// input node — and every later node in the block shifts down one coordinate
/// per earlier retirement. Because effects are a function-wide sequence,
/// every block at or after the earliest retired-node block is inside the
/// region, and every provenance-bearing node in a region block whose custody
/// moved — or whose own operation changed — gets one ledger row. Proposal and
/// validation share this reconstruction so a published candidate names
/// exactly the custody the walk independently derives.
pub(crate) fn plan_accounting(
    input_function: &optimization_unit::PsiOptimizationFunction,
    plan: &FieldValuePlan,
) -> Result<
    (Vec<semantic_vocabulary::BlockId>, Vec<ProvenanceRewrite>),
    FieldValueSpecializationError,
> {
    let machine = plan.machine;
    let mut removals: std::collections::BTreeMap<
        semantic_vocabulary::BlockId,
        std::collections::BTreeSet<u32>,
    > = std::collections::BTreeMap::new();
    let mut fold_sites: std::collections::BTreeSet<(semantic_vocabulary::BlockId, u32)> =
        std::collections::BTreeSet::new();
    let mut use_sites: std::collections::BTreeSet<(semantic_vocabulary::BlockId, u32)> =
        std::collections::BTreeSet::new();
    for row in &plan.reads {
        match row.resolution() {
            optimization_unit::FieldValueResolution::Constant(_) => {
                fold_sites.insert((row.site().block, row.site().node));
            }
            optimization_unit::FieldValueResolution::Forward(forwarded) => {
                removals
                    .entry(row.site().block)
                    .or_default()
                    .insert(row.site().node);
                for site in &forwarded.uses {
                    use_sites.insert((site.block, site.node));
                }
            }
        }
    }
    let earliest_removal = input_function
        .blocks
        .iter()
        .position(|block| removals.contains_key(&block.id));
    let mut affected = std::collections::BTreeSet::new();
    let mut provenance = Vec::new();
    for (block_position, block) in input_function.blocks.iter().enumerate() {
        let removal = removals.get(&block.id);
        let first_removed = removal.and_then(|set| set.iter().next().copied());
        let in_suffix = earliest_removal.is_some_and(|position| block_position > position);
        let mut block_affected = removal.is_some() || in_suffix;
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index)
                .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?;
            let coordinate = (block.id, node_index);
            if fold_sites.contains(&coordinate) || use_sites.contains(&coordinate) {
                block_affected = true;
            }
            let retired = removal.is_some_and(|set| set.contains(&node_index));
            let shifted = first_removed.is_some_and(|first| node_index > first);
            if !retired
                && !shifted
                && !in_suffix
                && !fold_sites.contains(&coordinate)
                && !use_sites.contains(&coordinate)
            {
                continue;
            }
            if !retired && node.provenance.is_empty() {
                continue;
            }
            let input_site = PsiRealizationSite::Node(super::NodeLocation {
                machine,
                block: block.id,
                node: node_index,
            });
            let output_index = node_index
                .checked_sub(
                    u32::try_from(
                        removal
                            .map(|set| set.iter().filter(|removed| **removed < node_index).count())
                            .unwrap_or(0),
                    )
                    .map_err(|_| FieldValueSpecializationError::CoordinateOverflow)?,
                )
                .ok_or(FieldValueSpecializationError::CoordinateOverflow)?;
            let output_site = PsiRealizationSite::Node(super::NodeLocation {
                machine,
                block: block.id,
                node: output_index,
            });
            provenance.push(ProvenanceRewrite {
                input: input_site,
                disposition: ProvenanceDisposition::RealizedAt(output_site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
        if block_affected {
            affected.insert(block.id);
        }
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok((affected.into_iter().collect(), provenance))
}
