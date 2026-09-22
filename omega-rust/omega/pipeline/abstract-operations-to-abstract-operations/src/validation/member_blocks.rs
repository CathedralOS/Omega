//! Member-block structure of one cyclic component: scalar definition sites,
//! the shared preheader every entry departs, the guaranteed-executed member
//! blocks, the member-parameter fixed point over reaching edges, and the
//! operand substitution a relocated computation needs.

use super::invariant_operations::admissible_invariant_scalar_computation;
use optimization_unit::{
    OptimizationNode, OptimizerCycleComponent, PsiOptimizationFunction, ValueDefinitionSite,
};
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};

/// Every scalar value definition site in `function`: function parameters,
/// block parameters, and node results. Sites are the only authority needed to
/// decide whether a use is loop-carried — a definition inside a component's
/// member blocks can change each iteration, and nothing else can.
pub(crate) fn value_definition_sites(
    function: &PsiOptimizationFunction,
) -> BTreeMap<ValueId, ValueDefinitionSite> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| node.definitions.iter()))
        }))
        .map(|definition| (definition.value, definition.site))
        .collect()
}

/// The block every one of `component`'s entry edges departs — the unique
/// preheader a relocation can target. Entries may arrive on several edges of
/// that one block's terminator (a multi-arm dispatch where every arm enters
/// the cycle); entries departing different blocks leave the component with no
/// shared preheader, and an entry source inside the roster is not a preheader
/// at all, so both decline.
pub(crate) fn shared_entry_source(component: &OptimizerCycleComponent) -> Option<BlockId> {
    let [first, rest @ ..] = component.entries.as_slice() else {
        return None;
    };
    (rest.iter().all(|entry| entry.source == first.source)
        && !component.members.contains(&first.source))
    .then_some(first.source)
}

/// Member blocks of `component` guaranteed to execute on every traversal that
/// leaves the component: they dominate every exit-edge source inside the
/// subgraph the component's internal edges induce over its member roster,
/// rooted at the entry targets — a traversal enters through any one of them,
/// so a member qualifies only when every path from every entry target to
/// every exit source passes through it. Relocating a node out of any other
/// member block would speculate executions the source traversal may never
/// perform — a bypassing exit can leave the component before the block runs —
/// so the scalar-motion boundary admits only these members even though every
/// admitted operation is total. A component with no exits admits every
/// member: no traversal leaves it. The bound is the standard
/// guaranteed-to-execute criterion; a traversal that enters and neither exits
/// nor completes an iteration may still bypass the block, which is the
/// residual speculation the boundary accepts.
pub(crate) fn guaranteed_executed_member_blocks(
    component: &OptimizerCycleComponent,
) -> BTreeSet<BlockId> {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let entry_targets: BTreeSet<BlockId> = component
        .entries
        .iter()
        .map(|entry| entry.target)
        .filter(|target| members.contains(target))
        .collect();
    if entry_targets.is_empty() {
        return BTreeSet::new();
    }
    let boundary: BTreeSet<BlockId> = component
        .exits
        .iter()
        .map(|exit| exit.source)
        .filter(|source| members.contains(source))
        .collect();
    let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = members
        .iter()
        .map(|member| (*member, BTreeSet::new()))
        .collect();
    for edge in &component.id.internal_edges {
        if members.contains(&edge.source) && members.contains(&edge.target) {
            predecessors
                .get_mut(&edge.target)
                .expect("member target has a predecessor row")
                .insert(edge.source);
        }
    }
    // Every entry target is a dominator root: a traversal can reach it on an
    // entry edge without executing any other member. This is the usual
    // multi-root dominance — a virtual super-root over the entry targets —
    // with the super-root implicit since it is never itself a member.
    let mut dominators: BTreeMap<BlockId, BTreeSet<BlockId>> = members
        .iter()
        .map(|member| {
            (
                *member,
                if entry_targets.contains(member) {
                    BTreeSet::from([*member])
                } else {
                    members.clone()
                },
            )
        })
        .collect();
    loop {
        let mut changed = false;
        for member in members
            .iter()
            .copied()
            .filter(|member| !entry_targets.contains(member))
        {
            let mut incoming = predecessors[&member]
                .iter()
                .filter_map(|predecessor| dominators.get(predecessor));
            let mut next = incoming.next().cloned().unwrap_or_default();
            for set in incoming {
                next = next.intersection(set).copied().collect();
            }
            next.insert(member);
            if dominators[&member] != next {
                dominators.insert(member, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    members
        .iter()
        .copied()
        .filter(|member| {
            boundary
                .iter()
                .all(|source| dominators[source].contains(member))
        })
        .collect()
}

/// Parameters of `component`'s member blocks whose value is provably the same
/// on every iteration. A member parameter qualifies when every edge reaching
/// its block binds it to itself, to a member parameter that resolves to the
/// same representative, or to that representative — a value defined outside
/// the member roster. An operand use of such a parameter can be satisfied in
/// the preheader by substituting the representative.
///
/// The map is a fixed point over the edges reaching member blocks, so a
/// parameter carried across a member-to-member edge resolves to the
/// preheader-visible value its chain anchors on. A representative defined
/// inside the roster never qualifies: an edge that spells a member-internal
/// node result marks the parameter loop-carried. Parameters whose bindings
/// never anchor outside the roster — pure self-carried cycles — stay
/// unresolved and are absent from the result.
pub(crate) fn invariant_member_parameters(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> BTreeMap<ValueId, ValueId> {
    /// Resolution states during the fixed point. `Unresolved` may promote once
    /// its deferred dependencies resolve; `Representative` can still degrade
    /// to `LoopCarried` when a deferred dependency resolves to a conflicting
    /// or carried value, and `LoopCarried` is final.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Resolution {
        Unresolved,
        Representative(ValueId),
        LoopCarried,
    }

    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let mut parameters = Vec::new();
    for member in &component.members {
        if let Some(block) = function.blocks.iter().find(|block| block.id == *member) {
            parameters.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.value, block.id)),
            );
        }
    }
    let mut resolutions: BTreeMap<ValueId, Resolution> = parameters
        .iter()
        .map(|(parameter, _)| (*parameter, Resolution::Unresolved))
        .collect();
    loop {
        let mut progressed = false;
        for (parameter, block) in &parameters {
            if resolutions[parameter] == Resolution::LoopCarried {
                continue;
            }
            let mut anchor = None;
            let mut loop_carried = false;
            for edge in function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
                .filter(|edge| edge.target == *block)
            {
                let Some(binding) = edge
                    .bindings
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                else {
                    loop_carried = true;
                    break;
                };
                if binding.argument == *parameter {
                    // A binding that re-spells the parameter preserves whatever
                    // the other edges establish.
                    continue;
                }
                let contribution = match sites.get(&binding.argument) {
                    Some(ValueDefinitionSite::FunctionParameter(_)) => {
                        Resolution::Representative(binding.argument)
                    }
                    Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                        if members.contains(site) =>
                    {
                        // A member parameter contributes its own resolution;
                        // an unresolved dependency defers to a later pass.
                        resolutions
                            .get(&binding.argument)
                            .copied()
                            .unwrap_or(Resolution::LoopCarried)
                    }
                    Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                    | Some(ValueDefinitionSite::Node { block: site, .. })
                        if !members.contains(site) =>
                    {
                        Resolution::Representative(binding.argument)
                    }
                    _ => Resolution::LoopCarried,
                };
                match contribution {
                    Resolution::Unresolved => {}
                    Resolution::Representative(value) => match anchor {
                        None => anchor = Some(value),
                        Some(anchor) if anchor == value => {}
                        Some(_) => {
                            loop_carried = true;
                            break;
                        }
                    },
                    Resolution::LoopCarried => {
                        loop_carried = true;
                        break;
                    }
                }
            }
            let next = if loop_carried {
                Resolution::LoopCarried
            } else {
                match anchor {
                    Some(value) => Resolution::Representative(value),
                    None => Resolution::Unresolved,
                }
            };
            if resolutions[parameter] != next {
                resolutions.insert(*parameter, next);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    resolutions
        .into_iter()
        .filter_map(|(parameter, resolution)| match resolution {
            Resolution::Representative(representative) => Some((parameter, representative)),
            Resolution::Unresolved | Resolution::LoopCarried => None,
        })
        .collect()
}

/// The operand substitution a relocated invariant scalar computation needs, or
/// `None` when the node is not an admitted computation or one of its uses is
/// genuinely loop-carried. A use whose definition already sits outside the
/// member roster needs no rewrite; a use of an invariant member parameter is
/// rebound to the representative every reaching edge agrees on; a use whose
/// member-internal definition is the result of another node in the same
/// relocation run — `relocating` — stays bound to that value, since the run
/// preserves the producer's result identity and places it earlier in the
/// preheader; and a member parameter every reaching edge binds to such a
/// run-covered result substitutes to that result directly. Any other
/// member-internal definition rejects the relocation.
pub(crate) fn invariant_scalar_operand_substitution(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    if !admissible_invariant_scalar_computation(node) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// The use-site invariance rule every scalar-operand relocation shares: a use
/// defined outside the member roster needs no rewrite; a use of an invariant
/// member parameter is rebound to the representative every reaching edge
/// agrees on; a use of a member-internal node result stays bound only when
/// that producer relocates in the same run (`relocating`); and a use of a
/// member parameter every reaching edge binds to one such run-covered result
/// is rebound to that result directly — the parameter spells the preserved
/// producer result on every traversal, so the moved node may name it.
/// Every other member-internal definition refuses. The operation-shape gate
/// stays with
/// the callers — [`invariant_scalar_operand_substitution`] admits the pure
/// computation whitelist and [`invariant_byte_read_admission`] admits the
/// byte-read shape — while this walk is deliberately operation-agnostic.
pub(super) fn member_scalar_operand_substitution(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let representatives = invariant_member_parameters(function, component);
    let mut substitution = BTreeMap::new();
    for value_use in &node.uses {
        let site = sites.get(&value_use.value)?;
        let inside = match site {
            ValueDefinitionSite::FunctionParameter(_) => false,
            ValueDefinitionSite::BlockParameter { block, .. }
            | ValueDefinitionSite::Node { block, .. } => members.contains(block),
        };
        if !inside {
            continue;
        }
        match site {
            ValueDefinitionSite::BlockParameter { block, .. } => {
                if let Some(representative) = representatives.get(&value_use.value) {
                    substitution.insert(value_use.value, *representative);
                } else {
                    let result = member_parameter_run_result(
                        function,
                        &members,
                        &sites,
                        value_use.value,
                        *block,
                        relocating,
                    )?;
                    substitution.insert(value_use.value, result);
                }
            }
            ValueDefinitionSite::Node { .. } if relocating.contains(&value_use.value) => {}
            _ => return None,
        }
    }
    Some(substitution)
}

/// The run-covered result a member block parameter may substitute to, or
/// `None` when the parameter does not spell one. [`invariant_member_parameters`]
/// deliberately marks a parameter bound to a member-internal node result
/// loop-carried — its static map cannot know which results the relocation run
/// preserves — but at the use site the run is known: when every edge reaching
/// the parameter's block binds it to the same member-internal result whose
/// producer relocates in the same run, the parameter is that result on every
/// traversal and the moved node may name it directly. A self-respelling
/// binding contributes nothing; any other binding — a member parameter, an
/// outside value, or a member result the run does not cover — refuses.
fn member_parameter_run_result(
    function: &PsiOptimizationFunction,
    members: &BTreeSet<BlockId>,
    sites: &BTreeMap<ValueId, ValueDefinitionSite>,
    parameter: ValueId,
    parameter_block: BlockId,
    relocating: &BTreeSet<ValueId>,
) -> Option<ValueId> {
    let mut result = None;
    let mut bound = false;
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .filter(|edge| edge.target == parameter_block)
    {
        let binding = edge
            .bindings
            .iter()
            .find(|binding| binding.parameter == parameter)?;
        if binding.argument == parameter {
            continue;
        }
        let Some(ValueDefinitionSite::Node { block, .. }) = sites.get(&binding.argument) else {
            return None;
        };
        if !members.contains(block) || !relocating.contains(&binding.argument) {
            return None;
        }
        bound = true;
        match result {
            None => result = Some(binding.argument),
            Some(result) if result == binding.argument => {}
            Some(_) => return None,
        }
    }
    if bound { result } else { None }
}
