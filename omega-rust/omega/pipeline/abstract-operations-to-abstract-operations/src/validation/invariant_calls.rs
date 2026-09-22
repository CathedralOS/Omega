//! Loop-invariance admission of member calls: unit, scalar, structural-scalar
//! and structural calls whose callee the whole-unit effect summary proves
//! pure and whose arguments land on preheader-visible or run-produced roots.
//! `unit_effect_summaries` is the effect product both the proposal and the
//! relocation freeze replay derive callee purity from.

use super::member_blocks::{member_scalar_operand_substitution, shared_entry_source};
use super::place_observations::{
    component_preserves_place_observations_tolerating, copyable_owned_argument_root,
    invariant_member_place_parameters, member_root_producer_count, place_observation_root_visible,
};
use super::relocation_rewrites::scalar_case_result_contained;
use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

/// Scalar-signature machine calls — `Call` — are the call family admitted for
/// loop-invariant motion: an exact internal callee invocation whose runtime
/// arguments are all scalars and whose single result is a scalar. The node
/// must keep its own operation identity as the first provenance row, define
/// exactly its spelled `result`/`scalar_type`, use exactly its `arguments` in
/// operand order, and carry no successors or ownership events. A call
/// carrying `crash_continuations` retains crash-route custody this family
/// does not yet re-express, so it stays inside; discharged
/// `requirement_obligations` instead move byte-exact inside the operation,
/// exactly like an obligated scalar computation's — they were proven against
/// the argument values, and operand substitution only rebinds a member
/// parameter to the representative every reaching edge proves equal. Callee
/// purity and member observability are decided separately by
/// [`invariant_scalar_call_admission`].
pub(crate) fn admissible_invariant_scalar_call(node: &OptimizationNode) -> Option<MachineId> {
    let O::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.definitions[0].value == *result
        && node.definitions[0].scalar_type == *scalar_type
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Unit-result machine calls — `CallUnit` — are the structural-signature
/// member of the call family admitted for loop-invariant motion: an exact
/// internal callee invocation whose scalar arguments ride the shared
/// substitution and whose structural arguments are borrows. The
/// node must keep its own operation identity as the first provenance row,
/// define no scalar (`CallUnit` produces no result — the relocated node
/// preserves the invocation itself), use exactly its scalar `arguments` in
/// operand order, carry no successors, and keep no crash-route custody. A
/// structural argument with `MutableBorrow` or `WriteOnlyBorrow` access hands
/// the callee write authority over a caller place — admitted only through
/// [`invariant_unit_call_admission`]'s exclusive member-produced-root
/// evidence — while `Owned` access admits only when the argument names a
/// whole root whose declared multiplicity is `Unrestricted`: the verifier
/// binds argument and callee parameter multiplicities equal, so the copy
/// shapes the cyclic owned-argument fence recognizes — an unrestricted
/// owned parameter or an unrestricted claim-free scalar-array result —
/// copy the payload into the callee's activation rather than moving
/// custody. An `Owned` argument over an affine or linear root genuinely
/// transfers the caller's place and stays refused.
/// `claim_transfers` must be empty: the node then carries exactly one
/// vacuous `ClaimTransfer` ownership row — the custody mirror of the empty
/// roster — which relocates byte-exact inside the moved operation.
/// `requirement_obligations` move byte-exact exactly like a scalar call's:
/// they were discharged against the argument values and operand substitution
/// only rebinds a member parameter to the representative every reaching edge
/// proves equal. Argument-root invariance and the whole-component
/// place-custody bound are decided separately by
/// [`invariant_unit_call_admission`].
pub(crate) fn admissible_invariant_unit_call(node: &OptimizationNode) -> Option<MachineId> {
    let O::CallUnit {
        psi_operation,
        callee,
        arguments,
        claim_transfers,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && claim_transfers.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())]
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Scalar-result structural-signature machine calls — `CallStructuralScalar`
/// — are the call family's third admitted member: an exact internal callee
/// invocation carrying both the scalar `arguments` a `Call` spells and the
/// shared-borrow `structural_arguments` a `CallUnit` spells, and defining
/// exactly one scalar result. The node must keep its own operation identity
/// as the first provenance row, define exactly its spelled `result`, use
/// exactly its scalar `arguments` in operand order, carry no successors, and
/// keep no crash-route custody. The structural side obeys the unit call's
/// whitelist verbatim: borrow arguments admit — a `MutableBorrow` or
/// `WriteOnlyBorrow` argument still needs
/// [`invariant_structural_scalar_call_admission`]'s exclusive
/// member-produced-root evidence, and an `Owned` argument admits only over
/// a whole root whose declared multiplicity is `Unrestricted`, the copy
/// shapes the cyclic owned-argument fence recognizes — while `claim_transfers` must
/// be empty, so the node
/// carries exactly one vacuous `ClaimTransfer` ownership row that relocates
/// byte-exact inside the moved operation. `requirement_obligations` move
/// byte-exact exactly like the other call variants': they were discharged
/// against the argument values and operand substitution only rebinds a
/// member parameter to the representative every reaching edge proves equal.
/// Callee purity, member observability, argument-root invariance, and the
/// whole-component place-custody bound are decided separately by
/// [`invariant_structural_scalar_call_admission`].
pub(crate) fn admissible_invariant_structural_scalar_call(
    node: &OptimizationNode,
) -> Option<MachineId> {
    let O::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        arguments,
        claim_transfers,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.definitions[0].value == result.value
        && node.definitions[0].scalar_type == result.scalar_type
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && claim_transfers.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())]
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Structural-result machine calls — `CallStructural` — are the call family's
/// fourth admitted member: an exact internal callee invocation returning a
/// fresh structural place. The admitted shapes are the ones the cyclic
/// eligibility fence already confines: an affine, claim-free result the
/// producing member block dispatches through a `StructuralCase` or returns
/// outright, so the verifier's per-traversal custody — produce inside the
/// member, discard on every dispatch edge — is exactly what the relocation
/// re-expresses; or an unrestricted claim-free result spelling one of the
/// frontier's plain-source shapes — a copy payload that never enters
/// `owned_places`, so the relocation re-expresses no custody for it at all
/// and the persistent preheader place simply reads the same value every
/// traversal. The node must keep its own operation identity as the first
/// provenance row, define no scalar, use exactly its scalar `arguments` in
/// operand order, carry no successors, and keep no crash-route custody.
/// `claim_transfers` and `returned_claim_transfers` must both be empty — the
/// node then carries exactly one vacuous `ClaimTransfer` ownership row, which
/// relocates byte-exact inside the moved operation. `structural_arguments`
/// obeys the argument whitelist the unit and scalar-result calls share: a
/// borrow argument lets the callee observe — and for a mutating borrow,
/// write — a caller place, so
/// [`invariant_structural_call_admission`] replays the whole-component
/// place-custody bound and each argument root's landing rule, while an
/// `Owned` argument admits only over a whole root whose declared
/// multiplicity is `Unrestricted` — an unrestricted owned parameter or an
/// unrestricted claim-free scalar-array result copies its payload into the
/// callee — and an `Owned` argument over an affine or linear root moves the
/// caller's place outright, custody movement this boundary cannot
/// re-express, so it stays refused.
/// `requirement_obligations`, `crash_continuations`, and
/// `selected_evidence` must be empty: the admitted contract carries
/// none of them, and a call that does stays inside rather than re-expressing
/// evidence this family has not reconstructed. Callee purity, member
/// observability, the place-custody bound, argument-root invariance, and the
/// result's member-roster containment are decided
/// separately by [`invariant_structural_call_admission`].
pub(crate) fn admissible_invariant_structural_call(
    node: &OptimizationNode,
) -> Option<(MachineId, terminal_psi::StructuralOperationResult)> {
    let O::CallStructural {
        psi_operation,
        result,
        callee,
        arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && matches!(
            result.multiplicity,
            terminal_psi::StructuralMultiplicity::Affine
                | terminal_psi::StructuralMultiplicity::Unrestricted
        )
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && selected_evidence.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())])
    .then(|| (*callee, result.clone()))
}

/// The complete unit-call admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned call shape
/// ([`admissible_invariant_unit_call`]) — which yields the exact internal
/// callee — the callee's transitive effect summary must prove no observable
/// effect, no crash, and no suspension, every node inside the component's
/// member roster must be unobservable under the same summaries, and the
/// component must preserve member-visible place contents and custody
/// ([`component_preserves_place_observations`]) — the whole-component bound
/// the byte family already replays, required here because the callee can
/// observe caller places through its borrows: only when no member mutates a
/// place any member could observe does the relocated call observe, on every
/// traversal, exactly what the in-loop invocation observed.
///
/// The callee's `structural_state` axis stays exempt under the same argument
/// the scalar call uses, tightened by the argument whitelist: every place the
/// callee could mutate is either activation-internal or reached through a
/// borrow the admission already gated — a shared borrow hands it no write
/// authority, and a mutable or write-only borrow may only name a
/// member-produced root the same run relocates and no other member observes —
/// so no member-visible place moves, and a `May` there reflects
/// callee-internal structural work or control edges carrying structural
/// bindings, not caller custody.
///
/// Each scalar `arguments` operand then obeys the shared use-site invariance
/// rule ([`member_scalar_operand_substitution`]). Each structural
/// argument's root must land somewhere the relocated run can see
/// it: already visible at the unique preheader insertion point, the
/// representative an invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]), or a root a node earlier in the
/// same run produced — `relocating_roots` — because the run preserves the
/// producer's declared place identity and orders it ahead of the call. A
/// member parameter the fixed point resolves rebinds on the moved node; a
/// member-produced root the run does not cover — or any other invisible root
/// — refuses the relocation. A mutable or write-only borrow's root instead
/// must be member-produced and already relocated by the same run: the callee
/// may write — and for a mutable borrow read — the cell, so only a fresh
/// per-traversal initializer keeps the invocation's view identical. An
/// `Owned` whole-root argument takes the same landing and additionally
/// requires the landed root to be copyable
/// ([`copyable_owned_argument_root`]) — an unrestricted owned parameter or
/// an unrestricted claim-free scalar-array result — so the relocated
/// invocation copies exactly the payload every in-loop invocation copied;
/// an `Owned` argument over affine or linear custody would move the
/// caller's place outright and stays refused.
///
/// Returns the scalar substitution plus the `(member parameter or
/// member-produced root, preheader-visible root)` rewrites the relocated
/// call performs on its structural arguments — empty when every argument
/// already names a visible or run-produced root.
pub(crate) fn invariant_unit_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let callee = admissible_invariant_unit_call(node)?;
    let O::CallUnit {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &BTreeSet::new(),
        effects,
    )
}

/// The complete scalar-result structural-call admission shared by the
/// proposal and the relocation freeze replay: `node` must carry the
/// source-owned call shape
/// ([`admissible_invariant_structural_scalar_call`]) — which yields the
/// exact internal callee — and then passes the unit call's whole evidence
/// surface unchanged: the pure transitive callee summary, the unobservable
/// member roster, the whole-component place-custody bound, the shared
/// scalar-operand substitution, and each structural argument's root —
/// borrow or copyable owned — landing somewhere the relocated run can see
/// it. The
/// place-custody bound carries one additional weight here: the relocated
/// call's scalar result is whatever the callee computed from those
/// arguments and borrows, so only when no member mutates or moves a place
/// does the preheader invocation return what every in-loop traversal's
/// invocation returned.
pub(crate) fn invariant_structural_scalar_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let callee = admissible_invariant_structural_scalar_call(node)?;
    let O::CallStructuralScalar {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &BTreeSet::new(),
        effects,
    )
}

/// The complete structural-result call admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned call
/// shape ([`admissible_invariant_structural_call`]) — which yields the exact
/// internal callee and its claim-free result. An affine result place must
/// stay inside the member roster spelled only through positions the
/// relocation's custody rewrite re-expresses
/// ([`scalar_case_result_contained`]): the producing call itself, the
/// member-block dispatch or structural return consuming it, and the edges
/// whose discard rosters the rewrite adjusts. An unrestricted result needs
/// neither bound: the place is a custody-free copy payload that never
/// enters `owned_places`, so no discard roster spells it and nothing about
/// its membership re-times. Every remaining evidence half is the borrow
/// calls' shared surface
/// ([`borrow_call_admission`]): the pure transitive callee, the unobservable
/// member roster, the shared scalar-operand substitution, the borrow and
/// copyable-owned argument whitelist, and each argument root's landing —
/// already preheader-visible, resolved through an invariant member
/// structural parameter, or produced by a node this component's run already
/// relocated — with a mutable or write-only borrow additionally requiring
/// that root's unique member producer among the relocated set and an
/// `Owned` argument requiring the landed root to declare an unrestricted
/// copyable shape.
///
/// The place-custody bound runs with the run's relocating roots plus an
/// affine result tolerated: a borrow argument lets the callee observe a
/// caller place the containment bound alone does not freeze, so the bound
/// must prove no member-visible place mutates or moves across traversals —
/// while the tolerated discards are exactly the confined results'
/// per-traversal disposal the relocation re-expresses rather than preserves.
/// Returns the scalar substitution plus the structural-argument root
/// rewrites the relocated call performs.
pub(crate) fn invariant_structural_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let (callee, result) = admissible_invariant_structural_call(node)?;
    let mut tolerated = relocating_roots.clone();
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
        if !scalar_case_result_contained(function, component, result.place) {
            return None;
        }
        tolerated.insert(result.place);
    }
    let O::CallStructural {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &tolerated,
        effects,
    )
}

/// The shared evidence every admitted structural-signature call replays once
/// its shape gate has yielded the exact internal callee and its
/// `structural_arguments` roster: the callee's transitive effect
/// summary must prove no observable effect, no crash, and no suspension
/// (the `structural_state` axis stays exempt — every caller place the callee
/// can write arrives through a borrow the place-custody bound already
/// accounted), every node inside the component's member roster
/// must be unobservable under the same summaries, and the component must
/// preserve member-visible place contents and custody
/// ([`component_preserves_place_observations`], run with `tolerated` member
/// discards — empty for the unit and scalar-result callers, the run's
/// relocating roots plus the call's own confined result for an affine
/// structural-result call, whose dispatch edges discard the fresh place the
/// relocation makes persistent) — only then does the
/// relocated call observe and return on every traversal exactly what the
/// in-loop invocation did. That bound is also what confines a mutating
/// borrow's authority: the call is tolerated inside the roster only when
/// every root its `MutableBorrow` or `WriteOnlyBorrow` arguments name is
/// exclusive to it, so no member read, second borrow, or store could observe
/// the callee's writes — and a `MutableBorrow` argument's root must be a
/// member-produced place a node earlier in the same run already relocated,
/// which is the only way the callee still reads the fresh initializer every
/// source traversal established. Each scalar `arguments` operand obeys the
/// shared use-site invariance rule
/// ([`member_scalar_operand_substitution`]), and
/// each structural argument's root must land where the relocated run can
/// see it — already visible at the unique preheader insertion point, the
/// representative an invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]), or a root a node earlier in the
/// same run produced — `relocating_roots`. Access refines the landing: a
/// shared borrow names any landed root, a mutable or write-only borrow only
/// a uniquely member-produced one the run already relocated, and an `Owned`
/// whole-root argument additionally requires the landed root to declare one
/// of the copyable shapes [`copyable_owned_argument_root`] recognizes — an
/// unrestricted owned parameter or an unrestricted claim-free scalar-array
/// result — because `Owned` over an unrestricted payload copies it into the
/// callee while `Owned` over affine or linear custody would move the
/// caller's place outright.
fn borrow_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    callee: MachineId,
    structural_arguments: &[terminal_psi::StructuralArgument],
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    tolerated: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    if !scalar_call_callee_pure(effects, callee) {
        return None;
    }
    if !component_members_unobservable(function, component, effects) {
        return None;
    }
    if !component_preserves_place_observations_tolerating(function, component, tolerated) {
        return None;
    }
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let preheader_source = shared_entry_source(component)?;
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    let representatives = invariant_member_place_parameters(function, component, relocating_roots);
    let mut rewrites = Vec::new();
    for argument in structural_arguments {
        match argument.access {
            // A mutating borrow hands the callee write — and for a mutable
            // borrow, read — authority over the argument's root. The only
            // admitted root is a member-produced place whose unique member
            // producer relocates in the same run ahead of the call: the
            // producer's declared place identity moves byte-exact, so the
            // argument keeps spelling it and no rewrite is needed. A second
            // member producer would let a staying establishment
            // re-initialize the cell behind the relocated call's single
            // invocation, so the producer count must be exactly one. A
            // caller root under
            // `MutableBorrow` is genuinely inadmissible — the callee may
            // read it, and its contents evolve across traversals as the
            // callee's own writes accumulate, so the relocated invocation
            // could not reproduce the per-traversal reads; under
            // `WriteOnlyBorrow` a caller root would be consistent in
            // principle — the callee cannot read it — but no admitted cyclic
            // source shape produces one today, so both take the single
            // member-produced rule. The spelled place must be the covered
            // root itself: a member structural parameter that resolves to
            // the root is not rebound here, because the callee writes
            // through the argument place it names and this admission never
            // substitutes a mutating argument's spelling.
            terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                if !(relocating_roots.contains(&argument.place)
                    && member_root_producer_count(function, component, argument.place) == 1)
                {
                    return None;
                }
            }
            terminal_psi::StructuralAccess::SharedBorrow => {
                let resolved = call_argument_root_landing(
                    function,
                    component,
                    preheader,
                    &representatives,
                    relocating_roots,
                    argument.place,
                )?;
                if resolved != argument.place {
                    rewrites.push((argument.place, resolved));
                }
            }
            // An `Owned` whole-root argument over a copyable root is the
            // observation a shared borrow is, spelled with value semantics:
            // the callee receives a copy of the payload, so the relocated
            // invocation copies — on the one traversal that runs — exactly
            // what every in-loop invocation copied. The root obeys the same
            // landing rule a borrow's does, then must declare one of the
            // copy shapes the cyclic owned-argument fence recognizes
            // ([`copyable_owned_argument_root`]): an unrestricted owned
            // parameter, or an unrestricted claim-free scalar-array result
            // — for a member-produced root, the landing rule already
            // required its unique producer to relocate in the same run
            // ahead of the call. An `Owned` argument over an affine or
            // linear root moves the caller's place into the callee — the
            // per-traversal transfer this boundary cannot re-express — and
            // a projected `Owned` argument or any other produced kind keeps
            // the refusal.
            terminal_psi::StructuralAccess::Owned => {
                if !argument.path.is_empty() {
                    return None;
                }
                let resolved = call_argument_root_landing(
                    function,
                    component,
                    preheader,
                    &representatives,
                    relocating_roots,
                    argument.place,
                )?;
                if !copyable_owned_argument_root(function, resolved) {
                    return None;
                }
                if resolved != argument.place {
                    rewrites.push((argument.place, resolved));
                }
            }
        }
    }
    Some((substitution, rewrites))
}

/// The root an admitted call argument may name once the call relocates:
/// `place` itself when it is already visible at the unique preheader
/// insertion point ([`place_observation_root_visible`]) or produced by a
/// node the same relocation run covers, else the representative an
/// invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]) when that lands the same way —
/// the place-level analog of [`member_scalar_operand_substitution`]. A
/// member-produced root qualifies only when its member producer is unique:
/// a second producer would re-establish the cell each traversal behind the
/// relocated call's single invocation.
fn call_argument_root_landing(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    preheader: &OptimizationBlock,
    representatives: &BTreeMap<PlaceId, PlaceId>,
    relocating_roots: &BTreeSet<PlaceId>,
    place: PlaceId,
) -> Option<PlaceId> {
    if place_observation_root_visible(function, preheader, place)
        || (relocating_roots.contains(&place)
            && member_root_producer_count(function, component, place) == 1)
    {
        return Some(place);
    }
    let representative = *representatives.get(&place)?;
    (place_observation_root_visible(function, preheader, representative)
        || (relocating_roots.contains(&representative)
            && member_root_producer_count(function, component, representative) == 1))
        .then_some(representative)
}

/// The complete scalar-call admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned call shape
/// ([`admissible_invariant_scalar_call`]) — which yields the exact internal
/// callee — the callee's transitive effect summary must prove no observable
/// effect, no crash, and no suspension, and every node inside the
/// component's member roster must be unobservable under the same summaries.
/// The member scan is the divergence half of call custody: a pure callee can
/// still fail to return, and relocating the call moves its possible
/// non-return ahead of every member node — member work the source traversal
/// performed before the call is skipped when the moved call never comes
/// back. That reorder is invisible only when no member performs observable
/// work, so the gate refuses the call when any member is observable; a
/// member call qualifies only under the same pure-callee rule the relocated
/// call obeys. Each scalar argument then obeys the shared use-site
/// invariance rule ([`member_scalar_operand_substitution`]): defined outside
/// the component, an invariant member parameter rebound to its agreed
/// representative, or the preserved result of a node earlier in the same
/// relocation run.
///
/// The callee's `structural_state` axis is deliberately exempt: a `Call`
/// passes only scalar arguments and returns only a scalar, so every place
/// the callee could touch is callee-internal — no caller-visible place
/// moves. The summary's `May` there only reflects control edges carrying
/// structural bindings, not reachable caller custody.
pub(crate) fn invariant_scalar_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<BTreeMap<ValueId, ValueId>> {
    let callee = admissible_invariant_scalar_call(node)?;
    if !scalar_call_callee_pure(effects, callee) {
        return None;
    }
    if !component_members_unobservable(function, component, effects) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// The effect-summary product computed over `unit`: the whole-unit
/// transitive per-function effect table call admission consults. The product
/// is keyed to `unit.identity` at computation, so both the proposal and the
/// freeze replay derive callee purity and member observability from the seed
/// rather than trusting any plan. An absent product yields an empty table —
/// every callee lookup then fails closed.
pub(crate) fn unit_effect_summaries(unit: &PsiOptimizationUnit) -> crate::EffectSummaryAnalysis {
    match crate::compute_analysis(unit, optimization_core::AnalysisKind::EffectSummaries) {
        Some(crate::AnalysisProduct::EffectSummaries(analysis)) => analysis,
        _ => crate::EffectSummaryAnalysis {
            nodes: Vec::new(),
            functions: Vec::new(),
        },
    }
}

/// Whether the transitive effect summary proves `callee` performs no
/// observable effect, no crash, and no suspension — the call-family purity a
/// relocated scalar call and a member call partner both need.
/// `structural_state` is deliberately not consulted: a scalar `Call` passes
/// no places, so any structural work its callee performs is callee-internal.
fn scalar_call_callee_pure(effects: &crate::EffectSummaryAnalysis, callee: MachineId) -> bool {
    effects
        .functions
        .iter()
        .find(|summary| summary.machine == callee)
        .is_some_and(|summary| {
            summary.observable == crate::EffectKnowledge::No
                && summary.crash == crate::EffectKnowledge::No
                && summary.suspension == crate::EffectKnowledge::No
        })
}

/// The statically realized callee a member call node invokes, when its call
/// family carries one: the direct `callee` of the internal-call variants, or
/// the sole permitted realization row of a rebound or stored dynamic
/// dispatch. Descriptor-parameter and dynamic-argument calls have no single
/// static callee here and stay unresolvable.
fn member_call_target(operation: &O) -> Option<MachineId> {
    match operation {
        O::Call { callee, .. }
        | O::CallUnit { callee, .. }
        | O::CallStructuralScalar { callee, .. }
        | O::CallStructural { callee, .. } => Some(*callee),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => Some(dynamic_dispatch.dispatch.realization),
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => Some(dynamic_dispatch.dispatch.realization),
        _ => None,
    }
}

/// Whether every node inside `component`'s member roster is unobservable —
/// the member half of call custody. Non-call members consult their own node
/// summary's observable axis; member calls must invoke a statically resolved
/// callee whose transitive summary is pure under the same rule the relocated
/// call obeys, so a descriptor-parameter or dynamic-argument member call —
/// no single static callee — refuses. A member node absent from the summary
/// refuses too: the gate fails closed rather than trusting a drifted table.
fn component_members_unobservable(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    effects: &crate::EffectSummaryAnalysis,
) -> bool {
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for (index, node) in block.nodes.iter().enumerate() {
            let index = match u32::try_from(index) {
                Ok(index) => index,
                Err(_) => return false,
            };
            match member_call_target(&node.operation) {
                Some(callee) => {
                    if !scalar_call_callee_pure(effects, callee) {
                        return false;
                    }
                }
                None => {
                    if matches!(
                        node.operation,
                        O::CallUnitWithDynamicArguments { .. }
                            | O::CallStructuralScalarWithDynamicArguments { .. }
                            | O::CallDynamicParameterScalar { .. }
                            | O::CallDynamicParameterUnit { .. }
                    ) {
                        return false;
                    }
                    let observable = effects
                        .nodes
                        .iter()
                        .find(|summary| {
                            summary.machine == function.machine
                                && summary.block == *member
                                && summary.node == index
                        })
                        .map(|summary| summary.observable);
                    if observable != Some(crate::EffectKnowledge::No) {
                        return false;
                    }
                }
            }
        }
    }
    true
}
