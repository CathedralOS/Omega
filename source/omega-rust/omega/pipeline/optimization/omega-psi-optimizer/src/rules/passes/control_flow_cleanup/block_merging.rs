//! Adjacent and non-adjacent block merging.

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct AdjacentBlockMergeRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct NonAdjacentBlockMergeRule;

impl AdjacentBlockMergeRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for AdjacentBlockMergeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for adjacent in function.blocks.windows(2) {
                let [predecessor, target] = adjacent else {
                    unreachable!("two-block window")
                };
                let eligible_first = target.nodes.first().is_some_and(|node| {
                    (node.successors.is_empty()
                        && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                            || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                                && matches!(
                                    node.operation,
                                    O::Return { .. }
                                        | O::ReturnUnit { .. }
                                        | O::ReturnStructural { .. }
                                        | O::Crash { .. }
                                ))))
                        || (matches!(node.operation, O::Conditional { .. })
                            && node.successors.len() == 2
                            && node.provenance.is_empty())
                });
                if target.id == function.entry || !eligible_first {
                    continue;
                }
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: jump_target,
                    bindings,
                    trivial_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || *jump_target != target.id
                    || function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .flat_map(|node| &node.successors)
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                {
                    continue;
                }
                let Some(ownership_witness) = adjacent_merge_ownership_witness(
                    unit,
                    function,
                    frontiers,
                    *incoming_edge,
                    target.id,
                ) else {
                    continue;
                };
                let Some(mut substitutions) = target
                    .parameters
                    .iter()
                    .zip(bindings)
                    .map(|(parameter, binding)| {
                        (binding.parameter == parameter.value
                            && binding.scalar_type == parameter.scalar_type
                            && replacement_dominates_parameter_uses(
                                function.machine,
                                binding.argument,
                                parameter.value,
                                machine_dominators,
                                use_definitions,
                            ))
                        .then_some(ScalarSubstitution {
                            from: parameter.value,
                            to: binding.argument,
                            scalar_type: parameter.scalar_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| target.parameters.len() == bindings.len())
                else {
                    continue;
                };
                substitutions.sort();
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        ownership_witness,
                        -2,
                        AdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl NonAdjacentBlockMergeRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for NonAdjacentBlockMergeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };

        let mut candidates = Vec::new();
        for function in &unit.functions {
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for (predecessor_position, predecessor) in function.blocks.iter().enumerate() {
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    psi_edge: incoming_edge,
                    target: target_id,
                    bindings,
                    trivial_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                let Some((target_position, target)) = function
                    .blocks
                    .iter()
                    .enumerate()
                    .find(|(_, block)| block.id == *target_id)
                else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || target.id == function.entry
                    || target_position == predecessor_position.saturating_add(1)
                    || !non_adjacent_merge_target_is_nonempty(target)
                    || !block_dominates(machine_dominators, predecessor.id, target.id)
                    || function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .flat_map(|node| &node.successors)
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                    || !adjacent_merge_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        target.id,
                    )
                {
                    continue;
                }
                let Some(mut substitutions) = target
                    .parameters
                    .iter()
                    .zip(bindings)
                    .map(|(parameter, binding)| {
                        (binding.parameter == parameter.value
                            && binding.scalar_type == parameter.scalar_type
                            && replacement_dominates_parameter_uses(
                                function.machine,
                                binding.argument,
                                parameter.value,
                                machine_dominators,
                                use_definitions,
                            ))
                        .then_some(ScalarSubstitution {
                            from: parameter.value,
                            to: binding.argument,
                            scalar_type: parameter.scalar_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| target.parameters.len() == bindings.len())
                else {
                    continue;
                };
                substitutions.sort();
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = non_adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_non_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        -2,
                        NonAdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

fn non_adjacent_merge_target_is_nonempty(
    target: &omega_optimization_unit::OptimizationBlock,
) -> bool {
    !target.nodes.is_empty()
        && !matches!(target.nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
}

pub(super) fn adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    target: BlockId,
) -> bool {
    adjacent_merge_ownership_witness(unit, function, frontiers, incoming, target).is_some()
}

fn adjacent_merge_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) || !facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
    {
        return None;
    }
    let mut rows = facts
        .into_iter()
        .map(|fact| {
            let fact = fact.expect("complete ownership frontier fact set");
            OwnershipFrontierWitnessRow {
                site: fact.site,
                fact: fact.identity,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|row| row.site);
    Some(OwnershipFrontierWitness { rows })
}

fn adjacent_merge_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_node = function.blocks[predecessor_position]
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, target]);
    let first = target_block.nodes.first()?;
    let mut realized = if !first.provenance.is_empty() {
        vec![ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.block,
                    node: predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    } else {
        return None;
    };
    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        for (node_index, node) in changed_nodes {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn non_adjacent_merge_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_block = &function.blocks[predecessor_position];
    let predecessor_node = predecessor_block
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let first = target_block.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut realized = if first.successors.is_empty() {
        vec![ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(predecessor)),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else {
        first
            .successors
            .iter()
            .map(|successor| ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    };

    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }

    let mut input_effect = 0u64;
    let mut input_starts = BTreeMap::new();
    for block in &function.blocks {
        input_starts.insert(block.id, input_effect);
        input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
    }
    let mut output_effect = 0u64;
    let mut effect_shifted = BTreeSet::new();
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target_block.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([predecessor.block, target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == target {
            continue;
        }
        let mut changed_uses = BTreeSet::new();
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .uses
                .iter()
                .any(|row| substituted_values.contains(&row.value))
            {
                changed_uses.insert(node_index);
                affected.insert(block.id);
            }
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty()
                || (!effect_shifted.contains(&block.id) && !changed_uses.contains(&node_index))
            {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}
