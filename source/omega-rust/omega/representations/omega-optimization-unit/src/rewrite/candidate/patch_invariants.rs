//! Patch-family shape, witness, substitution, and provenance invariants.

use std::collections::BTreeSet;

use super::super::*;

pub(super) fn validate(
    location: Option<NodeLocation>,
    affected_blocks: &[BlockId],
    substitutions: &[ScalarSubstitution],
    provenance: &[ProvenanceRewrite],
    witness: &PsiRewriteWitness,
    patch: &PsiRewritePatch,
) -> Result<(), PsiRewriteCandidateError> {
    match &patch {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            if provenance.iter().any(|row| {
                row.disposition
                    != ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                        location.unwrap(),
                    ))
                    || row.input != PsiRealizationSite::Node(location.unwrap())
            }) {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
            if provenance.is_empty()
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
            if substitutions
                != [ScalarSubstitution {
                    from: patch.parameter,
                    to: patch.replacement,
                    scalar_type: patch.scalar_type,
                }]
            {
                return Err(PsiRewriteCandidateError::BlockParameterSubstitutionMismatch);
            }
        }
        PsiRewritePatch::FoldConstantConditional(patch) => {
            let selected = PsiRealizationSite::Edge {
                machine: location.unwrap().machine,
                edge: patch.selected_edge,
            };
            let rejected = PsiRealizationSite::Edge {
                machine: location.unwrap().machine,
                edge: patch.rejected_edge,
            };
            if provenance
                .iter()
                .filter(|row| {
                    row.input == selected
                        && row.disposition == ProvenanceDisposition::RealizedAt(selected)
                })
                .count()
                != 1
                || provenance
                    .iter()
                    .filter(|row| {
                        row.input == rejected
                            && row.disposition
                                == ProvenanceDisposition::ProvenUnreachableAt(rejected)
                    })
                    .count()
                    != 1
                || !provenance.iter().any(|row| {
                    matches!(
                        row.disposition,
                        ProvenanceDisposition::ProvenUnreachableAt(_)
                    )
                })
                || !substitutions.is_empty()
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
            let incoming = PsiRealizationSite::Edge {
                machine: patch.predecessor.machine,
                edge: patch.incoming_edge,
            };
            let outgoing = PsiRealizationSite::Edge {
                machine: patch.predecessor.machine,
                edge: patch.outgoing_edge,
            };
            if patch.empty.node != 0
                || patch.empty.machine != patch.predecessor.machine
                || !affected_blocks.contains(&patch.empty.block)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.predecessor.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !provenance.iter().any(|row| {
                    row.input == incoming
                        && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                })
                || !provenance.iter().any(|row| {
                    row.input == outgoing
                        && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                })
                || !substitutions.is_empty()
                || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
            let outgoing = PsiRealizationSite::Edge {
                machine: patch.empty.machine,
                edge: patch.outgoing_edge,
            };
            let fanout = provenance
                .iter()
                .filter(|row| row.input == outgoing && row.disposition.is_realized())
                .count();
            if patch.empty.node != 0
                || !affected_blocks.contains(&patch.empty.block)
                || fanout == 0
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.empty.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !substitutions.is_empty()
                || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::MergeAdjacentBlock(patch) => {
            let incoming = PsiRealizationSite::Edge {
                machine: patch.predecessor.machine,
                edge: patch.incoming_edge,
            };
            if !affected_blocks.contains(&patch.target)
                || !provenance.iter().any(|row| row.input == incoming)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.predecessor.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(witness, PsiRewriteWitness::OwnershipFrontiers(_))
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
            let incoming = PsiRealizationSite::Edge {
                machine: patch.predecessor.machine,
                edge: patch.incoming_edge,
            };
            if !affected_blocks.contains(&patch.target)
                || !provenance.iter().any(|row| row.input == incoming)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.predecessor.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::FuseSharedTerminalJump(patch) => {
            let incoming = PsiRealizationSite::Edge {
                machine: patch.predecessor.machine,
                edge: patch.incoming_edge,
            };
            if !affected_blocks.contains(&patch.target)
                || !provenance.iter().any(|row| row.input == incoming)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.predecessor.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::RemoveDeadScalarNode(patch) => {
            let input = PsiRealizationSite::Node(patch.location);
            if !substitutions.is_empty()
                || !provenance.iter().any(|row| row.input == input)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.location.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(
                    witness,
                    PsiRewriteWitness::StructuralIdentity
                        | PsiRewriteWitness::AcceptedObligation(_)
                )
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
            let redundant_input = PsiRealizationSite::Node(patch.redundant);
            if patch.leader.machine != patch.redundant.machine
                || patch.leader.block != patch.redundant.block
                || patch.leader.node >= patch.redundant.node
                || patch.leader_operation == patch.redundant_operation
                || patch.leader_result == patch.redundant_result
                || substitutions
                    != [ScalarSubstitution {
                        from: patch.redundant_result,
                        to: patch.leader_result,
                        scalar_type: patch.scalar_type,
                    }]
                || !provenance.iter().any(|row| row.input == redundant_input)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.redundant.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(
                    witness,
                    PsiRewriteWitness::StructuralIdentity
                        | PsiRewriteWitness::AcceptedObligation(_)
                )
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
            let redundant_input = PsiRealizationSite::Node(patch.redundant);
            if patch.leader.machine != patch.redundant.machine
                || patch.leader.block == patch.redundant.block
                || patch.leader_operation == patch.redundant_operation
                || patch.leader_result == patch.redundant_result
                || substitutions
                    != [ScalarSubstitution {
                        from: patch.redundant_result,
                        to: patch.leader_result,
                        scalar_type: patch.scalar_type,
                    }]
                || !provenance.iter().any(|row| row.input == redundant_input)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.redundant.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(
                    witness,
                    PsiRewriteWitness::StructuralIdentity
                        | PsiRewriteWitness::AcceptedObligation(_)
                )
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
            let redundant_input = PsiRealizationSite::Node(patch.redundant);
            if !substitutions.is_empty()
                || patch.incoming.len() < 2
                || patch
                    .incoming
                    .windows(2)
                    .any(|pair| (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source))
                || patch.incoming.iter().any(|incoming| {
                    incoming.leader.machine != patch.redundant.machine
                        || incoming.leader_operation == patch.redundant_operation
                        || incoming.leader_result == patch.redundant_result
                        || !affected_blocks.contains(&incoming.source)
                })
                || !provenance.iter().any(|row| row.input == redundant_input)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.redundant.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(
                    witness,
                    PsiRewriteWitness::StructuralIdentity
                        | PsiRewriteWitness::AcceptedObligation(_)
                )
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
            let input = PsiRealizationSite::Node(patch.location);
            if substitutions
                != [ScalarSubstitution {
                    from: patch.result,
                    to: patch.replacement,
                    scalar_type: ScalarType::Integer(patch.scalar_type),
                }]
                || patch.result == patch.replacement
                || !provenance.iter().any(|row| row.input == input)
                || provenance.iter().any(|row| {
                    let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                        return true;
                    };
                    site.machine() != patch.location.machine
                        || site
                            .node()
                            .is_some_and(|location| !affected_blocks.contains(&location.block))
                })
                || !matches!(
                    witness,
                    PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
                )
            {
                return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
            }
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
            let pruned = patch
                .machines
                .iter()
                .map(|row| row.machine)
                .collect::<BTreeSet<_>>();
            if !affected_blocks.is_empty()
                    || !substitutions.is_empty()
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    || provenance.iter().any(|row| {
                        !pruned.contains(&row.input.machine())
                            || !matches!(row.disposition, ProvenanceDisposition::ProvenUnreachableAt(site) if site == row.input)
                    })
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
        }
    }
    Ok(())
}
