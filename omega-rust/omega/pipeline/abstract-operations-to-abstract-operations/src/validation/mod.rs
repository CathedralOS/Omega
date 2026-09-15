//! Optimizer module role: stage group. Abstract-optimization admission and publication replay.
//!
//! Independent unit and rewrite meaning lives in optimization-unit-semantics.
//! These checks additionally consume the preceding stage's sealed Terminal input.

use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use optimization_unit_semantics::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

mod context;
mod prephysical_manifest;
mod projection;

pub use context::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_psi_cycle_component_snapshot, validate_psi_ranking_certificate_snapshot,
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
    validate_verified_psi_cycle_components, validate_verified_psi_optimization_unit,
};

/// Scalar-constant leaf nodes are one operation class admitted for
/// loop-invariant motion out of a cyclic component. They read no values,
/// carry no control flow or ownership events, and keep their own operation
/// identity as the first provenance row, so an independent validator can track
/// the exact source node across the relocation.
pub(crate) fn admissible_scalar_leaf_relocation(node: &OptimizationNode) -> bool {
    let psi_operation = match &node.operation {
        O::IntegerConstant { psi_operation, .. }
        | O::IeeeFloatConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. } => *psi_operation,
        _ => return false,
    };
    node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

/// Side-effect-free total scalar computations are the second operation class
/// admitted for loop-invariant motion. Every admitted variant is pure scalar
/// (no place, claim, call, service, or control payload) and carries no
/// verifier obligation, so executing it once in the preheader cannot
/// introduce a crash or an observation the source node did not already own.
/// The node must still name its own operation as the first provenance row,
/// define exactly one result, and carry no successors or ownership events;
/// whether its operand uses are actually loop-invariant is decided per use
/// site by [`invariant_scalar_operand_substitution`].
pub(crate) fn admissible_invariant_scalar_computation(node: &OptimizationNode) -> bool {
    let psi_operation = match &node.operation {
        O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. }
        | O::IeeeFloatCompare { psi_operation, .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => *psi_operation,
        _ => return false,
    };
    node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && !node.uses.is_empty()
        && node.definitions.len() == 1
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

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

/// Member blocks of `component` guaranteed to execute on every traversal that
/// leaves the component: they dominate every exit-edge source inside the
/// subgraph the component's internal edges induce over its member roster,
/// rooted at the unique entry target. Relocating a node out of any other
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
    let [entry] = component.entries.as_slice() else {
        return BTreeSet::new();
    };
    if !members.contains(&entry.target) {
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
    let root = entry.target;
    let mut dominators: BTreeMap<BlockId, BTreeSet<BlockId>> = members
        .iter()
        .map(|member| {
            (
                *member,
                if *member == root {
                    BTreeSet::from([root])
                } else {
                    members.clone()
                },
            )
        })
        .collect();
    loop {
        let mut changed = false;
        for member in members.iter().copied().filter(|member| *member != root) {
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
/// rebound to the representative every reaching edge agrees on; and a use
/// whose member-internal definition is the result of another node in the same
/// relocation run — `relocating` — stays bound to that value, since the run
/// preserves the producer's result identity and places it earlier in the
/// preheader. Any other member-internal definition rejects the relocation.
pub(crate) fn invariant_scalar_operand_substitution(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    if !admissible_invariant_scalar_computation(node) {
        return None;
    }
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
            ValueDefinitionSite::BlockParameter { .. } => {
                substitution.insert(value_use.value, *representatives.get(&value_use.value)?);
            }
            ValueDefinitionSite::Node { .. } if relocating.contains(&value_use.value) => {}
            _ => return None,
        }
    }
    Some(substitution)
}

/// Rewrite the scalar operand fields of an admitted invariant computation.
/// Only the variants [`admissible_invariant_scalar_computation`] admits carry
/// plain `ValueId` operand positions; any other operation is left untouched.
pub(crate) fn substitute_invariant_scalar_operands(
    operation: &mut O,
    substitution: &BTreeMap<ValueId, ValueId>,
) {
    fn substitute(field: &mut ValueId, substitution: &BTreeMap<ValueId, ValueId>) {
        if let Some(representative) = substitution.get(field) {
            *field = *representative;
        }
    }
    match operation {
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. } => substitute(operand, substitution),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::IeeeFloatCompare { left, right, .. } => {
            substitute(left, substitution);
            substitute(right, substitution);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. } => {
            substitute(value, substitution);
            substitute(count, substitution);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => {
            substitute(left, substitution);
            substitute(right, substitution);
            substitute(addend, substitution);
        }
        _ => {}
    }
}
pub use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};
