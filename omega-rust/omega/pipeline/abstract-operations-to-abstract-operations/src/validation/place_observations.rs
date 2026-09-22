//! Whether a component's place observations are loop-invariant: the
//! whole-component custody gate, the storage root an observation names,
//! preheader visibility of that root, the member place-parameter fixed point,
//! and the place, byte-read and subslice admissions built on them.

use super::invariant_operations::{
    admissible_invariant_byte_read, admissible_invariant_place_read, admissible_invariant_subslice,
};
use super::member_blocks::{
    member_scalar_operand_substitution, shared_entry_source, value_definition_sites,
};
use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

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
/// movement visible to a member observation: no store, record establishment,
/// atomic event, or custody-moving ownership event inside a member, no call
/// that could reach a caller place through a mutating structural argument or
/// a transferred claim unless every such argument's root is exclusively the
/// call's own, and no
/// affine discard on any component-adjacent edge. A `ByteSequenceSubslice`,
/// an `EstablishByteSequenceLiteral`, an `EstablishPrimitiveLocal`, an
/// `EstablishRecord`, and an
/// `EstablishScalarArray` are
/// the only establishments this bound tolerates: the subslice reads its
/// source root's extent without mutating the root, the literal reads
/// nothing at all, the primitive local declares a fresh claim-free
/// storage cell without mutating an existing place, a record
/// establishment binds its field initializers into a fresh root — its
/// structural field copies read an unrestricted source without moving it —
/// and a scalar-array establishment binds its scalar leaves into a fresh
/// root — without mutating an existing place either. Each establishes only a
/// fresh root, and a member-produced root can anchor a rebind only when the
/// relocation run covers its producer
/// ([`invariant_member_place_parameters`]), so
/// no member observation of an existing root changes across traversals. A
/// member call node always carries one `ClaimTransfer` ownership row — the
/// custody mirror of its `claim_transfers` roster — so the bound reads the
/// row rather than requiring an empty list: only a row actually moving a
/// claim refuses. A member call carrying mutable or write-only borrows is
/// tolerated only when every root those borrows name is exclusive to the
/// call ([`exclusive_borrow_call_preserves_place_observations`]): the callee
/// can write a caller place only through those borrows — verified call
/// bindings hand a mutating parameter exactly the argument root — so its
/// writes land in a place no other member reads, borrows, or moves, which
/// leaves every member observation of every other root loop-invariant. An
/// `Owned` argument is tolerated on the same terms when its root declares a
/// copyable shape — an unrestricted owned parameter or an unrestricted
/// claim-free scalar-array result: the argument copies the payload into the
/// callee, so nothing the caller still holds changes hands.
/// When
/// this holds, every member place observation is loop-invariant — no
/// traversal can change what it observes — so an admitted read relocates
/// without a per-root write analysis. That whole-component bound is
/// deliberately conservative: it refuses every place read in a component
/// containing any other place-writing node rather than resolving member
/// structural parameters to decide which roots a store could reach.
pub(crate) fn component_preserves_place_observations(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> bool {
    component_preserves_place_observations_tolerating(function, component, &BTreeSet::new())
}

/// [`component_preserves_place_observations`] plus `tolerated`: a
/// member-departing edge — internal or exit — may discard a place in
/// `tolerated`, the run's already-relocated confined affine results (and the
/// admitting call's own), whose per-traversal disposal custody the
/// relocation re-expresses through [`rewrite_scalar_case_custody`] rather
/// than preserves. Every tolerated place's producer relocated under
/// `scalar_case_result_contained`, so the fresh member place's discards are
/// exactly the custody the persistent preheader result's frontier
/// reconstructs — internal edges keep it live, exits and member returns
/// dispose it. A tolerated place that is not an affine confined result can
/// never appear in `trivial_affine_discards`, so the set is inert for the
/// unrestricted roots `relocating_roots` also carries. Entry edges still
/// refuse every discard — a member-produced place cannot appear there in a
/// verified seed — and residual discards refuse absolutely: a relocated
/// confined result is trivial-affine custody, never nominal cleanup.
pub(super) fn component_preserves_place_observations_tolerating(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    tolerated: &BTreeSet<PlaceId>,
) -> bool {
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for node in &block.nodes {
            // A member call's ownership roster is one claim-transfer row
            // spelling the same custody its `claim_transfers` carry: an empty
            // row moves nothing. Every other ownership event — completion,
            // cleanup, structural return, crash frontier — is custody
            // movement the bound refuses.
            let custody_quiet = node.ownership.iter().all(
                |event| matches!(event, OwnershipEvent::ClaimTransfer(claims) if claims.is_empty()),
            );
            if !custody_quiet
                || !(node_preserves_place_observations(&node.operation)
                    || exclusive_borrow_call_preserves_place_observations(
                        function, component, node,
                    ))
            {
                return false;
            }
        }
    }
    // Any discard adjacent to the component — on an internal edge, an entry
    // edge, or an exit — refuses the family, except a member-departing edge
    // may discard a tolerated place: the per-traversal disposal of a
    // confined result whose producer the run relocates. An entry-edge
    // discard runs once before the first iteration, but a place it ends
    // could not be read inside the loop at all, so the refusal is only
    // conservative.
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
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
        .flat_map(|block| {
            let member_departing = members.contains(&block.id);
            block.nodes.iter().flat_map(move |node| {
                node.successors
                    .iter()
                    .map(move |edge| (member_departing, edge))
            })
        })
        .filter(|(_, edge)| adjacent.contains(&edge.psi_edge))
        .all(|(member_departing, edge)| {
            edge.residual_affine_discards.is_empty()
                && edge
                    .trivial_affine_discards
                    .iter()
                    .all(|place| member_departing && tolerated.contains(place))
        })
}

/// The operation whitelist [`component_preserves_place_observations`] applies
/// to every member node. Pure scalar work and scalar constants name no place;
/// read-only place observations cannot change what they observe; a byte
/// subslice reads its source root's extent and establishes only a fresh view
/// root, a byte-sequence literal establishes only a fresh immutable view
/// root over constant bytes, a primitive-local establishment declares
/// only a fresh claim-free storage cell, a record establishment binds
/// its initializers into a fresh root — a structural field copies an
/// unrestricted source without moving it — a scalar-array establishment
/// binds its scalar leaves into a fresh root, and a scalar-case
/// establishment binds scalar fields into a fresh sum root, so no existing
/// place mutates,
/// and a
/// fresh member-produced root anchors another parameter's invariant
/// representative only when its producer relocates in the same run; control
/// nodes carry their custody on their successor edges, which the edge scan
/// checks; a port write touches a service port rather than a place; a plain
/// scalar `Call` has no place or claim surface at all; and a unit, scalar,
/// or structural call that moves no claims and passes only shared-borrow
/// structural arguments cannot mutate any place it could observe. Every
/// other variant —
/// stores, affine-local establishments, dynamic-dispatch
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
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishScalarCase { .. }
        | O::EstablishRecord { .. }
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
        // A structural-result call carries `returned_claim_transfers` beside
        // the outgoing roster: a pure callee returning claims would spell a
        // second ownership row, which `custody_quiet` already refuses — the
        // field check keeps the same refusal local to the operation.
        O::CallStructural {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => {
            claim_transfers.is_empty()
                && returned_claim_transfers.is_empty()
                && structural_arguments
                    .iter()
                    .all(|argument| argument.access == terminal_psi::StructuralAccess::SharedBorrow)
        }
        _ => false,
    }
}

/// The second-tier member tolerance [`component_preserves_place_observations`]
/// applies to a call node the strict whitelist refuses: a `CallUnit`,
/// `CallStructuralScalar`, or `CallStructural` that moves no claims, whose
/// borrow arguments —
/// `SharedBorrow`, `MutableBorrow`, or `WriteOnlyBorrow` — are confined as
/// below, and whose `Owned` arguments each name a whole root whose declared
/// custody is copyable ([`copyable_owned_argument_root`]): an `Unrestricted`
/// owned parameter or an unrestricted claim-free scalar-array result. An
/// owned whole-root argument over an unrestricted payload copies the
/// elements into the callee — the caller's place keeps its contents, so the
/// argument is an observation, not custody movement — while an `Owned`
/// argument over an affine or linear root moves the caller's place into the
/// callee outright and refuses, as does any produced kind the cyclic
/// owned-argument fence does not recognize. Mutating borrows each name a
/// root no other member can observe. The callee's caller-visible write
/// authority is exactly its
/// mutable and write-only parameter roots — verified call bindings cannot
/// hand it another place — so confining those roots to the one call leaves
/// the bound's guarantee intact: no member observation of any place another
/// member wrote changes across traversals, and no custody moves.
fn exclusive_borrow_call_preserves_place_observations(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
) -> bool {
    let (structural_arguments, claim_transfers) = match &node.operation {
        O::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => (structural_arguments, claim_transfers),
        // A structural-result call must also move no claims back: a
        // non-empty `returned_claim_transfers` would spell a second
        // ownership row the caller-side `custody_quiet` check refuses
        // anyway, so refusing here keeps the roster reasoning local.
        O::CallStructural {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } if returned_claim_transfers.is_empty() => (structural_arguments, claim_transfers),
        _ => return false,
    };
    if !claim_transfers.is_empty() {
        return false;
    }
    let mut borrowed = BTreeSet::new();
    for argument in structural_arguments {
        match argument.access {
            terminal_psi::StructuralAccess::SharedBorrow => {}
            terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                borrowed.insert(argument.place);
            }
            // An owned whole-root argument over a copyable unrestricted
            // root copies the payload into the callee — a read of the
            // root's contents, not custody movement. An owned argument
            // over an affine or linear root, a projected owned argument,
            // or a produced kind outside the cyclic owned-argument fence's
            // admitted shapes moves the caller's place into the callee
            // outright.
            terminal_psi::StructuralAccess::Owned => {
                if !argument.path.is_empty()
                    || !copyable_owned_argument_root(function, argument.place)
                {
                    return false;
                }
            }
        }
    }
    // A mutating root must appear on exactly one argument in the borrower's
    // own roster: a second spelling — at any access — would alias the cell
    // the callee writes through, and this bound does not lean on the source
    // verifier's anti-aliasing rules to rule that roster out.
    if structural_arguments
        .iter()
        .filter(|argument| borrowed.contains(&argument.place))
        .count()
        != borrowed.len()
    {
        return false;
    }
    borrowed_roots_exclusive(function, component, node, &borrowed)
}

/// Whether every place in `roots` is referenced inside `component`'s member
/// roster only by `borrower` itself and, for a member-produced root, by the
/// one node that declares it. "Referenced" covers every place field a member
/// node can spell — an observed `source`, a store `destination`, a call's
/// structural-argument root at any access, a case-inspection root — and every
/// member structural parameter an edge landing inside the roster binds to a
/// borrowed root (or to another tainted parameter): the binding carries the
/// root's contents into the member, so a member spelling that parameter
/// observes the borrowed root even though it never names it directly. A
/// member node's own produced root is not a reference — declaring a fresh
/// place observes nothing. Member-adjacent edge discards spelling a tainted
/// place count as references too, even though the edge-discard half of
/// [`component_preserves_place_observations`] already refuses them.
///
/// Exclusivity is what makes a member's mutating borrow custody-preserving:
/// the callee's writes through the borrow land in a place no member read,
/// second borrow, store, case inspection, or moved node could observe, so
/// the only member that can ever see the root's contents is the borrowing
/// call itself — which relocation then reasons about directly.
fn borrowed_roots_exclusive(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    borrower: &OptimizationNode,
    roots: &BTreeSet<PlaceId>,
) -> bool {
    if roots.is_empty() {
        return true;
    }
    let Some(PsiProvenance::Operation(borrower_operation)) = borrower.provenance.first().copied()
    else {
        return false;
    };
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    // Member structural parameters that can carry a borrowed root: a fixed
    // point over the bindings of every edge landing inside the roster — a
    // binding whose argument spells a tainted place (at any projection
    // depth) makes its parameter an alias member nodes can observe through.
    let mut tainted: BTreeSet<PlaceId> = roots.clone();
    loop {
        let mut progressed = false;
        for edge in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
            .filter(|edge| members.contains(&edge.target))
        {
            for binding in &edge.structural_bindings {
                if tainted.contains(&binding.argument.place) && tainted.insert(binding.parameter) {
                    progressed = true;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for node in &block.nodes {
            if node.provenance.first() == Some(&PsiProvenance::Operation(borrower_operation)) {
                continue;
            }
            let mut references = BTreeSet::new();
            if !member_place_references(&node.operation, &mut references) {
                return false;
            }
            if let Some(produced) = produced_place_root(&node.operation) {
                references.remove(&produced);
            }
            if references.iter().any(|place| tainted.contains(place)) {
                return false;
            }
            // A member terminator's own successor edges: a discard spelling a
            // tainted place is custody movement the exclusive-borrower
            // reading cannot allow, and a binding carrying one into a member
            // parameter is already accounted by the taint fixpoint.
            for edge in &node.successors {
                if edge
                    .trivial_affine_discards
                    .iter()
                    .any(|place| tainted.contains(place))
                    || edge
                        .residual_affine_discards
                        .iter()
                        .any(|discard| tainted.contains(&discard.place))
                {
                    return false;
                }
            }
        }
    }
    true
}

/// Collect every place `operation` can read, write, borrow, or move into
/// `references`, excluding the place it produces — the caller removes that
/// separately. Returns `false` for a variant whose place surface this scan
/// does not model, so the exclusivity check fails closed rather than
/// undercounting a member's references. Only place-free scalar work,
/// observations, stores, calls, and control variants are modeled; every
/// establishment's produced root is collected as well so the caller can
/// distinguish declaration from observation.
pub(super) fn member_place_references(operation: &O, references: &mut BTreeSet<PlaceId>) -> bool {
    match operation {
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::IntegerStructuralField { source, .. }
        | O::StructuralCase { source, .. } => {
            references.insert(*source);
        }
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. } => {
            references.insert(*destination);
        }
        O::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        } => {
            references.insert(*destination);
            references.insert(*source);
        }
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => {
            references.insert(destination.place);
        }
        // The three structural-signature call variants whose complete place
        // surface is `structural_arguments`: a dynamic-argument sibling's
        // descriptor arguments and a boundary call's completion claim
        // sources carry place surfaces this scan does not model, and a
        // `ReturnStructural` spells its returned place plus owned
        // declaration and discard rosters — all of those fail closed below.
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                references.insert(argument.place);
            }
        }
        // Establishments and scalar work: the establishment's declared place
        // is a production, not a reference; the caller removes it. A record
        // establishment's field initializers can embed a whole structural
        // argument — a borrowed or owned place moved into the record — so
        // each structural field value's root counts as a reference.
        // Constants,
        // scalar computations, plain scalar calls, port writes, descriptor
        // parameters, and non-structural terminators name no place at all.
        O::EstablishScalarArray { result, .. }
        | O::EstablishPrimitiveLocal { result, .. }
        | O::EstablishScalarCase { result, .. } => {
            references.insert(result.place);
        }
        O::EstablishRecord { result, fields, .. } => {
            references.insert(result.place);
            for field in fields {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                    references.insert(argument.place);
                }
            }
        }
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            references.insert(place.id);
        }
        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
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
        | O::Call { .. }
        | O::PortWrite { .. }
        | O::DynamicDescriptorParameter { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::Crash { .. } => {}
        // Atomic events, dynamic calls, descriptor stores, and any variant
        // not modeled above fail closed: the exclusivity scan refuses rather
        // than guesses which fields carry places.
        _ => return false,
    }
    true
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
/// place only another non-preheader block establishes stays invisible —
/// though [`invariant_observation_root`] still admits a member-produced root
/// the relocation run already covers, since its producer lands in the
/// preheader ahead of the run's consumers; a
/// member structural parameter is never itself visible but may rebind to a
/// visible or run-covered representative through
/// [`invariant_member_place_parameters`].
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
    produced_place_root(operation) == Some(root)
}

/// The place root `operation` establishes, when it establishes one: the
/// structural-result places, the declaration-carried literal and
/// affine-local places, and a single-attempt compare-exchange outcome. A
/// relocated producer keeps its root byte-exact inside the moved operation,
/// so the relocation run's produced-root roster is exactly this relation
/// read off the moved nodes — a member node whose argument or observed root
/// a run-covered producer declared can treat the root as landing in the
/// preheader with its producer.
pub(crate) fn produced_place_root(operation: &O) -> Option<PlaceId> {
    match operation {
        O::EstablishPrimitiveLocal { result, .. }
        | O::ByteSequenceSubslice { result, .. }
        | O::EstablishScalarArray { result, .. }
        | O::EstablishScalarCase { result, .. }
        | O::EstablishRecord { result, .. }
        | O::CallStructural { result, .. } => Some(result.place),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => Some(result.place),
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
        O::AtomicEvent {
            event: abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. },
            ..
        } => Some(outcome.place),
        _ => None,
    }
}

/// The place roots `operation` mutably borrows through its structural
/// arguments — the set the producer's move-together coupling and the
/// freeze's independent coverage replay both read. A mutable borrower can
/// read the cell it writes, so a relocated establishment would leave a
/// staying mutable borrower reading accumulated post-write contents where
/// the source traversal re-initialized the cell. Write-only borrows are not
/// collected: their callee cannot read the cell, so a staying write-only
/// borrower keeps writing deterministic contents no member observes.
pub(crate) fn mutable_borrow_roots(operation: &O) -> BTreeSet<PlaceId> {
    let structural_arguments = match operation {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return BTreeSet::new(),
    };
    structural_arguments
        .iter()
        .filter(|argument| argument.access == terminal_psi::StructuralAccess::MutableBorrow)
        .map(|argument| argument.place)
        .collect()
}

/// The number of member nodes that produce `root` inside `component`'s
/// roster. A verified unit declares a fresh place per establishment, so an
/// admitted relocation expects exactly one; a roster producing a mut-borrowed
/// or run-consumed root twice would let a staying producer re-initialize the
/// cell a relocated consumer reads once, so the call admission and the
/// exclusivity scan both refuse every multi-producer spelling.
pub(super) fn member_root_producer_count(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    root: PlaceId,
) -> usize {
    component
        .members
        .iter()
        .filter_map(|member| function.blocks.iter().find(|block| block.id == *member))
        .flat_map(|block| block.nodes.iter())
        .filter(|node| produced_place_root(&node.operation) == Some(root))
        .count()
}

/// Whether `root`'s declared custody makes an `Owned` whole-root argument a
/// copy rather than a custody move. The verifier binds an argument's declared
/// multiplicity equal to its callee parameter's, and the interpreter copies
/// an `Unrestricted` argument's payload into the callee's activation while
/// the caller keeps its place — where an affine or linear root spelled
/// `Owned` genuinely transfers custody. The admitted shapes are exactly the
/// ones the cyclic owned-argument fence recognizes, narrowed to the
/// copy-only multiplicities: a machine or block structural parameter
/// declared `!is_self`, `Owned`, `Unrestricted`, and qualification-free —
/// the fence's `plain_owned` shape without its `Affine` arm — and a
/// whole-root `EstablishScalarArray` result that is unrestricted,
/// claim-free, and qualification-free, produced by any block in the
/// function. Every other produced kind — and any root with restricted
/// custody — keeps the refusal: their `Owned` spellings genuinely transfer
/// custody or carry alias structure this boundary has not spelled out. Of
/// the three call families sharing this rule, only `CallStructuralScalar`
/// spells `Owned` arguments in a verified source unit today — attached
/// units confine call arguments to borrows, scalar-graph `CallUnit`
/// callees carry plain primitive parameters only, and a `CallStructural`
/// affine result confined to its own case terminator can never sit behind
/// an owned root — but the fence still answers for the other families:
/// the freeze replay independently re-derives admission on hand-moved
/// units, where no source grammar prunes the spellings first.
pub(super) fn copyable_owned_argument_root(
    function: &PsiOptimizationFunction,
    root: PlaceId,
) -> bool {
    function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.structural_parameters.iter()),
        )
        .any(|parameter| {
            parameter.place == root
                && !parameter.is_self
                && parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        || function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter())
            .any(|node| {
                matches!(
                    &node.operation,
                    O::EstablishScalarArray { result, .. }
                        if result.place == root
                            && result.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                )
            })
}

/// The complete invariant place-read admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned observation
/// shape ([`admissible_invariant_place_read`]), the component must perform no
/// place mutation or custody movement
/// ([`component_preserves_place_observations`]), and the root it observes must
/// land somewhere the relocated run can see it — visible at the unique
/// preheader insertion point ([`place_observation_root_visible`]), produced
/// by a node earlier in the same relocation run (`relocating_roots`), or
/// transitively when the observed root is a member structural parameter every
/// reaching edge binds to one such representative
/// ([`invariant_member_place_parameters`]). Returns the root the relocated
/// observation rebinds to: the node's own root when it is already
/// preheader-visible or run-covered, so a byte-exact move, a member-parameter
/// rebind, and a run-covered member-produced root share one admission.
pub(crate) fn invariant_place_observation_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<PlaceId> {
    let source = admissible_invariant_place_read(node)?;
    invariant_observation_root(function, component, source, relocating_roots)
}

/// The root an admitted observation rebinds to when it relocates: `source`
/// itself when it is already visible at the unique preheader insertion
/// point or produced by a node the same relocation run covers, so a
/// byte-exact move and a member-parameter rebind share one admission. The
/// whole-component place-custody gate
/// ([`component_preserves_place_observations`]) and the root's landing —
/// preheader visibility ([`place_observation_root_visible`]) or a uniquely
/// member-produced root the run already relocates — are enforced here so both
/// the proposal and the relocation freeze replay derive the same root from
/// the seed rather than trusting a plan. A run-covered root qualifies only
/// when its member producer is unique: a second producer would re-establish
/// the cell each traversal behind the relocated observation's single read.
pub(crate) fn invariant_observation_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    source: PlaceId,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<PlaceId> {
    let preheader_source = shared_entry_source(component)?;
    if !component_preserves_place_observations(function, component) {
        return None;
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    if place_observation_root_visible(function, preheader, source)
        || (relocating_roots.contains(&source)
            && member_root_producer_count(function, component, source) == 1)
    {
        return Some(source);
    }
    let representatives = invariant_member_place_parameters(function, component, relocating_roots);
    let representative = representatives.get(&source)?;
    (place_observation_root_visible(function, preheader, *representative)
        || (relocating_roots.contains(representative)
            && member_root_producer_count(function, component, *representative) == 1))
        .then_some(*representative)
}

/// The complete `ByteSequenceRead` admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned byte-read
/// shape ([`admissible_invariant_byte_read`]), its storage root must resolve
/// to a root the relocated run can see through the shared observation-root
/// admission ([`invariant_observation_root`]) — preheader-visible or produced
/// by a node the same run covers — and each scalar operand must satisfy the
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
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, length) = admissible_invariant_byte_read(node)?;
    let root = invariant_observation_root(function, component, source, relocating_roots)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length, relocating_roots)
        .then_some((root, substitution))
}

/// The complete `ByteSequenceSubslice` admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned subslice
/// shape ([`admissible_invariant_subslice`]), its storage root must resolve
/// to a root the relocated run can see through the shared observation-root
/// admission ([`invariant_observation_root`]) — preheader-visible or produced
/// by a node the same run covers — and each scalar operand — `start`,
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
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, _, length) = admissible_invariant_subslice(node)?;
    let root = invariant_observation_root(function, component, source, relocating_roots)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length, relocating_roots)
        .then_some((root, substitution))
}

/// Whether `rebound_length` — the substituted `length` operand an admitted
/// byte read or subslice relocates with — is defined by a `ByteSequenceLength`
/// measuring `root`, the rebound storage root the operation observes. A
/// member-internal producer qualifies only when its own observation root
/// resolves to that same root under the same run-covered member roots — the
/// operand substitution keeps it bound only when it relocates in the same
/// run; a producer outside the component must already measure the root
/// directly.
fn byte_length_operand_measures_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    root: PlaceId,
    rebound_length: ValueId,
    relocating_roots: &BTreeSet<PlaceId>,
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
            invariant_observation_root(function, component, *measured, relocating_roots)
                == Some(root)
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
/// representative — a root no member block establishes, or a member-produced
/// root the relocation run already covers. An observation reading through
/// such a parameter can be re-expressed on the representative root when that
/// root is visible at the preheader insertion point or lands there with its
/// run-covered producer.
///
/// A projected binding (`argument.path` nonempty) cannot anchor a root
/// rebind: the observation grammar names one root place, so a parameter
/// bound to `self.field` or to a subview stays loop-carried. A binding to a
/// root a member block establishes — an operation result, a byte-sequence
/// literal, or an affine local — is likewise loop-carried when the run does
/// not cover its producer: the traversal re-establishes that root every
/// iteration. A run-covered member-produced root instead anchors like any
/// outside root — the relocated producer lands in the preheader ahead of the
/// run's consumers, so the root persists across traversals and the parameter
/// spells it on every one. Verified edge bindings already
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
    relocating_roots: &BTreeSet<PlaceId>,
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
    // produces a fresh root, so a binding to one can anchor an invariant
    // representative only when the relocation run covers its producer — the
    // moved establishment lands in the preheader ahead of the run's
    // consumers, and the root then persists across traversals.
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
                } else if member_produced(argument.place)
                    && !relocating_roots.contains(&argument.place)
                {
                    // A member-produced root the run does not cover is
                    // re-established fresh every traversal — loop-carried.
                    // A covered root's producer lands in the preheader ahead
                    // of the run's consumers, so the root persists like any
                    // outside-established place and can anchor the
                    // representative.
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
