//! Optimizer module role: proposal leaf. Component-custody-derived exact relocation candidates.

use super::*;

pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
    let mut candidates = Vec::new();
    for component in session.cycle_components().components() {
        if let Some(candidate) = component_candidate(session, component)? {
            candidates.push(candidate);
        }
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(LoopInvariantScalarMotionError::CandidateBudgetExhausted {
            required,
            limit: candidate_limit,
        });
    }
    Ok(candidates)
}

/// Plan the exact admissible-leaf relocation for one component, realize the
/// transformed unit, and bind the observed destinations into the candidate.
fn component_candidate(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Result<Option<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
    let Some(plan) = component_plan(session, component)? else {
        return Ok(None);
    };
    let output = apply::realize(
        session.unit(),
        component,
        &plan.leaves,
        plan.certificate_tail,
    )?;
    let relocations = plan
        .leaves
        .iter()
        .map(|leaf| {
            Ok(LoopInvariantScalarRelocation {
                destination: apply::operation_location(&output, leaf.psi_operation)
                    .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
                leaf: leaf.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
    let identity = candidate_identity(
        session.unit().identity,
        output.identity,
        &component.id,
        &relocations,
    );
    Ok(Some(LoopInvariantScalarMotionCandidate {
        identity,
        input: session.unit().identity,
        output: output.identity,
        component: component.id.clone(),
        relocations,
    }))
}

/// The independently replayable relocation plan for one component: every
/// admissible scalar-constant leaf still inside a member block, plus the
/// number of countdown-certificate constants already occupying the preheader
/// tail (the dedicated countdown boundary owns their role order).
pub(super) struct ComponentPlan {
    pub(super) leaves: Vec<LoopInvariantScalarLeaf>,
    pub(super) certificate_tail: usize,
}

pub(super) fn component_plan(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Result<Option<ComponentPlan>, LoopInvariantScalarMotionError> {
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let [entry] = component.entries.as_slice() else {
        return Ok(None);
    };
    if component.members.contains(&entry.source) {
        return Ok(None);
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == entry.source)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let Some(terminator_index) = preheader.nodes.len().checked_sub(1) else {
        return Ok(None);
    };
    if !preheader.nodes[terminator_index]
        .successors
        .iter()
        .any(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
    {
        return Ok(None);
    }
    let certificate_operations = certificate_operations(session, component);
    let certificate_tail = preheader.nodes[..terminator_index]
        .iter()
        .rev()
        .take_while(|node| {
            matches!(
                node.provenance.first(),
                Some(PsiProvenance::Operation(operation))
                    if certificate_operations.contains(operation)
            )
        })
        .count();
    let mut leaves = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
        for (index, node) in block.nodes.iter().enumerate() {
            if !crate::validation::admissible_scalar_leaf_relocation(node) {
                continue;
            }
            let psi_operation = match node.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => *operation,
                _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
            };
            if certificate_operations.contains(&psi_operation) {
                continue;
            }
            let [definition] = node.definitions.as_slice() else {
                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
            };
            leaves.push(LoopInvariantScalarLeaf {
                psi_operation,
                result: definition.value,
                scalar_type: definition.scalar_type,
                location: NodeLocation {
                    machine,
                    block: *member,
                    node: u32::try_from(index)
                        .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?,
                },
                provenance: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    if leaves.is_empty() {
        return Ok(None);
    }
    Ok(Some(ComponentPlan {
        leaves,
        certificate_tail,
    }))
}

/// Operations owned by the authenticated countdown relation for this
/// component. The dedicated countdown boundary retains their role-ordered
/// tail; the general boundary never takes custody of them.
fn certificate_operations(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> std::collections::BTreeSet<OperationId> {
    session
        .ranking_certificates()
        .certificates()
        .iter()
        .filter(|certificate| certificate.component == component.id)
        .flat_map(|certificate| {
            [
                certificate.guard.zero_operation,
                certificate.descent.one_operation,
            ]
        })
        .collect()
}
