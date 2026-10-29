//! Optimizer module role: executable entrance. Control-flow cleanup: the
//! pass's exact rule order, one file per graph transformation beneath it --
//! constant-conditional folding, linear and path-qualified empty-block
//! threading (sharing the bypassable-empty-block walk and the ownership
//! predicate the replay checkers also reconstruct through), adjacent and
//! non-adjacent block merges (sharing the unique-predecessor merge walk,
//! `block_merge_substitutions` and `merge_boundary_ownership` with shared-jump
//! fusion), and unreachable private-machine pruning.

use crate::UseDefinitionAnalysis;
use crate::rules::catalog::BuiltInRuleRegistration;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{
    FuelSettlement, NodeLocation, OptimizationBlock, OptimizationNode, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiProvenance, PsiRealizationSite,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use std::collections::BTreeSet;

mod adjacent_block_merge;
mod block_merge_substitutions;
mod constant_conditional_fold;
mod linear_empty_block_thread;
mod merge_boundary_ownership;
mod non_adjacent_block_merge;
mod path_qualified_empty_block_thread;
mod shared_jump_fusion;
mod unreachable_private_machine_prune;

pub(crate) use adjacent_block_merge::AdjacentBlockMergeRule;
pub(crate) use constant_conditional_fold::ConstantConditionalFoldRule;
pub(crate) use linear_empty_block_thread::LinearEmptyBlockThreadRule;
pub(crate) use non_adjacent_block_merge::NonAdjacentBlockMergeRule;
pub(crate) use path_qualified_empty_block_thread::PathQualifiedEmptyBlockThreadRule;
pub(crate) use shared_jump_fusion::SharedJumpFusionRule;
pub(crate) use unreachable_private_machine_prune::UnreachablePrivateMachinePruneRule;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ConstantConditionalFoldRule),
        BuiltInRuleRegistration::new(1, LinearEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(2, PathQualifiedEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(3, AdjacentBlockMergeRule),
        BuiltInRuleRegistration::new(4, SharedJumpFusionRule),
        BuiltInRuleRegistration::new(5, UnreachablePrivateMachinePruneRule),
        BuiltInRuleRegistration::new(6, NonAdjacentBlockMergeRule),
    ]
}

/// The contract every structural control-flow cleanup rule states by hand:
/// this pass, the scalar roster those rewrites invalidate under structural
/// identity. Only the rule identity, version and required analyses differ.
fn control_flow_cleanup_rule_contract(
    identity: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        crate::rules::CONTROL_FLOW_CLEANUP_PASS_NAME,
        version,
        required_analyses,
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )
}

/// The two cleanup rules whose rewrite reaches past scalar dataflow -- the
/// conditional fold and the unreachable-machine prune -- publish the call
/// graph beside the scalar roster. `safety_class` stays the rule's own: the
/// prune re-expresses the same program, the fold replaces the operation.
fn call_graph_control_flow_cleanup_rule_contract(
    identity: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        crate::rules::CONTROL_FLOW_CLEANUP_PASS_NAME,
        version,
        required_analyses,
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::CallGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        safety_class,
    )
}

/// One realized-at rewrite shared by every cleanup rule's site stamp: the
/// disposition lands at `realized_at` while provenance and fuel move over from
/// the stamped source.
pub(super) fn provenance_realized_at(
    input: PsiRealizationSite,
    realized_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    fuel: &[FuelSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::RealizedAt(realized_at),
        sources: sources.to_vec(),
        fuel: fuel.to_vec(),
    }
}

/// The unreachable-site twin of `provenance_realized_at`: the disposition lands
/// at `unreachable_at` while provenance and fuel move over from the source.
pub(super) fn provenance_proven_unreachable_at(
    input: PsiRealizationSite,
    unreachable_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    fuel: &[FuelSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(unreachable_at),
        sources: sources.to_vec(),
        fuel: fuel.to_vec(),
    }
}

/// One node's provenance and fuel realized in place at its own site. Both of
/// the row's realization sites name the node's location, so a rule that leaves
/// a node in place and merely restamps it names only that location.
pub(super) fn node_realized_provenance(
    location: NodeLocation,
    node: &OptimizationNode,
) -> ProvenanceRewrite {
    let site = PsiRealizationSite::Node(location);
    provenance_realized_at(site, site, &node.provenance, &node.fuel)
}

/// Record the in-place site row of every provenance-bearing node `nodes`
/// selects, in block order. The node keeps its own location, so this is the
/// surviving-node stamp every removal and merge accounting epilogue shares;
/// a node whose position shifts keeps its own old-to-new row.
pub(super) fn record_node_provenance<'node>(
    machine: MachineId,
    block: &OptimizationBlock,
    nodes: impl Iterator<Item = (usize, &'node OptimizationNode)>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in nodes {
        if node.provenance.is_empty() {
            continue;
        }
        realized.push(node_realized_provenance(
            NodeLocation {
                machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            },
            node,
        ));
    }
    Some(())
}

/// Stamp each provenance-bearing node of the merged block at the position the
/// merge gives it: target node `i` lands at predecessor node `predecessor.node
/// + i`. Both merges restate their target's roster this way.
pub(super) fn record_merged_node_positions(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    target_block: &OptimizationBlock,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
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
        realized.push(provenance_realized_at(
            input,
            output,
            &node.provenance,
            &node.fuel,
        ));
    }
    Some(())
}

/// A scalar CFG rewrite applies only where no block declares descriptor
/// telescoping parameters: structural substitution is outside these rules.
pub(super) fn descriptor_telescope_free(function: &PsiOptimizationFunction) -> bool {
    function
        .blocks
        .iter()
        .all(|block| block.structural_parameters.is_empty())
}

/// The terminal jump a block-erasing rewrite starts from: the block's last node
/// must be a cleanup-free jump, and its own location is where the rewrite's
/// patch begins. Every merge, fusion and thread rule begins here, so the
/// requirement that an erased edge carries no cleanup action is stated once
/// instead of at each rewrite's eligibility gate.
pub(super) struct TerminalJump<'a> {
    pub(super) block: &'a OptimizationBlock,
    pub(super) location: NodeLocation,
    pub(super) edge: EdgeId,
    pub(super) target: BlockId,
    pub(super) bindings: &'a [ValueBinding],
}

pub(super) fn terminal_jump<'a>(
    function: &PsiOptimizationFunction,
    block: &'a OptimizationBlock,
) -> Option<TerminalJump<'a>> {
    let (index, node) = block.nodes.iter().enumerate().last()?;
    let O::Jump {
        psi_edge,
        target,
        bindings,
        cleanup_actions,
        ..
    } = &node.operation
    else {
        return None;
    };
    if !cleanup_actions.is_empty() {
        return None;
    }
    Some(TerminalJump {
        block,
        location: NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        },
        edge: *psi_edge,
        target: *target,
        bindings,
    })
}

/// One unique-predecessor merge candidate: the block holding a terminal jump
/// and the block that jump alone enters; `admit` states the roster relation the
/// adjacent and non-adjacent merges each require.
pub(super) struct MergeCandidate<'a> {
    pub(super) predecessor_position: usize,
    pub(super) jump: TerminalJump<'a>,
    pub(super) target_position: usize,
    pub(super) target: &'a OptimizationBlock,
}

pub(super) fn merge_candidates<'a>(
    function: &'a PsiOptimizationFunction,
    admit: impl Fn(&MergeCandidate<'a>) -> bool,
) -> Vec<MergeCandidate<'a>> {
    function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(predecessor_position, predecessor)| {
            let jump = terminal_jump(function, predecessor)?;
            let (target_position, target) = function
                .blocks
                .iter()
                .enumerate()
                .find(|(_, block)| block.id == jump.target)?;
            let unique_entry = target.id != function.entry
                && function
                    .block_edges()
                    .filter(|edge| edge.target == target.id)
                    .count()
                    == 1;
            let candidate = MergeCandidate {
                predecessor_position,
                jump,
                target_position,
                target,
            };
            (unique_entry && admit(&candidate)).then_some(candidate)
        })
        .collect()
}

/// One block's node scan against the running effect ledger: a node whose
/// declared input or output no longer matches the expected ordinal marks its
/// block affected and stamps realized provenance at the node site. Shared by
/// the cleanup rules' accounting epilogues; `excluded` is the one decision
/// site exempt from the drift check.
pub(super) fn record_block_effect_drift(
    function: &PsiOptimizationFunction,
    block: &OptimizationBlock,
    expected_effect: &mut u64,
    excluded: Option<NodeLocation>,
    affected: &mut BTreeSet<BlockId>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in block.nodes.iter().enumerate() {
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(node_index).ok()?,
        };
        let effect_changes = node.effect.input != *expected_effect
            || node.effect.output != expected_effect.checked_add(1)?;
        if effect_changes && excluded != Some(location) {
            affected.insert(block.id);
            if !node.provenance.is_empty() {
                realized.push(node_realized_provenance(location, node));
            }
        }
        *expected_effect = expected_effect.checked_add(1)?;
    }
    Some(())
}

/// Every block an empty-block thread can bypass: a non-entry block holding
/// exactly one node, whose cleanup-free terminal jump leaves no parameter
/// observed past the block's own entry. The incoming sites a thread admits, and
/// how their provenance is restamped, stay with each rule.
pub(super) fn threadable_empty_blocks<'a>(
    function: &'a PsiOptimizationFunction,
    use_definitions: &'a UseDefinitionAnalysis,
) -> impl Iterator<Item = TerminalJump<'a>> + 'a {
    function.blocks.iter().filter_map(|block| {
        if block.id == function.entry || block.nodes.len() != 1 {
            return None;
        }
        let jump = terminal_jump(function, block)?;
        let parameter_escapes = block.parameters.iter().any(|parameter| {
            use_definitions.uses.iter().any(|(machine, use_site)| {
                *machine == function.machine
                    && use_site.value == parameter.value
                    && (use_site.block != block.id || use_site.node != 0)
            })
        });
        (!parameter_escapes).then_some(jump)
    })
}

/// Close a thread's accounting: every other block's nodes are checked against
/// the running effect ledger -- `excluded` is the one decision site the rewrite
/// itself restates -- then the affected roster and provenance rows return in
/// canonical order. The linear thread excludes its retargeted predecessor.
pub(super) fn thread_accounting_tail(
    function: &PsiOptimizationFunction,
    empty: BlockId,
    excluded: Option<NodeLocation>,
    mut affected: BTreeSet<BlockId>,
    mut realized: Vec<ProvenanceRewrite>,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty {
            continue;
        }
        record_block_effect_drift(
            function,
            block,
            &mut expected_effect,
            excluded,
            &mut affected,
            &mut realized,
        )?;
    }
    realized.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
