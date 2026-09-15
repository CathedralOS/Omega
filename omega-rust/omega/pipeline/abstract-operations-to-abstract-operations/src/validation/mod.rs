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

/// Place-observation leaf nodes are the first non-scalar family admitted for
/// loop-invariant motion out of a cyclic component: a fresh observation of an
/// established storage root. Every admitted variant is a verifier-approved
/// read that defines exactly one result, carries no scalar uses, successors,
/// or ownership events, and keeps its own operation identity as the first
/// provenance row. `ByteSequenceRead` stays outside the family this wave —
/// its scalar index/length operands and its bounds obligation add a second
/// evidence dimension the bounded gate does not yet reconstruct. Invariance
/// of the observed root and the root's preheader visibility are decided
/// separately by [`invariant_place_observation_admission`].
pub(crate) fn admissible_invariant_place_read(node: &OptimizationNode) -> Option<PlaceId> {
    let (psi_operation, source) = match &node.operation {
        O::PrimitiveScalarRead {
            psi_operation,
            source,
            ..
        }
        | O::StructuralCaseMembership {
            psi_operation,
            source,
            ..
        }
        | O::ByteSequenceLength {
            psi_operation,
            source,
            ..
        }
        | O::StructuralByteSequenceFieldLength {
            psi_operation,
            source,
            ..
        }
        | O::BooleanStructuralField {
            psi_operation,
            source,
            ..
        }
        | O::IntegerStructuralField {
            psi_operation,
            source,
            ..
        } => (*psi_operation, *source),
        _ => return None,
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && node.definitions.len() == 1
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty())
    .then_some(source)
}

/// Whether `component`'s member blocks perform no place mutation or custody
/// movement at all: no store, view or record establishment, atomic event, or
/// ownership event inside a member, no call that could reach a caller place
/// through a mutating structural argument or a transferred claim, and no
/// affine discard on any component-adjacent edge. When this holds, every
/// member place observation is loop-invariant — no traversal can change what
/// it observes — so an admitted read relocates without a per-root write
/// analysis. That whole-component bound is deliberately conservative: the
/// first non-scalar slice refuses every place read in a component containing
/// any place-writing node rather than resolving member structural parameters
/// to decide which roots a store could reach.
pub(crate) fn component_preserves_place_observations(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> bool {
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for node in &block.nodes {
            if !node.ownership.is_empty() || !node_preserves_place_observations(&node.operation) {
                return false;
            }
        }
    }
    // Any discard adjacent to the component — on an internal edge, the unique
    // entry edge, or an exit — refuses the family. An entry-edge discard runs
    // once before the first iteration, but a place it ends could not be read
    // inside the loop at all, so the refusal is only conservative.
    let adjacent: BTreeSet<EdgeId> = component
        .id
        .internal_edges
        .iter()
        .chain(component.entries.iter())
        .chain(component.exits.iter())
        .map(|edge| edge.edge)
        .collect();
    function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .filter(|edge| adjacent.contains(&edge.psi_edge))
        .all(|edge| {
            edge.trivial_affine_discards.is_empty() && edge.residual_affine_discards.is_empty()
        })
}

/// The operation whitelist [`component_preserves_place_observations`] applies
/// to every member node. Pure scalar work and scalar constants name no place;
/// read-only place observations cannot change what they observe; control
/// nodes carry their custody on their successor edges, which the edge scan
/// checks; a port write touches a service port rather than a place; a plain
/// scalar `Call` has no place or claim surface at all; and a unit or scalar
/// call that moves no claims and passes only shared-borrow structural
/// arguments cannot mutate any place it could observe. Every other variant —
/// stores, establishments, dynamic-dispatch and structural calls, boundary
/// calls, atomic events, descriptor stores — fails closed.
fn node_preserves_place_observations(operation: &O) -> bool {
    match operation {
        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::SaturatingIntegerMultiply { .. }
        | O::WrappingIntegerDivide { .. }
        | O::ExactIntegerDivide { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::ExactIntegerRemainder { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::IeeeFloatCompare { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::PortWrite { .. }
        | O::DynamicDescriptorParameter { .. }
        | O::Call { .. } => true,
        O::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => {
            claim_transfers.is_empty()
                && structural_arguments
                    .iter()
                    .all(|argument| argument.access == terminal_psi::StructuralAccess::SharedBorrow)
        }
        _ => false,
    }
}

/// Whether `root`, the storage root an admitted place observation names, is
/// visible at the component's unique-entry preheader insertion point: a
/// function structural parameter or result root, a provider-attachment root,
/// a structural parameter of the preheader block itself, or a place
/// established by a preheader node ahead of the terminator. The relocated run
/// inserts ahead of the countdown-certificate tail; those tail nodes are
/// scalar constants and never produce places, so scanning every
/// non-terminator preheader node here is exactly the proposal's
/// ahead-of-insertion computation. A root produced inside the component, a
/// member structural parameter, or a place only another block establishes
/// stays invisible — resolving member place parameters transitively like
/// [`invariant_member_parameters`] is the documented next slice.
pub(crate) fn place_observation_root_visible(
    function: &PsiOptimizationFunction,
    preheader: &OptimizationBlock,
    root: PlaceId,
) -> bool {
    function
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == root)
        || function
            .result
            .structural()
            .is_some_and(|result| result.place == root)
        || function.structural_places.iter().any(|declaration| {
            declaration.id == root
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::ProviderAttachment { .. }
                )
        })
        || preheader
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == root)
        || preheader
            .nodes
            .iter()
            .take(preheader.nodes.len().saturating_sub(1))
            .any(|node| produces_place_root(&node.operation, root))
}

/// Whether `operation` establishes `root`: the structural-result places, the
/// declaration-carried literal and affine-local places, and a single-attempt
/// compare-exchange outcome. This is the producer half of
/// [`place_observation_root_visible`].
fn produces_place_root(operation: &O, root: PlaceId) -> bool {
    match operation {
        O::EstablishPrimitiveLocal { result, .. }
        | O::ByteSequenceSubslice { result, .. }
        | O::EstablishScalarArray { result, .. }
        | O::EstablishScalarCase { result, .. }
        | O::EstablishRecord { result, .. }
        | O::CallStructural { result, .. } => result.place == root,
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => result.place == root,
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => place.id == root,
        O::AtomicEvent { event, .. } => matches!(
            event,
            abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. }
                if outcome.place == root
        ),
        _ => false,
    }
}

/// The complete invariant place-read admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned observation
/// shape ([`admissible_invariant_place_read`]), the component must perform no
/// place mutation or custody movement
/// ([`component_preserves_place_observations`]), and the root it observes must
/// be visible at the unique-entry preheader insertion point
/// ([`place_observation_root_visible`]). An admitted read carries no operand
/// rewrites at all — the root already names a preheader-visible place — so
/// the node relocates byte-exact like a scalar-constant leaf.
pub(crate) fn invariant_place_observation_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
) -> bool {
    let Some(source) = admissible_invariant_place_read(node) else {
        return false;
    };
    let [entry] = component.entries.as_slice() else {
        return false;
    };
    if component.members.contains(&entry.source) {
        return false;
    }
    if !component_preserves_place_observations(function, component) {
        return false;
    }
    let Some(preheader) = function
        .blocks
        .iter()
        .find(|block| block.id == entry.source)
    else {
        return false;
    };
    place_observation_root_visible(function, preheader, source)
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
