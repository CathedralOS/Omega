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

/// Parameters of `component`'s unique entry target whose value is provably the
/// same on every iteration: the entry edge binds the parameter to a
/// representative defined outside the member roster, and every other edge
/// reaching the target binds it either to itself or to that same
/// representative. An operand use of such a parameter can be satisfied in the
/// preheader by substituting the representative.
///
/// This is the deliberately bounded first invariant family: parameters of
/// non-entry member blocks resolve through internal edges and are not yet
/// traced transitively, and a representative defined inside the roster never
/// qualifies even when an edge spells it.
pub(crate) fn invariant_entry_target_parameters(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> BTreeMap<ValueId, ValueId> {
    let mut representatives = BTreeMap::new();
    let [entry] = component.entries.as_slice() else {
        return representatives;
    };
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let Some(entry_edge) = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .find(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
    else {
        return representatives;
    };
    let Some(header) = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
    else {
        return representatives;
    };
    for parameter in &header.parameters {
        let param = parameter.value;
        let Some(representative) = entry_edge
            .bindings
            .iter()
            .find(|binding| binding.parameter == param)
            .map(|binding| binding.argument)
        else {
            continue;
        };
        let outside = match sites.get(&representative) {
            Some(ValueDefinitionSite::FunctionParameter(_)) => true,
            Some(ValueDefinitionSite::BlockParameter { block, .. })
            | Some(ValueDefinitionSite::Node { block, .. }) => !members.contains(block),
            None => false,
        };
        if !outside {
            continue;
        }
        let invariant = function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
            .filter(|edge| edge.target == entry.target && edge.psi_edge != entry.edge)
            .all(|edge| {
                edge.bindings
                    .iter()
                    .find(|binding| binding.parameter == param)
                    .is_some_and(|binding| {
                        binding.argument == param || binding.argument == representative
                    })
            });
        if invariant {
            representatives.insert(param, representative);
        }
    }
    representatives
}

/// The operand substitution a relocated invariant scalar computation needs, or
/// `None` when the node is not an admitted computation or one of its uses is
/// genuinely loop-carried. A use whose definition already sits outside the
/// member roster needs no rewrite; a use of an invariant entry-target
/// parameter is rebound to that parameter's entry representative; and a use
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
    let [entry] = component.entries.as_slice() else {
        return None;
    };
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let representatives = invariant_entry_target_parameters(function, component);
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
            ValueDefinitionSite::BlockParameter { block, .. } if *block == entry.target => {
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
