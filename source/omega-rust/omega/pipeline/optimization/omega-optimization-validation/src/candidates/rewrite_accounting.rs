//! Provenance and custody accounting reconstructed independently of producers.

use super::*;

pub(crate) fn preserve_edge_custody(
    node: &omega_optimization_unit::OptimizationNode,
) -> Vec<OptimizationEdge> {
    let expected = expected_edges(&node.operation);
    expected
        .into_iter()
        .map(|mut edge| {
            if let Some(existing) = node
                .successors
                .iter()
                .find(|existing| existing.psi_edge == edge.psi_edge)
            {
                edge.provenance = existing.provenance.clone();
                edge.fuel = existing.fuel.clone();
            }
            edge
        })
        .collect()
}

pub(crate) fn rewrite_scalar_substitutions(
    operation: &mut O,
    substitutions: &[ScalarSubstitution],
    machine: MachineId,
    removed_block: BlockId,
) {
    for substitution in substitutions {
        rewrite_block_parameter_operation(
            operation,
            RedundantBlockParameterRewrite {
                machine,
                block: removed_block,
                position: 0,
                parameter: substitution.from,
                replacement: substitution.to,
                scalar_type: substitution.scalar_type,
            },
        );
    }
}

pub(crate) fn reconstruct_adjacent_merge_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> bool {
    reconstruct_adjacent_merge_ownership_witness(unit, function, incoming, target).is_some()
}

pub(crate) fn reconstruct_adjacent_merge_ownership_witness(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    target: BlockId,
) -> Option<OwnershipFrontierWitness> {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return (function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty())
        .then_some(OwnershipFrontierWitness { rows: Vec::new() });
    }
    if !facts.iter().all(Option::is_some)
        || !facts
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

pub(crate) fn reconstruct_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: AdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut affected = BTreeSet::from([predecessor.id, target.id]);
    let first = target.nodes.first()?;
    let mut realized = if first.successors.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: patch.predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
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
    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(omega_optimization_unit::ProvenanceRewrite {
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
            realized.push(omega_optimization_unit::ProvenanceRewrite {
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
            realized.push(omega_optimization_unit::ProvenanceRewrite {
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

pub(crate) fn reconstruct_non_adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    patch: NonAdjacentBlockMergeRewrite,
    substitutions: &[ScalarSubstitution],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.target)?;
    if target_position == predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor = &function.blocks[predecessor_position];
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = &function.blocks[target_position];
    let first = target.nodes.first()?;
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let mut realized = if !first.provenance.is_empty() {
        vec![omega_optimization_unit::ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                patch.predecessor,
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| omega_optimization_unit::ProvenanceRewrite {
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

    for (node_index, node) in target.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target.id,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.id,
            node: patch
                .predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(omega_optimization_unit::ProvenanceRewrite {
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
        if block.id == patch.target {
            continue;
        }
        if input_starts.get(&block.id).copied()? != output_effect {
            effect_shifted.insert(block.id);
        }
        let output_nodes = if block.id == patch.predecessor.block {
            block
                .nodes
                .len()
                .checked_sub(1)?
                .checked_add(target.nodes.len())?
        } else {
            block.nodes.len()
        };
        output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
    }

    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([patch.predecessor.block, patch.target]);
    affected.extend(effect_shifted.iter().copied());
    for block in &function.blocks {
        if block.id == patch.target {
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
            realized.push(omega_optimization_unit::ProvenanceRewrite {
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

pub(crate) fn reconstruct_shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    patch: SharedJumpFusionRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor = function
        .blocks
        .iter()
        .find(|block| block.id == patch.predecessor.block)?;
    let predecessor_node = predecessor
        .nodes
        .get(usize::try_from(patch.predecessor.node).ok()?)?;
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == patch.incoming_edge)?;
    let target = function
        .blocks
        .iter()
        .find(|block| block.id == patch.target)?;
    let [terminal] = target.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: patch.incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: patch.target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(patch.predecessor);
    let mut provenance = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: input_edge,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(output_clone),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: input_terminal,
            disposition: ProvenanceDisposition::RealizedAt(input_terminal),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        },
    ];
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let mut blocks = vec![patch.predecessor.block, patch.target];
    blocks.sort();
    blocks.dedup();
    Some((blocks, provenance))
}

pub(crate) fn reconstruct_dead_scalar_node_accounting(
    function: &PsiOptimizationFunction,
    patch: DeadScalarNodeRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == patch.location.block)?;
    let node_position = usize::try_from(patch.location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let mut provenance = vec![omega_optimization_unit::ProvenanceRewrite {
        input: PsiRealizationSite::Node(patch.location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(patch.location)),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(omega_optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut blocks = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        blocks.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

pub(crate) fn reconstruct_proof_certified_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: ProofCertifiedScalarIdentityRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.location,
        source_operation: patch.source_operation,
        result: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.result)
        {
            continue;
        }
        blocks.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    blocks.dedup();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

pub(crate) fn reconstruct_local_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: LocalScalarCommonSubexpressionRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for use_block in &function.blocks {
        if blocks.contains(&use_block.id)
            || !use_block
                .nodes
                .iter()
                .flat_map(|node| &node.uses)
                .any(|row| row.value == patch.redundant_result)
        {
            continue;
        }
        blocks.push(use_block.id);
        for (index, node) in use_block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(omega_optimization_unit::ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    blocks.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

pub(crate) fn reconstruct_phi_translated_cse_accounting(
    function: &PsiOptimizationFunction,
    patch: &PhiTranslatedScalarGvnRewrite,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let dead = DeadScalarNodeRewrite {
        location: patch.redundant,
        source_operation: patch.redundant_operation,
        result: patch.redundant_result,
        scalar_type: patch.scalar_type,
    };
    let (mut blocks, mut provenance) = reconstruct_dead_scalar_node_accounting(function, dead)?;
    for incoming in &patch.incoming {
        let edge = function
            .blocks
            .iter()
            .find(|block| block.id == incoming.source)?
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .find(|edge| edge.psi_edge == incoming.edge && edge.target == patch.redundant.block)?;
        blocks.push(incoming.source);
        let site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: incoming.edge,
        };
        provenance.push(omega_optimization_unit::ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: edge.provenance.clone(),
            fuel: edge.fuel.clone(),
        });
    }
    blocks.sort();
    blocks.dedup();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((blocks, provenance))
}

pub(crate) fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
    let replace = |value: &mut ValueId| {
        if *value == from {
            *value = to;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings {
            replace(&mut binding.argument);
        }
    };
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump { bindings, .. } => rewrite_bindings(bindings),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            rewrite_bindings(&mut when_true.bindings);
            rewrite_bindings(&mut when_false.bindings);
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}

pub(crate) fn rewrite_successor_operation(
    operation: &mut O,
    edge: EdgeId,
    target: BlockId,
    bindings: &[omega_abstract_operations::ValueBinding],
) -> bool {
    match operation {
        O::Jump {
            psi_edge,
            target: operation_target,
            bindings: operation_bindings,
            ..
        } if *psi_edge == edge => {
            *operation_target = target;
            *operation_bindings = bindings.to_vec();
            true
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let successor = if when_true.psi_edge == edge {
                when_true
            } else if when_false.psi_edge == edge {
                when_false
            } else {
                return false;
            };
            successor.target = target;
            successor.bindings = bindings.to_vec();
            true
        }
        _ => false,
    }
}

pub(crate) fn reconstruct_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[omega_abstract_operations::ValueBinding],
    outgoing: &[omega_abstract_operations::ValueBinding],
) -> Option<Vec<omega_abstract_operations::ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        omega_abstract_operations::ValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

pub(crate) fn reconstruct_linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| {
        unit.ownership_frontier_facts
            .iter()
            .find(|fact| fact.machine == function.machine && fact.site == site)
    });
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(Option::is_some)
        && facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

pub(crate) fn reconstruct_linear_thread_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let predecessor_node = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        omega_optimization_unit::ProvenanceRewrite {
            input: predecessor_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: predecessor_edge.provenance.clone(),
            fuel: predecessor_edge.fuel.clone(),
        },
        omega_optimization_unit::ProvenanceRewrite {
            input: empty_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: empty_edge.provenance.clone(),
            fuel: empty_edge.fuel.clone(),
        },
    ];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != predecessor {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
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

pub(crate) fn reconstruct_path_thread_accounting(
    function: &PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[EdgeId],
) -> Option<(
    Vec<BlockId>,
    Vec<omega_optimization_unit::ProvenanceRewrite>,
)> {
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let outgoing = empty_node.successors.first()?;
    let outgoing_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: outgoing.psi_edge,
    };
    let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
    if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
        return None;
    }
    let mut affected = BTreeSet::from([empty.block]);
    let mut realized = Vec::new();
    for block in &function.blocks {
        for node in &block.nodes {
            for edge in &node.successors {
                if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                    continue;
                }
                affected.insert(block.id);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: edge.psi_edge,
                };
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
                realized.push(omega_optimization_unit::ProvenanceRewrite {
                    input: outgoing_site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: outgoing.provenance.clone(),
                    fuel: outgoing.fuel.clone(),
                });
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
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
