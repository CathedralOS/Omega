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
/// (no place, claim, call, service, or control payload) and either carries no
/// verifier obligation or carries one the checker already discharged against
/// its operand values — a range, divisor, or shift-count totality proof. The
/// discharge stays valid after relocation because operand substitution only
/// rebinds a member parameter to the representative every reaching edge
/// proves equal, or keeps a run-internal operand bound to the result identity
/// the same relocation run preserves; the obligation itself moves byte-exact
/// inside the operation. The non-speculative gate already guarantees the
/// moved node ran on every traversal, so executing it once in the preheader
/// cannot introduce a crash or an observation the source node did not already
/// own. The node must still name its own operation as the first provenance
/// row, define exactly one result, and carry no successors or ownership
/// events; whether its operand uses are actually loop-invariant is decided
/// per use site by [`invariant_scalar_operand_substitution`].
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
        | O::IntegerExactCast { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
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
/// provenance row. `ByteSequenceRead` is not a leaf here — its scalar
/// index/length operands and its bounds obligation add a second evidence
/// dimension — so it admits through [`admissible_invariant_byte_read`] and
/// [`invariant_byte_read_admission`] instead. Invariance of the observed root
/// and the root's preheader visibility are decided separately by
/// [`invariant_place_observation_admission`].
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

/// A `ByteSequenceRead` is the non-scalar family admitted after place
/// observations: still one verifier-approved observation of an established
/// storage root, but it additionally reads two scalar operands — the dynamic
/// `index` and the `length` a `ByteSequenceLength` on the same source defined —
/// and carries the bounds obligation the index proof produced. The node must
/// keep its own operation identity as the first provenance row, define exactly
/// one result, use exactly its `index` and `length` operands in operand order,
/// and carry no successors or ownership events. Root invariance and preheader
/// visibility are decided by the shared [`invariant_observation_root`]
/// resolution, operand invariance by [`invariant_byte_read_admission`]'s
/// substitution half.
pub(crate) fn admissible_invariant_byte_read(
    node: &OptimizationNode,
) -> Option<(PlaceId, ValueId, ValueId)> {
    let O::ByteSequenceRead {
        psi_operation,
        source,
        index,
        length,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.uses.len() == 2
        && node.uses[0].value == *index
        && node.uses[1].value == *length
        && node.successors.is_empty()
        && node.ownership.is_empty())
    .then_some((*source, *index, *length))
}

/// A `ByteSequenceSubslice` is the structural-producing member of the byte
/// observation family: still a verifier-approved read of an established
/// storage root's extent, but it additionally evaluates two scalar endpoints,
/// pairs them with the `length` a `ByteSequenceLength` on the same source
/// defined, and establishes a fresh view root under its bounds obligation.
/// The node must keep its own operation identity as the first provenance row,
/// define no scalar value — its result is the structural view the operation
/// spells — use exactly its `start`, `end`, and `length` operands in operand
/// order, and carry no successors or ownership events. A result carrying
/// qualifications or claims keeps evidence rows this boundary does not yet
/// re-express, so only an unqualified view relocates. Root invariance and
/// preheader visibility are decided by the shared [`invariant_observation_root`]
/// resolution, operand invariance by [`invariant_subslice_admission`]'s
/// substitution half.
pub(crate) fn admissible_invariant_subslice(
    node: &OptimizationNode,
) -> Option<(PlaceId, ValueId, ValueId, ValueId)> {
    let O::ByteSequenceSubslice {
        psi_operation,
        result,
        source,
        start,
        end,
        length,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == 3
        && node.uses[0].value == *start
        && node.uses[1].value == *end
        && node.uses[2].value == *length
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some((*source, *start, *end, *length))
}

/// The storage root an admitted place observation, byte read, or subslice
/// names — whichever observation gate the node's operation shape admits
/// through. `same_relocated_node` needs the expected root to replay the
/// member parameter's rebind without trusting the transformed unit's
/// spelling.
pub(crate) fn invariant_observation_source(node: &OptimizationNode) -> Option<PlaceId> {
    admissible_invariant_place_read(node)
        .or_else(|| admissible_invariant_byte_read(node).map(|(source, _, _)| source))
        .or_else(|| admissible_invariant_subslice(node).map(|(source, _, _, _)| source))
}

/// Whether `component`'s member blocks perform no place mutation or custody
/// movement at all: no store, record establishment, atomic event, or
/// ownership event inside a member, no call that could reach a caller place
/// through a mutating structural argument or a transferred claim, and no
/// affine discard on any component-adjacent edge. A `ByteSequenceSubslice` is
/// the one establishment this bound tolerates: it reads its source root's
/// extent without mutating the root and its result is a fresh view root —
/// [`invariant_member_place_parameters`] already refuses member-produced
/// roots as representatives, so the fresh view can never anchor a rebind and
/// no member observation of an existing root changes across traversals. When
/// this holds, every member place observation is loop-invariant — no
/// traversal can change what it observes — so an admitted read relocates
/// without a per-root write analysis. That whole-component bound is
/// deliberately conservative: the first non-scalar slice refuses every place
/// read in a component containing any place-writing node rather than
/// resolving member structural parameters to decide which roots a store
/// could reach.
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
    // Any discard adjacent to the component — on an internal edge, an entry
    // edge, or an exit — refuses the family. An entry-edge discard runs
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
/// read-only place observations cannot change what they observe; a byte
/// subslice reads its source root's extent and establishes only a fresh view
/// root — no existing place mutates, and a fresh member-produced root can
/// never anchor another parameter's invariant representative; control
/// nodes carry their custody on their successor edges, which the edge scan
/// checks; a port write touches a service port rather than a place; a plain
/// scalar `Call` has no place or claim surface at all; and a unit or scalar
/// call that moves no claims and passes only shared-borrow structural
/// arguments cannot mutate any place it could observe. Every other variant —
/// stores, record and local establishments, dynamic-dispatch and structural
/// calls, boundary calls, atomic events, descriptor stores — fails closed.
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
        | O::ByteSequenceSubslice { .. }
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
/// visible at the component's unique preheader insertion point: a
/// function structural parameter or result root, a provider-attachment root,
/// a structural parameter of the preheader block itself, or a place
/// established by a preheader node ahead of the terminator. The relocated run
/// inserts ahead of the countdown-certificate tail; those tail nodes are
/// scalar constants and never produce places, so scanning every
/// non-terminator preheader node here is exactly the proposal's
/// ahead-of-insertion computation. A root produced inside the component or a
/// place only another non-preheader block establishes stays invisible; a
/// member structural parameter is never itself visible but may rebind to a
/// visible representative through [`invariant_member_place_parameters`].
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
/// be visible at the unique preheader insertion point
/// ([`place_observation_root_visible`]) — either directly, or transitively
/// when the observed root is a member structural parameter every reaching
/// edge binds to the same preheader-visible representative
/// ([`invariant_member_place_parameters`]). Returns the root the relocated
/// observation rebinds to: the node's own root when it is already
/// preheader-visible, so a byte-exact move and a member-parameter rebind
/// share one admission.
pub(crate) fn invariant_place_observation_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
) -> Option<PlaceId> {
    let source = admissible_invariant_place_read(node)?;
    invariant_observation_root(function, component, source)
}

/// The root an admitted observation rebinds to when it relocates: `source`
/// itself when it is already visible at the unique preheader insertion
/// point, so a byte-exact move and a member-parameter rebind share one
/// admission. The whole-component place-custody gate
/// ([`component_preserves_place_observations`]) and the root's preheader
/// visibility ([`place_observation_root_visible`]) — direct or through the
/// member structural parameter's agreed representative
/// ([`invariant_member_place_parameters`]) — are enforced here so both the
/// proposal and the relocation freeze replay derive the same root from the
/// seed rather than trusting a plan.
pub(crate) fn invariant_observation_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    source: PlaceId,
) -> Option<PlaceId> {
    let preheader_source = shared_entry_source(component)?;
    if !component_preserves_place_observations(function, component) {
        return None;
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    if place_observation_root_visible(function, preheader, source) {
        return Some(source);
    }
    let representatives = invariant_member_place_parameters(function, component);
    let representative = representatives.get(&source)?;
    place_observation_root_visible(function, preheader, *representative).then_some(*representative)
}

/// The complete `ByteSequenceRead` admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned byte-read
/// shape ([`admissible_invariant_byte_read`]), its storage root must resolve
/// to a preheader-visible root through the shared observation-root admission
/// ([`invariant_observation_root`]), and each scalar operand must satisfy the
/// same use-site invariance rule an admitted scalar computation obeys —
/// defined outside the component, an invariant member parameter rebound to
/// its agreed representative, or the preserved result of a node earlier in
/// the same relocation run.
///
/// The `length` operand carries one additional coupling the generic rule
/// cannot express: byte-view validation requires it to be defined by a
/// `ByteSequenceLength` measuring the very root the read observes. A
/// member-internal producer qualifies only when it relocates in the same run
/// and its own root resolves to the read's rebound root; a producer outside
/// the component qualifies only when it already measures that root. Anything
/// else — a member parameter, a function parameter, or a length observation
/// of a different root — would re-express the read against a length the
/// transformed unit could not validate, so the admission refuses.
///
/// Returns the root the relocated read rebinds to plus the operand
/// substitution its member-parameter uses need.
pub(crate) fn invariant_byte_read_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, length) = admissible_invariant_byte_read(node)?;
    let root = invariant_observation_root(function, component, source)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length)
        .then_some((root, substitution))
}

/// The complete `ByteSequenceSubslice` admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned subslice
/// shape ([`admissible_invariant_subslice`]), its storage root must resolve
/// to a preheader-visible root through the shared observation-root admission
/// ([`invariant_observation_root`]), and each scalar operand — `start`,
/// `end`, and `length` — must satisfy the same use-site invariance rule an
/// admitted scalar computation obeys: defined outside the component, an
/// invariant member parameter rebound to its agreed representative, or the
/// preserved result of a node earlier in the same relocation run.
///
/// The `length` operand carries the same producer coupling a byte read obeys:
/// byte-view validation requires it to be defined by a `ByteSequenceLength`
/// measuring the very root the subslice observes, so the moved operation
/// still validates against its bounds obligation. The structural result —
/// the fresh view place with its type, multiplicity, and qualifications —
/// and the bounds obligation are not substitutable positions: the relocation
/// preserves them byte-exact rather than re-spelling them.
///
/// Returns the root the relocated subslice rebinds to plus the operand
/// substitution its member-parameter uses need.
pub(crate) fn invariant_subslice_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, _, length) = admissible_invariant_subslice(node)?;
    let root = invariant_observation_root(function, component, source)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length)
        .then_some((root, substitution))
}

/// Whether `rebound_length` — the substituted `length` operand an admitted
/// byte read or subslice relocates with — is defined by a `ByteSequenceLength`
/// measuring `root`, the rebound storage root the operation observes. A
/// member-internal producer qualifies only when its own observation root
/// resolves to that same root — the operand substitution keeps it bound only
/// when it relocates in the same run; a producer outside the component must
/// already measure the root directly.
fn byte_length_operand_measures_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    root: PlaceId,
    rebound_length: ValueId,
) -> bool {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let Some(ValueDefinitionSite::Node { block, node }) = sites.get(&rebound_length) else {
        return false;
    };
    let Some(producing) = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == *block)
        .and_then(|block| {
            usize::try_from(*node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
    else {
        return false;
    };
    match &producing.operation {
        O::ByteSequenceLength {
            source: measured, ..
        } if members.contains(block) => {
            invariant_observation_root(function, component, *measured) == Some(root)
        }
        O::ByteSequenceLength {
            source: measured, ..
        } => *measured == root,
        _ => false,
    }
}

/// Structural parameters of `component`'s member blocks whose root is
/// provably the same on every iteration — the place analog of
/// [`invariant_member_parameters`]. A member view parameter qualifies when
/// every edge reaching its block binds it to itself, to a member structural
/// parameter that resolves to the same representative, or to that
/// representative — a root no member block establishes. An observation
/// reading through such a parameter can be re-expressed on the
/// representative root when that root is visible at the preheader insertion
/// point.
///
/// A projected binding (`argument.path` nonempty) cannot anchor a root
/// rebind: the observation grammar names one root place, so a parameter
/// bound to `self.field` or to a subview stays loop-carried. A binding to a
/// root a member block establishes — an operation result, a byte-sequence
/// literal, or an affine local — is likewise loop-carried: the traversal
/// re-establishes that root every iteration. Verified edge bindings already
/// guarantee the argument's access is compatible with the parameter's, so
/// the resolved representative carries at least the access the member
/// observation used.
///
/// The map is a fixed point over the structural bindings on edges reaching
/// member blocks, so a parameter carried across a member-to-member edge
/// resolves to the root its chain anchors on. Parameters whose bindings
/// never anchor outside the roster — pure self-carried cycles — stay
/// unresolved and are absent from the result.
pub(crate) fn invariant_member_place_parameters(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> BTreeMap<PlaceId, PlaceId> {
    /// Resolution states during the fixed point. `Unresolved` may promote once
    /// its deferred dependencies resolve; `Representative` can still degrade
    /// to `LoopCarried` when a deferred dependency resolves to a conflicting
    /// or carried root, and `LoopCarried` is final.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Resolution {
        Unresolved,
        Representative(PlaceId),
        LoopCarried,
    }

    let member_blocks: Vec<&OptimizationBlock> = component
        .members
        .iter()
        .filter_map(|member| function.blocks.iter().find(|block| block.id == *member))
        .collect();
    let member_parameters: BTreeSet<PlaceId> = member_blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
        })
        .collect();
    // Whether `place` is a root a member block establishes: every iteration
    // produces a fresh root, so a binding to one can never anchor an
    // invariant representative.
    let member_produced = |place: PlaceId| {
        member_blocks.iter().any(|block| {
            block
                .nodes
                .iter()
                .any(|node| produces_place_root(&node.operation, place))
        })
    };
    let parameters: Vec<(PlaceId, BlockId)> = member_blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(|parameter| (parameter.place, block.id))
        })
        .collect();
    let mut resolutions: BTreeMap<PlaceId, Resolution> = parameters
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
                    .structural_bindings
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                else {
                    loop_carried = true;
                    break;
                };
                let argument = &binding.argument;
                if argument.place == *parameter && argument.path.is_empty() {
                    // A root binding that re-spells the parameter preserves
                    // whatever the other edges establish.
                    continue;
                }
                let contribution = if !argument.path.is_empty() {
                    // A projected argument names a sub-place, not a root the
                    // relocated observation could spell.
                    Resolution::LoopCarried
                } else if member_parameters.contains(&argument.place) {
                    // A member parameter contributes its own resolution; an
                    // unresolved dependency defers to a later pass.
                    resolutions
                        .get(&argument.place)
                        .copied()
                        .unwrap_or(Resolution::LoopCarried)
                } else if member_produced(argument.place) {
                    Resolution::LoopCarried
                } else {
                    Resolution::Representative(argument.place)
                };
                match contribution {
                    Resolution::Unresolved => {}
                    Resolution::Representative(place) => match anchor {
                        None => anchor = Some(place),
                        Some(anchor) if anchor == place => {}
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
                    Some(place) => Resolution::Representative(place),
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
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// The use-site invariance rule every scalar-operand relocation shares: a use
/// defined outside the member roster needs no rewrite; a use of an invariant
/// member parameter is rebound to the representative every reaching edge
/// agrees on; a use of a member-internal node result stays bound only when
/// that producer relocates in the same run (`relocating`); every other
/// member-internal definition refuses. The operation-shape gate stays with
/// the callers — [`invariant_scalar_operand_substitution`] admits the pure
/// computation whitelist and [`invariant_byte_read_admission`] admits the
/// byte-read shape — while this walk is deliberately operation-agnostic.
fn member_scalar_operand_substitution(
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
/// An obligated variant's `obligation` is not an operand position — it stays
/// byte-exact inside the moved operation.
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
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => substitute(operand, substitution),
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
        | O::WrappingIntegerDivide { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::IeeeFloatCompare { left, right, .. } => {
            substitute(left, substitution);
            substitute(right, substitution);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            substitute(value, substitution);
            substitute(count, substitution);
        }
        // A relocated byte read rebinds its `index` and `length` through the
        // same invariant-parameter substitution a pure computation uses. Its
        // `source` root and `obligation` are not scalar operand positions —
        // the root moves through `substitute_invariant_place_root` and the
        // obligation stays byte-exact.
        O::ByteSequenceRead { index, length, .. } => {
            substitute(index, substitution);
            substitute(length, substitution);
        }
        // A relocated subslice rebinds `start`, `end`, and `length` the same
        // way; its `source` root moves through `substitute_invariant_place_root`
        // while its structural result place and obligation stay byte-exact.
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => {
            substitute(start, substitution);
            substitute(end, substitution);
            substitute(length, substitution);
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

/// Rewrite the observed storage root of an admitted place observation, byte
/// read, or subslice from an invariant member parameter to its agreed
/// representative. Only the variants [`admissible_invariant_place_read`],
/// [`admissible_invariant_byte_read`], and [`admissible_invariant_subslice`]
/// admit carry a `source` root position; the rewrite fires only when the
/// operation's current root is `parameter`, so a drifted plan cannot rebind a
/// different place. Returns whether the root was rebound.
pub(crate) fn substitute_invariant_place_root(
    operation: &mut O,
    parameter: PlaceId,
    representative: PlaceId,
) -> bool {
    let source = match operation {
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::IntegerStructuralField { source, .. } => source,
        _ => return false,
    };
    if *source != parameter {
        return false;
    }
    *source = representative;
    true
}
pub use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};
