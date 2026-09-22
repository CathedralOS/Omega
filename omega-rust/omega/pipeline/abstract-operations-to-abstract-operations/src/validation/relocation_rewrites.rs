//! The rewrites a relocation performs on the transformed unit: the
//! scalar-case custody rewrite over exit edges and member cleanup rosters,
//! and the operand, place-root and call-root substitutions that rebind an
//! admitted operation from member parameters to their representatives.

use super::place_observations::member_place_references;
use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

/// Whether the affine scalar-case, empty-record, or structural-call result
/// — or the affine trivial-local place — `picked` stays
/// inside `component`'s
/// member roster spelled only through positions the relocation's custody
/// rewrite covers: the producing establishment or call itself, a `StructuralCase`
/// dispatch or read-only inspection of the sum, a structural return's source
/// or exit-disposal roster, and the member-internal or exit edges whose
/// discard rosters the rewrite adjusts. The cyclic eligibility fence already
/// proved `picked` is produced and dispatched (or returned) in one member
/// block and discarded on every dispatch edge; once the establishment lands
/// in the preheader the result lives through every member, so any other
/// spelling — a store destination, a call's structural argument, a reference
/// carrier, an edge structural binding — would move or observe custody the
/// rewrite cannot re-express and refuses. An unmodeled member operation
/// fails closed for the same reason. `picked` on a residual discard, or on
/// an edge departing a non-member block, is impossible in a verified seed —
/// the result is dead outside the roster — and refuses anyway so the replay
/// never trusts it. A member `ReturnUnit` whose cleanup already invokes a
/// nominal receiver has no `DiscardRoot` slot the rewrite can spell, so that
/// one terminator shape refuses; every other member terminator is
/// expressible: member `Return`/`ReturnUnit` blocks insert
/// `DiscardRoot(picked)` in schedule order, a member `ReturnStructural`
/// either returns `picked` outright or lists it in `trivial_affine_discards`,
/// and a `Crash` carries only claims.
pub(super) fn scalar_case_result_contained(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    picked: PlaceId,
) -> bool {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    for block in &function.blocks {
        for node in &block.nodes {
            let member = members.contains(&block.id);
            let confined = match &node.operation {
                // The producer spells its own result — a declaration, not a
                // use of an existing place — and only a member node can
                // produce it inside this component. A `CallStructural`
                // producer is the same declaration position: the admitted
                // relocation carries an affine claim-free result the cyclic
                // eligibility fence already confined to the producing
                // member block's dispatch or return. An `EstablishRecord`
                // producer is likewise the empty declaration's own place —
                // the admissibility gate confines an affine result to the
                // field-free shape, so no structural field argument can
                // spell `picked` here.
                O::EstablishScalarCase { result, .. }
                | O::EstablishRecord { result, .. }
                | O::CallStructural { result, .. }
                    if result.place == picked =>
                {
                    member
                }
                // A trivial affine local producer is the same declaration
                // position: the establishment spells its own declared place,
                // and only a member node can produce it inside this
                // component — the cyclic eligibility fence already confined
                // the fresh affine place to the producing member's disposal
                // edges.
                O::EstablishTrivialAffineLocal { place, .. } if place.id == picked => member,
                // Dispatch, inspection, and structural-return positions keep
                // the result inside re-expressible custody when they live
                // inside the roster.
                O::StructuralCase { source, .. }
                | O::StructuralCaseMembership { source, .. }
                | O::IntegerStructuralField { source, .. }
                | O::BooleanStructuralField { source, .. }
                | O::PrimitiveScalarRead { source, .. }
                | O::ByteSequenceRead { source, .. }
                | O::ByteSequenceSubslice { source, .. }
                | O::ByteSequenceLength { source, .. }
                | O::StructuralByteSequenceFieldLength { source, .. }
                    if *source == picked =>
                {
                    member
                }
                // A member structural return disposes `picked` through its
                // exit roster or returns it outright — either shape is
                // expressible — while a non-member return cannot name a
                // place that is dead outside the member roster.
                O::ReturnStructural { source, .. } => member || *source != picked,
                operation => {
                    // Every modeled operation spelling `picked` outside the
                    // whitelist — a store destination, a call argument, a
                    // produced root collision — refuses. An operation this
                    // scan cannot model could spell it anywhere, so a member
                    // occurrence fails closed; a non-member operation cannot
                    // spell `picked` in a verified seed at all.
                    let mut references = BTreeSet::new();
                    if member_place_references(operation, &mut references) {
                        !references.contains(&picked)
                    } else {
                        !member
                    }
                }
            };
            if !confined {
                return false;
            }
            if !member {
                continue;
            }
            // A member `Return`/`ReturnUnit` cleanup invoking a nominal
            // receiver has no `DiscardRoot` slot for `picked` — the cleanup
            // shape requires all-nominal actions once any appear — so the
            // custody rewrite could not express its disposal and the
            // relocation refuses.
            if let O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } = &node.operation
                && cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                })
            {
                return false;
            }
            for edge in &node.successors {
                if edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
                    || edge.structural_bindings.iter().any(|binding| {
                        binding.parameter == picked || binding.argument.place == picked
                    })
                {
                    return false;
                }
            }
        }
    }
    // `picked` on an edge departing outside the member roster is dead-place
    // custody a verified seed cannot carry — refuse rather than trust it.
    function
        .blocks
        .iter()
        .filter(|block| !members.contains(&block.id))
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .all(|edge| {
            !edge.trivial_affine_discards.contains(&picked)
                && !edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
        })
}

/// Rewrite one retained member node's edge and cleanup custody for the
/// relocated affine scalar-case, empty-record, and structural-call results
/// in
/// `case_results`: strip each such
/// place from every member-internal edge — the persistent preheader result
/// stays live across the traversal where the source's fresh place died at
/// dispatch — and insert it, in the Terminal cleanup schedule's order, on
/// every exit edge that did not already dispose it, in every member
/// `Return`/`ReturnUnit` cleanup roster, and in a member `ReturnStructural`'s
/// `trivial_affine_discards` when some other root is returned. The operation's
/// own edge lists, the `successors` mirror, and the `Cleanup` ownership
/// mirror are rewritten in lockstep so the transformed node still satisfies
/// `successors_match_operation` and `expected_ownership`. The same rewrite
/// normalizes the seed's retained nodes inside the relocation freeze replay,
/// so the fence compares the admitted transformed spelling against the same
/// re-derived custody rather than trusting it.
pub(crate) fn rewrite_scalar_case_custody(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    members: &BTreeSet<BlockId>,
    case_results: &BTreeSet<PlaceId>,
    node: &mut OptimizationNode,
) {
    if case_results.is_empty() {
        return;
    }
    match &mut node.operation {
        O::Jump {
            target,
            trivial_affine_discards,
            ..
        } => {
            rewrite_case_edge_discards(
                structural_places,
                members,
                *target,
                case_results,
                trivial_affine_discards,
            );
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for successor in [when_true, when_false] {
                rewrite_case_edge_discards(
                    structural_places,
                    members,
                    successor.target,
                    case_results,
                    &mut successor.trivial_affine_discards,
                );
            }
        }
        O::StructuralCase { cases, .. } => {
            for case in cases {
                rewrite_case_edge_discards(
                    structural_places,
                    members,
                    case.target,
                    case_results,
                    &mut case.trivial_affine_discards,
                );
            }
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for place in case_results {
                insert_case_cleanup_discard(structural_places, cleanup_actions, *place);
            }
        }
        O::ReturnStructural {
            source,
            trivial_affine_discards,
            ..
        } => {
            for place in case_results {
                if *source != *place {
                    insert_case_discard_ordered(structural_places, trivial_affine_discards, *place);
                }
            }
        }
        _ => {}
    }
    for edge in &mut node.successors {
        rewrite_case_edge_discards(
            structural_places,
            members,
            edge.target,
            case_results,
            &mut edge.trivial_affine_discards,
        );
    }
    for event in &mut node.ownership {
        if let OwnershipEvent::Cleanup(actions) = event {
            for place in case_results {
                insert_case_cleanup_discard(structural_places, actions, *place);
            }
        }
    }
}

/// Adjust one edge's `trivial_affine_discards` for relocated scalar-case
/// results: a member-internal target keeps the persistent place live, so its
/// discard is stripped; an exit target disposes it at the position the
/// Terminal cleanup schedule assigns — operation-result places precede
/// locals and parameters, ordered by descending producer.
fn rewrite_case_edge_discards(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    members: &BTreeSet<BlockId>,
    target: BlockId,
    case_results: &BTreeSet<PlaceId>,
    discards: &mut Vec<PlaceId>,
) {
    if members.contains(&target) {
        discards.retain(|place| !case_results.contains(place));
    } else {
        for place in case_results {
            insert_case_discard_ordered(structural_places, discards, *place);
        }
    }
}

/// Insert `place` into an ordered `trivial_affine_discards` roster at the
/// slot the Terminal cleanup schedule assigns: a relocated case result or
/// trivial affine local sorts ahead of parameters, with operation results
/// ordered by descending producer and locals by descending declaration
/// ordinal — see [`case_result_schedule_key`]. A roster already listing
/// `place` — a dispatch edge that exits the component — is left byte-exact.
fn insert_case_discard_ordered(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    discards: &mut Vec<PlaceId>,
    place: PlaceId,
) {
    if discards.contains(&place) {
        return;
    }
    let Some(key) = case_result_schedule_key(structural_places, place) else {
        return;
    };
    let position = discards
        .iter()
        .position(|existing| {
            case_result_schedule_key(structural_places, *existing)
                .is_none_or(|existing_key| existing_key >= key)
        })
        .unwrap_or(discards.len());
    discards.insert(position, place);
}

/// Insert `DiscardRoot(place)` into a `Return`/`ReturnUnit` cleanup roster at
/// the same schedule slot [`insert_case_discard_ordered`] computes: ahead of
/// the first action that is not an operation-result root with a strictly
/// earlier schedule key, which keeps it inside the leading root run ahead of
/// any residual or parameter actions.
fn insert_case_cleanup_discard(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    actions: &mut Vec<terminal_psi::TerminalAffineCleanupAction>,
    place: PlaceId,
) {
    if actions.iter().any(|action| {
        matches!(
            action,
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(existing)
                if *existing == place
        )
    }) {
        return;
    }
    let Some(key) = case_result_schedule_key(structural_places, place) else {
        return;
    };
    let position = actions
        .iter()
        .position(|action| {
            !matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(existing)
                    if case_result_schedule_key(structural_places, *existing)
                        .is_some_and(|existing_key| existing_key < key)
            )
        })
        .unwrap_or(actions.len());
    actions.insert(
        position,
        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
    );
}

/// The schedule slot `expected_trivial_affine_discards` assigns a place:
/// rank 0 is an operation result — `Reverse(producer)`, later producers
/// discard first — and rank 1 is a trivial affine local —
/// `Reverse(declaration_ordinal)`, later declarations first — with the
/// `structural_places` declaration index preserving the roster's stable
/// order for equal keys. Parameters have no key and always sort last.
fn case_result_schedule_key(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    place: PlaceId,
) -> Option<(u8, std::cmp::Reverse<u64>, usize)> {
    structural_places
        .iter()
        .enumerate()
        .find_map(|(index, declaration)| {
            (declaration.id == place).then_some(match declaration.kind {
                StructuralPlaceKind::OperationResult { producer, .. } => {
                    (0, std::cmp::Reverse(producer.get()), index)
                }
                StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    ..
                } => (1, std::cmp::Reverse(u64::from(declaration_ordinal)), index),
                _ => return None,
            })
        })
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
        // A relocated call rebinds each scalar `arguments` operand through
        // the same invariant-parameter substitution a pure computation uses
        // — every admitted call variant spells its scalar operands in that
        // one field. `callee`, results, `structural_arguments`,
        // `claim_transfers`, and `requirement_obligations` are not scalar
        // operand positions — they stay byte-exact inside the moved
        // operation, while the structural argument roots rebind through
        // `substitute_invariant_call_roots`. `crash_continuations` are not
        // substituted in place either: an admitted crash-custody call
        // re-derives them from the callee's published routes through
        // [`call_crash_continuations`].
        O::Call { arguments, .. }
        | O::CallUnit { arguments, .. }
        | O::CallStructuralScalar { arguments, .. }
        | O::CallStructural { arguments, .. } => {
            for argument in arguments {
                substitute(argument, substitution);
            }
        }
        // A relocated primitive-local establishment rebinds its initializing
        // `value` through the same invariant-parameter substitution a pure
        // computation uses — the declared place, structural type, and result
        // custody stay byte-exact inside the moved operation.
        O::EstablishPrimitiveLocal { value, .. } => {
            substitute(&mut value.value, substitution);
        }
        // A relocated scalar-array establishment rebinds each element operand
        // through the same invariant-parameter substitution — the declared
        // place, structural type, and result custody stay byte-exact inside
        // the moved operation.
        O::EstablishScalarArray { elements, .. } => {
            for element in elements {
                substitute(element, substitution);
            }
        }
        // A relocated scalar-case establishment rebinds each case-field value
        // through the same invariant-parameter substitution — the declared
        // place, structural type, result case, result custody, field
        // identities, and bounded-integer range obligations stay byte-exact
        // inside the moved operation.
        O::EstablishScalarCase { fields, .. } => {
            for field in fields {
                substitute(&mut field.value, substitution);
            }
        }
        // A relocated record establishment rebinds each scalar field
        // initializer through the same invariant-parameter substitution —
        // the declared place, structural type, result custody, declaration
        // order, structural field arguments, and bounded-integer range
        // obligations stay byte-exact inside the moved operation.
        O::EstablishRecord { fields, .. } => {
            for field in fields {
                if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                    substitute(value, substitution);
                }
            }
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

/// Rebind the structural-argument roots of an admitted call operation or the
/// structural-field argument roots of an admitted record establishment:
/// `rewrites` maps each member-parameter root the
/// admission resolved to the preheader-visible or run-covered root the
/// relocated call now
/// names. Every static-call variant carries the same `structural_arguments`
/// field shape, so the substitution walks whichever one the operation is —
/// `CallUnit` and `CallStructuralScalar` are the admitted
/// structural-signature families today (`CallStructural` stays matched for
/// shape completeness), and a relocation never reaches here for an
/// operation the admission refused. A shared-borrow argument whose root a
/// node earlier in the same run produced — an `EstablishPrimitiveLocal` or a
/// byte-literal declaration — needs no rewrite at all: the run keeps the
/// producer's declared place identity byte-exact. A relocated
/// `EstablishRecord` instead rebinds the `place` of each structural field
/// initializer whose copied root is an invariant member structural
/// parameter — a field naming a run-produced root keeps spelling it
/// byte-exact.
/// Returns `true` only when every requested rewrite found at least one
/// argument to rebind: a planned rewrite that fires on no argument means the
/// plan drifted from the operation, which the relocation callers treat as a
/// candidate mismatch. Multiple arguments may share a root — one rewrite
/// rebinds them all.
pub(crate) fn substitute_invariant_call_roots(
    operation: &mut O,
    rewrites: &BTreeMap<PlaceId, PlaceId>,
) -> bool {
    let mut applied = BTreeSet::new();
    match operation {
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
            for argument in structural_arguments.iter_mut() {
                if let Some(root) = rewrites.get(&argument.place) {
                    applied.insert(argument.place);
                    argument.place = *root;
                }
            }
        }
        O::EstablishRecord { fields, .. } => {
            for field in fields.iter_mut() {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &mut field.value
                    && let Some(root) = rewrites.get(&argument.place)
                {
                    applied.insert(argument.place);
                    argument.place = *root;
                }
            }
        }
        _ => {}
    }
    rewrites.keys().all(|from| applied.contains(from))
}

/// The crash continuations a `Call` to `callee` carries once its actual
/// scalar arguments are `arguments`: the callee's verifier-owned published
/// crash routes instantiated under the formal→actual substitution — the
/// same derivation [`terminal_verifier::substitute_crash_routes`] performs
/// when the terminal verifier checks an invocation's custody roster. A
/// relocated call re-derives the roster against its substituted arguments
/// rather than carrying the member-spelled payload byte-exact, so the moved
/// node's crash evidence is reconstructed, not trusted. `None` when the
/// callee carries no verifier-owned contract — a bare reconstruction seed
/// cannot re-derive routes — or when the contract declares erased scalar or
/// proof formals, whose erased-argument substitution rows this
/// reconstruction does not track.
pub(crate) fn call_crash_continuations(
    callee: &PsiOptimizationFunction,
    arguments: &[ValueId],
) -> Option<Vec<terminal_psi::CrashRouteBucket>> {
    let contract = callee.verified_contract.as_ref()?;
    if !(contract.erased_scalar_formals.is_empty()
        && contract.erased_proof_formals.is_empty()
        && callee.parameters.len() == arguments.len())
    {
        return None;
    }
    let substitutions = callee
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            (
                parameter.value,
                ScalarTerm::value(*argument, parameter.scalar_type),
            )
        })
        .collect();
    Some(terminal_verifier::substitute_crash_routes(
        &contract.crash_routes,
        &substitutions,
    ))
}
