//! Optimizer module role: proposal leaf. Component-custody-derived exact relocation candidates.

use super::{
    LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate, LoopInvariantScalarMotionError,
    LoopInvariantScalarNode, LoopInvariantScalarRelocation, NodeLocation, OperationId,
    PsiProvenance, ValueDefinitionSite, ValueId, VerifiedPsiOptimizationSession, apply,
    candidate_identity,
};
use abstract_operations::AbstractOperation;
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

/// Plan the exact admissible-node relocation for one component, realize the
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
        &plan.nodes,
        plan.certificate_tail,
    )?;
    let relocations = plan
        .nodes
        .iter()
        .map(|node| {
            Ok(LoopInvariantScalarRelocation {
                destination: apply::operation_location(&output, node.psi_operation)
                    .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
                node: node.clone(),
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
/// admissible scalar node still inside a member block — a scalar-constant
/// leaf, an invariant place observation (byte-exact, or with its storage root
/// rebound to the representative its member structural parameter resolves
/// to), a byte read or subslice (root rebound, scalar operands substituted,
/// `length` still coupled to a `ByteSequenceLength` on the rebound root, and
/// for a subslice the structural result preserved inside the moved
/// operation), an invariant scalar computation (an obligated variant keeps
/// its discharged obligation byte-exact inside the moved operation), or
/// a computation whose
/// member-internal operands are all defined by nodes earlier in the same run —
/// plus the number of countdown-certificate constants already occupying the
/// preheader tail (the dedicated countdown boundary owns their role order).
/// `nodes` is in run order: each node's member-internal producers were
/// admitted ahead of it, so the relocated sequence stays def-before-use.
pub(super) struct ComponentPlan {
    pub(super) nodes: Vec<LoopInvariantScalarNode>,
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
    // The component's entry edges may share one preheader block even when
    // several of them exist — a multi-arm dispatch whose every arm enters the
    // cycle. Entries departing different blocks leave no unique insertion
    // point, so the component declines.
    let Some(preheader_source) = crate::validation::shared_entry_source(component) else {
        return Ok(None);
    };
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let Some(terminator_index) = preheader.nodes.len().checked_sub(1) else {
        return Ok(None);
    };
    // Whether reaching the preheader guarantees entering the component: every
    // successor of its terminator must be a member — each is an entry edge by
    // construction. A relocation past a terminator with a non-member
    // successor would execute on traversals that never enter the loop, so the
    // gate declines non-leaf motion when this does not hold.
    let members: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let guaranteed_entry = !preheader.nodes[terminator_index].successors.is_empty()
        && preheader.nodes[terminator_index]
            .successors
            .iter()
            .all(|edge| members.contains(&edge.target));
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
    let insertion = terminator_index - certificate_tail;
    let sites = crate::validation::value_definition_sites(function);
    // Profitability gate: a computation moves only when reaching the
    // preheader guarantees entering the component, and only out of a member
    // block guaranteed to execute on every traversal that leaves it. A block
    // a bypassing exit can skip keeps its computations inside the loop —
    // moving them would speculate work the source traversal may never
    // perform, so the boundary declines them even though every admitted
    // operation is total. Scalar-constant leaves stay exempt: re-expressing a
    // constant in the preheader performs no work the traversal could have
    // skipped. Both halves of the gate are derived topology over the
    // authenticated component, so validation replays them exactly.
    let guaranteed = crate::validation::guaranteed_executed_member_blocks(component);
    // Invariant discovery is a fixed point: a computation whose
    // member-internal operand is defined by an already-planned relocation is
    // itself invariant — the run preserves the producer's result identity and
    // orders it first — so each pass admits exactly the nodes whose remaining
    // in-loop operands the run already covers. Scanning members in roster
    // order until no pass admits anything keeps the admitted set and the run
    // order canonical for independent replay.
    let mut nodes = Vec::new();
    let mut relocating = std::collections::BTreeSet::new();
    let mut admitted = std::collections::BTreeSet::new();
    // Every rebound operand must already be visible where the relocated run
    // lands: a function parameter, a preheader block parameter, or a preheader
    // node defined ahead of the run. Representatives defined by other
    // dominating blocks would need a dominance query this family does not run,
    // so they stay inside the loop.
    let representable = |substitution: &std::collections::BTreeMap<ValueId, ValueId>| {
        substitution
            .values()
            .all(|representative| match sites.get(representative) {
                Some(ValueDefinitionSite::FunctionParameter(_)) => true,
                Some(ValueDefinitionSite::BlockParameter { block, .. })
                    if *block == preheader_source =>
                {
                    true
                }
                Some(ValueDefinitionSite::Node {
                    block,
                    node: defined,
                }) if *block == preheader_source => {
                    usize::try_from(*defined).is_ok_and(|defined| defined < insertion)
                }
                _ => false,
            })
    };
    loop {
        let mut progressed = false;
        for member in &component.members {
            let block = function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
            for (index, node) in block.nodes.iter().enumerate() {
                let psi_operation = match node.provenance.first() {
                    Some(PsiProvenance::Operation(operation)) => *operation,
                    _ => continue,
                };
                if admitted.contains(&psi_operation)
                    || certificate_operations.contains(&psi_operation)
                {
                    continue;
                }
                let mut root_rewrite = None;
                let operand_rewrites = if crate::validation::admissible_scalar_leaf_relocation(node)
                {
                    Vec::new()
                } else if let Some(source) =
                    crate::validation::admissible_invariant_place_read(node)
                {
                    // An invariant place observation keeps both halves of the
                    // non-speculative gate — observing a root performs work a
                    // skipped traversal would not — and additionally needs the
                    // component to preserve place custody and its storage root
                    // to be visible at the preheader insertion point, either
                    // directly or as the representative its member structural
                    // parameter resolves to.
                    if !(guaranteed_entry && guaranteed.contains(member)) {
                        continue;
                    }
                    let Some(root) = crate::validation::invariant_place_observation_admission(
                        function, component, node,
                    ) else {
                        continue;
                    };
                    root_rewrite = (root != source).then_some((source, root));
                    Vec::new()
                } else if let Some((source, _, _)) =
                    crate::validation::admissible_invariant_byte_read(node)
                {
                    // A byte read keeps the same non-speculative gate as a
                    // place observation — the read performs work a bypassed
                    // traversal would not — then needs both evidence halves at
                    // once: its storage root resolves through the shared
                    // observation-root admission while its `index` and
                    // `length` operands obey the scalar substitution rule, and
                    // `length` must stay paired with a `ByteSequenceLength`
                    // measuring the rebound root so the moved operation still
                    // validates against its obligation.
                    if !(guaranteed_entry && guaranteed.contains(member)) {
                        continue;
                    }
                    let Some((root, substitution)) =
                        crate::validation::invariant_byte_read_admission(
                            function,
                            component,
                            node,
                            &relocating,
                        )
                    else {
                        continue;
                    };
                    if !representable(&substitution) {
                        continue;
                    }
                    root_rewrite = (root != source).then_some((source, root));
                    substitution.into_iter().collect()
                } else if let Some((source, _, _, _)) =
                    crate::validation::admissible_invariant_subslice(node)
                {
                    // A subslice keeps the byte family's whole evidence
                    // surface: the non-speculative gate, the observation-root
                    // resolution, the `start`/`end`/`length` substitution, and
                    // the `length` coupling to a `ByteSequenceLength` on the
                    // rebound root. Its structural result is not a definable
                    // operand — the moved operation preserves the fresh view
                    // place and the bounds obligation byte-exact.
                    if !(guaranteed_entry && guaranteed.contains(member)) {
                        continue;
                    }
                    let Some((root, substitution)) =
                        crate::validation::invariant_subslice_admission(
                            function,
                            component,
                            node,
                            &relocating,
                        )
                    else {
                        continue;
                    };
                    if !representable(&substitution) {
                        continue;
                    }
                    root_rewrite = (root != source).then_some((source, root));
                    substitution.into_iter().collect()
                } else {
                    if !(guaranteed_entry && guaranteed.contains(member)) {
                        continue;
                    }
                    let Some(substitution) =
                        crate::validation::invariant_scalar_operand_substitution(
                            function,
                            component,
                            node,
                            &relocating,
                        )
                    else {
                        continue;
                    };
                    if !representable(&substitution) {
                        continue;
                    }
                    substitution.into_iter().collect()
                };
                let result = match node.definitions.as_slice() {
                    [definition] => {
                        relocating.insert(definition.value);
                        LoopInvariantNodeResult::Scalar {
                            value: definition.value,
                            scalar_type: definition.scalar_type,
                        }
                    }
                    [] => match &node.operation {
                        // Only a shape-gated subslice reaches relocation
                        // without a scalar definition — any other zero- or
                        // multi-definition node cannot pass an admission gate,
                        // so reaching one here means the plan drifted.
                        AbstractOperation::ByteSequenceSubslice { result, .. }
                            if crate::validation::admissible_invariant_subslice(node).is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                    },
                    _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                };
                admitted.insert(psi_operation);
                nodes.push(LoopInvariantScalarNode {
                    psi_operation,
                    result,
                    location: NodeLocation {
                        machine,
                        block: *member,
                        node: u32::try_from(index)
                            .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?,
                    },
                    operand_rewrites,
                    root_rewrite,
                    provenance: node.provenance.clone(),
                    fuel: node.fuel.clone(),
                });
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    if nodes.is_empty() {
        return Ok(None);
    }
    Ok(Some(ComponentPlan {
        nodes,
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
