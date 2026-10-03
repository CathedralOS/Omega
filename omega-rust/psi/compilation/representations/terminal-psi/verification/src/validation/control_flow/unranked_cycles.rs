//! Eligibility for cyclic scalar work, owned inputs, locals, views, receivers,
//! unrestricted value construction and complete records under ordinary custody,
//! trivial affine local establishments, unrestricted plain leaf copies,
//! field-replacement window pairs whose displaced value is disposed before
//! re-entry, affine structural call results with call-scoped loans that are
//! disposed before re-entry, and entry claims pinned on owned machine
//! parameters for the machine's whole cyclic lifetime.

use super::{
    OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};
use crate::validation::{
    BTreeSet, BlockId, ClaimId, ControlCycleIneligibility, OperationResult, PlaceId,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceKind, TerminalAffineCleanupAction,
};
use crate::validation::{block_views, primitive_storage};

/// Eligibility carries no proof or dominance authority. The caller runs the
/// ordinary operand, view, successor, and frontier checks after this fence.
/// Returns the first clause the machine fails so the decline names the wall it
/// hit rather than a bare cycle rejection.
pub(super) fn ineligibility(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<ControlCycleIneligibility> {
    let claim_roots = machine
        .entry_claims
        .iter()
        .map(|claim| claim.input)
        .chain(
            machine
                .content_entry_claims
                .iter()
                .map(|claim| claim.input.root),
        )
        .collect::<BTreeSet<PlaceId>>();
    let scalar_case_result = machine.result.structural().is_some_and(|result| {
        matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && crate::validation::scalar::case::plain_type(module, result.structural_type)
    });
    let plain_record_result = machine.result.structural().is_some_and(|result| {
        matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && module.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            })
            && crate::validation::record::plain_type(module, result.structural_type)
    });
    if machine.result.structural().is_some() && !scalar_case_result && !plain_record_result {
        return Some(ControlCycleIneligibility::ResultShape);
    }
    if !claims_pinned_at_entry(machine, &claim_roots) {
        return Some(ControlCycleIneligibility::UnpinnedClaims);
    }
    if !machine.content_identity_reshuffles.is_empty() {
        return Some(ControlCycleIneligibility::ContentIdentityReshuffles);
    }
    if !machine.content_partition_compositions.is_empty() {
        return Some(ControlCycleIneligibility::ContentPartitionCompositions);
    }
    // Invocation storage requirements are entry facts, not loop invariants.
    // Reconstruction invalidates them on writes and drops them at feedback
    // cuts; only independently proved destination arrival rows reintroduce
    // current-storage facts. Cyclic shape alone supplies no proof authority.
    if let Some(index) = machine.structural_parameters.iter().position(|parameter| {
        !plain_owned(parameter)
            && !(parameter.access == StructuralAccess::Owned
                && claim_roots.contains(&parameter.place))
            // A shared bounded-owner loan freezes its referent: every loop
            // arrival observes the same live length and byte predicates.
            && !block_views::shared_bounded_owner_loan(module, parameter)
            && (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !(persistent_receiver(module, parameter)
                    || block_views::mutable_records::parameter_type(module, parameter)
                    // A shared loan freezes its referent, so it cannot admit
                    // less than an exclusive loan of the same carrier: this
                    // asks the same classifier the mutable arm above asks.
                    // Spelling a narrower root list here refused a shared
                    // `&[i32; 6]` in a cycle while `&mut [i32; 6]` passed, and
                    // `record::plain_type` -- the safety conjunct either way --
                    // is defined for a fixed array exactly as for a record,
                    // "its element leaf repeated".
                    || (parameter.access == StructuralAccess::SharedBorrow
                        && (block_views::mutable_records::borrowed_carrier_type(
                            module,
                            parameter.structural_type,
                        ) || crate::validation::record::shared_view_carrier_type(
                            module,
                            parameter.structural_type,
                        )))
                    || ((parameter.access == StructuralAccess::SharedBorrow
                        || (parameter.access == StructuralAccess::MutableBorrow
                            && (machine.result == TerminalMachineResult::Unit
                                || scalar_case_result
                                // A scalar result moves no custody — the
                                // borrowed carrier's cycle reads and writes
                                // carry the identical proof as `Unit`.
                                || machine.result.scalar().is_some())))
                        && module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && (matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(carrier)
                                    if carrier.is_borrowed()
                                ) || matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ElementView { .. }
                                ) || matches!(
                                    declaration.shape,
                                    StructuralTypeShape::PrimitiveScalar(_)
                                ))
                        }))))
    }) {
        return Some(ControlCycleIneligibility::Parameter { index });
    }
    if let Some(place) = machine.structural_places.iter().find(|place| {
        !(matches!(
            place.kind,
            StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. }
                | StructuralPlaceKind::OperationResult { .. }
                | StructuralPlaceKind::ByteSequenceLiteral { .. }
                | StructuralPlaceKind::ProviderAttachment { .. }
                | StructuralPlaceKind::TrivialAffineLocal { .. }
        ) || (scalar_case_result || plain_record_result)
            && place.kind == StructuralPlaceKind::Result)
    }) {
        return Some(ControlCycleIneligibility::StructuralPlace { place: place.id });
    }
    let cyclic_blocks = crate::control_graph::cyclic_components(machine)
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    machine.blocks.iter().find_map(|block| {
        let case_source = confined_case_source(module, block);
        let terminator_eligible =
            matches!(
                block.terminator,
                Terminator::Jump { .. }
                    | Terminator::Conditional { .. }
                    | Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::Crash { .. }
            ) || matches!(block.terminator, Terminator::StructuralCase { .. })
                && case_source.is_some()
                || (scalar_case_result || plain_record_result) && matches!(&block.terminator,
                    Terminator::ReturnStructural { source, returned_claims, .. }
                    if returned_claims.is_empty()
                        && (crate::validation::scalar::case::plain_return_source(module, machine, *source)
                            || (plain_record_result && crate::validation::record::plain_return_source(module, machine, *source))));
        if !terminator_eligible {
            return Some(ControlCycleIneligibility::BlockTerminator { block: block.id });
        }
        block
            .operations
            .iter()
            .find(|operation| {
                // A prefix/exit constructor executes outside every cycle and
                // may establish an affine plain home. The frontier still owns
                // it through all arrivals and requires exact final disposal.
                (cyclic_blocks.contains(&block.id)
                    || operation.result.structural().is_none_or(|result|
                        block_views::mutable_records::owned_home(module, machine, result.place).is_none()))
                    && !cycle_operation_eligible(module, machine, block, case_source, &cyclic_blocks, operation)
            })
            .map(|operation| ControlCycleIneligibility::BlockOperation {
                block: block.id,
                operation: operation.id,
            })
    })
}

/// Claim custody around a cycle is pinned when every claim root is an owned
/// machine entry parameter and no block parameter, operation, or terminator
/// anywhere in the machine names a root or moves a claim identity. Nothing
/// can then rebind, borrow, consume, discard, or transfer the claimed places
/// during the machine's cyclic lifetime, so each loop arrival presents the
/// identical claim frontier; the traversal's per-arrival snapshot comparison
/// remains the actual custody proof. A machine carrying claims that are not
/// pinned keeps this fence closed.
fn claims_pinned_at_entry(machine: &TerminalMachine, roots: &BTreeSet<PlaceId>) -> bool {
    let identities = machine
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .chain(machine.content_entry_claims.iter().map(|claim| claim.claim))
        .collect::<BTreeSet<ClaimId>>();
    roots.iter().all(|root| {
        machine.structural_parameters.iter().any(|parameter| {
            parameter.place == *root && parameter.access == StructuralAccess::Owned
        })
    }) && machine.blocks.iter().all(|block| {
        block
            .structural_parameters
            .iter()
            .all(|parameter| !roots.contains(&parameter.place))
            && block
                .operations
                .iter()
                .all(|operation| operation_leaves_custody(operation, roots, &identities))
            && terminator_leaves_custody(&block.terminator, roots, &identities)
    })
}

/// Any mention of a pinned root place by an operation — read, write, borrow,
/// move, or establishment — disturbs the anchor, as does binding or
/// transferring a pinned claim identity.
fn operation_leaves_custody(
    operation: &terminal_psi::Operation,
    roots: &BTreeSet<PlaceId>,
    identities: &BTreeSet<ClaimId>,
) -> bool {
    let clears = |place: PlaceId| !roots.contains(&place);
    let retains = |claim: ClaimId| !identities.contains(&claim);
    if let OperationResult::Structural(result) = &operation.result
        && (!clears(result.place) || result.claims.iter().any(|binding| !retains(binding.claim)))
    {
        return false;
    }
    match &operation.kind {
        OperationKind::EstablishReference { source } => clears(source.place),
        OperationKind::ByteSequencePredicate { operand, .. } => clears(operand.source()),
        OperationKind::ReleaseReference { source }
        | OperationKind::PrimitiveScalarRead { source, .. }
        | OperationKind::StructuralByteSequenceFieldLength { source, .. }
        | OperationKind::StructuralByteSequenceFieldRead { source, .. }
        | OperationKind::StructuralCaseMembership { source, .. }
        | OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. }
        | OperationKind::ByteSequenceSubslice { source, .. }
        | OperationKind::ElementViewLength { source }
        | OperationKind::ElementViewRead { source, .. }
        | OperationKind::ElementViewSubslice { source, .. }
        | OperationKind::BooleanStructuralField { source, .. }
        | OperationKind::IntegerStructuralField { source, .. } => clears(*source),
        OperationKind::EstablishElementView {
            destination,
            source,
            ..
        }
        | OperationKind::EstablishMutableElementView {
            destination,
            source,
            ..
        } => clears(*destination) && clears(source.place),
        OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
        | OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::AtomicAccess {
            place: destination, ..
        }
        | OperationKind::StructuralScalarFieldStore { destination, .. }
        | OperationKind::EstablishByteSequenceLiteral { destination, .. }
        | OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::ElementViewWrite { destination, .. }
        | OperationKind::EstablishTrivialAffineLocal { destination } => clears(*destination),
        OperationKind::ByteSequenceStore {
            destination,
            source,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination,
            source,
            ..
        } => clears(*destination) && clears(*source),
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => {
            fields.iter().all(|field| match &field.value {
                terminal_psi::RecordFieldValue::Structural(argument) => clears(argument.place),
                _ => true,
            })
        }
        OperationKind::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => {
            structural_arguments
                .iter()
                .all(|argument| clears(argument.place))
                && claim_transfers
                    .iter()
                    .all(|transfer| retains(transfer.claim))
        }
        OperationKind::CallStructural {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => {
            structural_arguments
                .iter()
                .all(|argument| clears(argument.place))
                && claim_transfers
                    .iter()
                    .all(|transfer| retains(transfer.claim))
                && returned_claim_transfers
                    .iter()
                    .all(|transfer| retains(transfer.caller_claim))
        }
        OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } => {
            structural_arguments
                .iter()
                .all(|argument| clears(argument.place))
                && completion_receipts
                    .iter()
                    .all(|receipt| retains(receipt.claim))
        }
        _ => true,
    }
}

/// An edge's structural bindings, affine discards, and cleanup actions must
/// not name a pinned root, and a structural return must not hand back a
/// pinned claim. A crash's lower bound only asserts that pinned claims are
/// still live at the site, which ordinary crash-frontier validation checks.
fn terminator_leaves_custody(
    terminator: &Terminator,
    roots: &BTreeSet<PlaceId>,
    identities: &BTreeSet<ClaimId>,
) -> bool {
    let clears = |place: &PlaceId| !roots.contains(place);
    let edge_leaves_custody =
        |structural_arguments: &[StructuralArgument],
         trivial_affine_discards: &[PlaceId],
         residual_affine_discards: &[terminal_psi::StructuralAffineDiscard]| {
            structural_arguments
                .iter()
                .all(|argument| clears(&argument.place))
                && trivial_affine_discards.iter().all(clears)
                && residual_affine_discards
                    .iter()
                    .all(|discard| clears(&discard.place))
        };
    match terminator {
        Terminator::Jump {
            structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => edge_leaves_custody(
            structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
        ),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false].iter().all(|edge| {
            edge_leaves_custody(
                &edge.structural_arguments,
                &edge.trivial_affine_discards,
                &[],
            )
        }),
        Terminator::StructuralCase { source, cases } => {
            clears(source)
                && cases
                    .iter()
                    .all(|case| case.trivial_affine_discards.iter().all(clears))
        }
        Terminator::Return {
            cleanup_actions, ..
        } => cleanup_actions.iter().all(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => clears(place),
            TerminalAffineCleanupAction::DiscardResidual(discard) => clears(&discard.place),
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => clears(&cleanup.place),
        }),
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } => trivial_affine_discards.iter().all(clears),
        Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => edge_leaves_custody(&[], trivial_affine_discards, residual_affine_discards),
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
            cleanups.iter().all(|cleanup| clears(&cleanup.place))
        }
        Terminator::ReturnStructural {
            source,
            returned_claims,
            trivial_affine_discards,
            ..
        } => {
            clears(source)
                && trivial_affine_discards.iter().all(clears)
                && returned_claims
                    .iter()
                    .all(|claim| !identities.contains(claim))
        }
        Terminator::Crash { .. } => true,
    }
}

/// Whether some edge leaving a cyclic block schedules `place` for disposal.
/// The producing block's own edges count the same as a successor's — the
/// check is "a traversal cannot re-enter the cycle head still owning the
/// place", which the fixed-point frontier join then proves edge by edge.
fn result_discarded_in_cycle(
    machine: &TerminalMachine,
    cyclic_blocks: &BTreeSet<BlockId>,
    place: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .filter(|block| cyclic_blocks.contains(&block.id))
        .flat_map(|block| terminator_discards(&block.terminator))
        .any(|discarded| discarded == place)
}

/// Whether some window store in a cyclic block consumes `place` as its repair
/// value — the swap spelling, where one displaced subtree reseats another
/// hole. The store's consumption is the disposal, which the frontier join
/// then proves identical on every traversal, as for a discard roster.
fn reseated_in_cycle(
    machine: &TerminalMachine,
    cyclic_blocks: &BTreeSet<BlockId>,
    place: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .filter(|block| cyclic_blocks.contains(&block.id))
        .flat_map(|block| &block.operations)
        .any(|operation| {
            matches!(&operation.kind, OperationKind::StoreStructuralField { value, .. }
                if value.place == place)
        })
}

/// Whether some edge leaving a cyclic block moves `place` whole into an owned
/// successor block parameter. The edge's two-phase binding consumes the
/// source before it establishes the target, so the moved value is the
/// successor parameter's custody from then on; that parameter's own disposal
/// before re-entry is the fixed-point join's to prove, like any loop-carried
/// block parameter's.
fn moved_to_successor_in_cycle(
    machine: &TerminalMachine,
    cyclic_blocks: &BTreeSet<BlockId>,
    place: PlaceId,
) -> bool {
    machine
        .blocks
        .iter()
        .filter(|block| cyclic_blocks.contains(&block.id))
        .flat_map(|block| successor_bindings(&block.terminator))
        .any(|(target, arguments)| {
            machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == target)
                .is_some_and(|target| {
                    arguments.iter().zip(&target.structural_parameters).any(
                        |(argument, parameter)| {
                            argument.place == place
                                && argument.path.is_empty()
                                && argument.access == StructuralAccess::Owned
                                && parameter.access == StructuralAccess::Owned
                        },
                    )
                })
        })
}

/// Result custody is independent of whether a constructor or a call produced
/// it. This is only an eligibility fence: the ordinary frontier walk proves
/// consumption on every path and rejects a second production while the old
/// value is still live. A consumer in another block is therefore sufficient
/// here, but a consumer on only one branch cannot make an invalid join pass.
fn result_disposed_before_reentry(
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    cyclic_blocks: &BTreeSet<BlockId>,
    place: PlaceId,
) -> bool {
    !cyclic_blocks.contains(&block.id)
        || result_discarded_in_cycle(machine, cyclic_blocks, place)
        || moved_to_successor_in_cycle(machine, cyclic_blocks, place)
        || reseated_in_cycle(machine, cyclic_blocks, place)
}

/// The structural successor bindings of each edge that carries them.
fn successor_bindings(terminator: &Terminator) -> Vec<(BlockId, &[StructuralArgument])> {
    match terminator {
        Terminator::Jump {
            target,
            structural_arguments,
            ..
        } => vec![(*target, structural_arguments.as_slice())],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (when_true.target, when_true.structural_arguments.as_slice()),
            (
                when_false.target,
                when_false.structural_arguments.as_slice(),
            ),
        ],
        _ => Vec::new(),
    }
}

fn terminator_discards(terminator: &Terminator) -> Vec<PlaceId> {
    match terminator {
        Terminator::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        }
        | Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => trivial_affine_discards
            .iter()
            .copied()
            .chain(residual_affine_discards.iter().map(|discard| discard.place))
            .collect(),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .trivial_affine_discards
            .iter()
            .copied()
            .chain(when_false.trivial_affine_discards.iter().copied())
            .collect(),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .flat_map(|case| case.trivial_affine_discards.iter().copied())
            .collect(),
        Terminator::Return {
            cleanup_actions, ..
        } => cleanup_actions
            .iter()
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard.place),
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup.place),
            })
            .collect(),
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        }
        | Terminator::ReturnStructural {
            trivial_affine_discards,
            ..
        } => trivial_affine_discards.clone(),
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
            cleanups.iter().map(|cleanup| cleanup.place).collect()
        }
        Terminator::Crash { .. } => Vec::new(),
    }
}

fn cycle_operation_eligible(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    case_source: Option<semantic_vocabulary::PlaceId>,
    cyclic_blocks: &BTreeSet<BlockId>,
    operation: &terminal_psi::Operation,
) -> bool {
    match &operation.kind {
        // All unrestricted constructors have one lifetime: their ordinary
        // validator authenticates the payload, and each traversal publishes
        // a fresh value without disposal debt. Dominance checks require this
        // traversal's operands; frontier replay prevents rebinding a result
        // still pinned by a shared loan. No source-arrangement roster belongs
        // here. Affine construction retains its separate disposal rules.
        OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishStructuralArray { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishByteSequenceConcatenation { .. } => {
            let Some(result) = operation.result.structural() else {
                return false;
            };
            let lifetime_valid = result.multiplicity == StructuralMultiplicity::Unrestricted
                || match &operation.kind {
                    OperationKind::EstablishScalarCase { .. } => {
                        matches!(&block.terminator, Terminator::ReturnStructural { source, .. }
                            if *source == result.place)
                            || case_source == Some(result.place)
                            || (result.multiplicity == StructuralMultiplicity::Affine
                                && result.qualification_establishments.is_empty()
                                && result_disposed_before_reentry(
                                    machine,
                                    block,
                                    cyclic_blocks,
                                    result.place,
                                ))
                    }
                    // A complete record transfers its children's custody into
                    // the parent. Ordinary field validation and frontier replay
                    // check every move, selected cleanup and exact arrival;
                    // neither empty fields nor unrestricted contents are an
                    // extra requirement for the same transaction in a cycle.
                    OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishRecord { .. } => true,
                    _ => false,
                };
            lifetime_valid && match &operation.kind {
                OperationKind::EstablishScalarCase { .. } => {
                    crate::validation::scalar::case::fields(module, machine, operation).is_ok()
                }
                OperationKind::EstablishStructuralArray { .. } => {
                    crate::validation::structural_array::operands(module, machine, operation)
                        .is_ok()
                }
                OperationKind::EstablishRecord { .. } => {
                    crate::validation::record::fields(module, machine, operation).is_ok()
                }
                OperationKind::EstablishStructuralCase { .. } => {
                    crate::validation::structural::case::fields(module, machine, operation).is_ok()
                }
                OperationKind::EstablishScalarArray { .. } => {
                    crate::validation::scalar::array::shape(module, machine, operation).is_ok()
                }
                OperationKind::EstablishZeroedStructural { .. } => {
                    crate::validation::structural::operations::validate_establish_zeroed_structural(
                        module, machine, operation,
                    )
                    .is_ok()
                }
                OperationKind::EstablishByteSequenceConcatenation { .. } => {
                    crate::validation::byte_sequence::concatenation::validate(
                        module, machine, operation,
                    )
                    .is_ok()
                }
                _ => false,
            }
        }
        // A confined result — the same block's affine dispatch or return
        // source — keeps its per-traversal disposal roster, while an
        // unrestricted result spelling one of the frontier's plain-source
        // shapes is a copy payload: the place never enters `owned_places`,
        // so re-entering the producer on every traversal replaces a
        // custody-free value and no member-disposal obligation exists. The
        // place's dominance and the consuming reads' availability are still
        // proven by the ordinary operation walk, and a claimed, projected,
        // or non-plain result keeps the fence closed.
        OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. } => {
            operation.result.structural().is_some_and(|result| {
                case_source == Some(result.place)
                    || matches!(&block.terminator, Terminator::ReturnStructural { source, .. }
                                    if *source == result.place)
                    || (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && (crate::validation::scalar::case::plain_return_source(
                            module,
                            machine,
                            result.place,
                        ) || crate::validation::record::plain_return_source(
                            module,
                            machine,
                            result.place,
                        ) || crate::validation::scalar::array::plain_return_source(
                            module,
                            machine,
                            result.place,
                        )))
                    // A claim-free affine result stays confined when some
                    // in-cycle edge schedules its disposal: the place enters
                    // `owned_places` at the call, the frontier replay refuses
                    // an already-owned place on re-entry, and the fixed-point
                    // join proves disposal identical on every traversal — so
                    // a scheduled back-edge discard is the confinement, while
                    // a result no edge discards remains ineligible.
                    || (result.multiplicity == StructuralMultiplicity::Affine
                        && result.claims.is_empty()
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.qualification_establishments.is_empty()
                        && result_discarded_in_cycle(machine, cyclic_blocks, result.place))
                    // A reference-free result whose arguments are all
                    // call-scoped loans or copies may also run outside every
                    // cycle, or move into an owned successor parameter or a
                    // window hole before re-entry.
                    || owned_call_result(module, machine, block, cyclic_blocks, operation, result)
            })
        }
        // Ordinary scalar calls retain their complete signature,
        // requirement, and crash checks after this eligibility fence.
        OperationKind::Call { .. } => operation.result.scalar().is_some(),
        OperationKind::PortWrite { .. } => operation.result == OperationResult::Unit,
        OperationKind::EstablishPrimitiveLocal { .. } => {
            primitive_storage::validate_establishment(module, machine, operation).is_ok()
        }
        // A trivial affine local establishment is the direct spelling of
        // the empty-record arm's declaration: the destination is a
        // machine-declared whole affine place of empty-record type, so the
        // operation carries no field custody at all — its whole disposal
        // obligation is the place's own lifecycle, which the frontier
        // replay enforces exactly: the place enters `owned_places` at the
        // establishment, leaves it only through an edge's discard roster,
        // an owned-argument move, or a return's ordered cleanup, and the
        // fixed-point join demands identical custody on every arrival — so
        // a member-block establishment re-arms once per traversal while an
        // entry-block establishment can stay live for the whole cyclic
        // lifetime. The ordinary operand, liveness, and frontier checks
        // still run after eligibility.
        OperationKind::EstablishTrivialAffineLocal { .. } => operation.result
            == OperationResult::Unit
            && crate::validation::structural::operations::validate_establish_trivial_affine_local(
                module, machine, operation,
            )
            .is_ok(),
        OperationKind::PrimitiveScalarRead { source, path } => {
            operation.result.scalar().is_some_and(|result| {
                primitive_storage::read_type(module, machine, operation.id, *source, path)
                    == Ok(result.scalar_type)
            })
        }
        // A whole-root write through a borrowed primitive-scalar parameter
        // mutates caller-owned storage, like a byte-sequence write through
        // `&mut [u8]`: no result custody exists and the referent never joins
        // the machine's own frontier, so every arrival presents the identical
        // anchor. `store_type` retains the full destination checks — a
        // non-scalar or non-writable parameter still declines there — while a
        // locally-established scalar keeps the existing admission.
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            operation.result == OperationResult::Unit
                && (primitive_storage::local_result(machine, *destination).is_some()
                    || !path.is_empty()
                    || primitive_storage::parameter_type(machine, *destination, true).is_some())
                && primitive_storage::store_type(module, machine, operation.id, *destination, path)
                    .is_ok()
        }
        // An atomic event re-arms nothing: its custody is the root's own
        // access, exactly as for the primitive read and store above.
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
        } => crate::validation::atomic_access::leaf_type(
            module,
            machine,
            operation.id,
            *place,
            path,
            *field,
            *event,
        )
        .is_ok(),
        // Unit and scalar results publish no structural custody. Their
        // arguments therefore use the ordinary call transaction regardless
        // of producer, projection or return spelling: borrowed arguments
        // close at return; owned arguments consume their exact live places.
        // The ordinary call validator checks the callee, result signature,
        // paths, access, requirements and crash coverage. Reference replay
        // checks conflicting loans; frontier replay records moves and demands
        // the same custody on every arrival, including backedges. A consumed
        // affine input cannot be reused on the next iteration without a
        // replenishing transfer. Do not duplicate those checks with a roster
        // of allowed argument producers here. Claim movement remains fenced
        // independently until cyclic claim custody is supported.
        OperationKind::CallUnit {
            claim_transfers, ..
        }
        | OperationKind::CallStructuralScalar {
            claim_transfers, ..
        } => {
            matches!(
                operation.result,
                OperationResult::Unit | OperationResult::Scalar(_)
            ) && claim_transfers.is_empty()
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let boundary_parameters = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .map(|boundary| boundary.structural_parameters.as_slice())
                .unwrap_or(&[]);
            // A scalar result is an ordinary value, as an in-module call's
            // is: it carries no custody across the cycle.
            (operation.result == OperationResult::Unit
                || operation.result.scalar().is_some()
                || operation.result.structural().is_some_and(|result| {
                    case_source == Some(result.place)
                                    // An affine boundary result disposed on
                                    // every outgoing edge never becomes
                                    // loop-carried custody either.
                                    || (result.multiplicity == StructuralMultiplicity::Affine
                                        && trivially_discarded_on_all_edges(
                                            block,
                                            result.place,
                                        ))
                }))
                && completion_receipts.is_empty()
                && structural_arguments.len() == boundary_parameters.len()
                && structural_arguments.iter().zip(boundary_parameters).all(
                    |(argument, expected)| {
                        (argument.path.is_empty()
                            && argument.access == StructuralAccess::SharedBorrow
                            && (machine
                                .structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == argument.place)
                                || block_views::parameter(machine, argument.place).is_some()
                                || machine.structural_places.iter().any(|place| {
                                    place.id == argument.place
                                        && matches!(
                                            place.kind,
                                            StructuralPlaceKind::ByteSequenceLiteral { .. }
                                        )
                                })
                                || crate::validation::byte_sequence::subslice::borrowed_result_in_module(
                                    module, machine,
                                    argument.place,
                                )
                                .is_some()
                                || crate::validation::element_view::subslice::borrowed_result_in_module(
                                    module, machine,
                                    argument.place,
                                )
                                .is_some()))
                            // Like an internal call, a boundary may borrow a
                            // produced home for this call only. Ordinary
                            // argument checking retains type/access, producer
                            // dominance and live-storage authority; result
                            // disposal and completion receipts remain fenced
                            // above, independently of the source's spelling.
                            || (argument.path.is_empty()
                                && borrows_operation_result(machine, argument))
                            || byte_field_boundary_loan(module, machine, argument, expected)
                            // A borrowed projection loans a subtree for the
                            // boundary call's duration only — the provider
                            // cannot retain it past the operation, so the loan
                            // is created and released inside the call on every
                            // traversal and loop-carried custody is identical
                            // on each arrival. Reusing the unit-call arm's
                            // unrestricted subloan shapes keeps the root's
                            // authority checks and the positional pairing
                            // identical.
                            || (!argument.path.is_empty()
                                && (crate::validation::structural::operations::is_unrestricted_shared_subloan(
                                    module, machine, expected, argument,
                                ) || crate::validation::structural::operations::is_unrestricted_mutable_subloan(
                                    module, machine, expected, argument,
                                )))
                    },
                )
        }
        // A field-replacement window pair: `MoveStructuralField` vacates one
        // declared structural field beneath a borrowed-window root and hands
        // out the displaced subtree, and `StoreStructuralField` reseats that
        // hole. The static shapes prove the root is a claim-free
        // mutable-borrowed parameter or a completed owned home and that the
        // field's declared type matches the moved result and the repair
        // value. The frontier replay owns the window's custody on every
        // arrival: the hole rides `restoration_debt` in each snapshot, so an
        // arrival whose open windows differ from the block's established
        // entry — a hole left open across a back edge — fails the fixed-point
        // join, every non-crash exit rejects pending debt, and the repair
        // value must be live, claim-free and of the hole's type.
        //
        // The displaced value is the remaining obligation. An unrestricted
        // plain one is a copy payload, as for the unrestricted structural
        // case and scalar array arms: the frontier never retains it, and it
        // owes no disposal. An affine one enters `owned_places` at the move.
        // Outside every cycle the move executes once per activation, and the
        // exit rosters must dispose it exactly. Inside a cycle it must be
        // disposed before re-entry: some in-cycle edge schedules its discard,
        // or an in-cycle window store consumes it as a repair value, and the
        // fixed-point join proves that disposal identical on every traversal.
        // An affine move in a cycle that nothing there disposes keeps the
        // fence closed.
        //
        // A runtime-selected element's hole is its whole array prefix, so a
        // store at another runtime index would close it without proving the
        // same element was reseated. Vacating and reseating a copy payload
        // moves no custody either way; an affine element keeps the fence
        // closed until the window names its element exactly.
        OperationKind::MoveStructuralField { path, .. } => {
            crate::validation::borrowed_windows::validate_move_static(module, machine, operation)
                .is_ok()
                && operation.result.structural().is_some_and(|result| {
                    (result.multiplicity == StructuralMultiplicity::Unrestricted
                        && crate::validation::record::completed_source(
                            module,
                            machine,
                            result.place,
                        )
                        .is_some())
                        || (result.multiplicity == StructuralMultiplicity::Affine
                            && terminal_psi::is_static_structural_path(path)
                            && (!cyclic_blocks.contains(&block.id)
                                || result_discarded_in_cycle(machine, cyclic_blocks, result.place)
                                || reseated_in_cycle(machine, cyclic_blocks, result.place)))
                })
        }
        OperationKind::StoreStructuralField { .. } => {
            operation.result == OperationResult::Unit
                && crate::validation::borrowed_windows::validate_store_static(
                    module, machine, operation,
                )
                .is_ok()
        }
        // An unrestricted plain leaf copy observes its owner without
        // consuming or refining it, the structural sibling of the primitive
        // read below: the frontier never retains the fresh copy payload as a
        // disposal obligation, so re-entering the copy each traversal
        // replaces a custody-free value. The source's liveness, the copy's
        // producer dominance, and a runtime index's bounds obligation are
        // still checked by the ordinary walk. A shared-borrow relocation is a
        // live reference carrier and keeps the fence closed.
        OperationKind::StructuralLeafCopy { source, path } => {
            operation.result.structural().is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && (crate::validation::record::completed_source(module, machine, result.place)
                        .is_some()
                        || crate::validation::record::shared_view_member_source(
                            module,
                            machine,
                            result.place,
                        )
                        .is_some())
            }) && crate::validation::structural::leaf_copy::validate(
                module, machine, operation, *source, path,
            )
            .is_ok()
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            operation.result.structural().is_some_and(|result| {
                crate::validation::byte_sequence::subslice::borrowed_result_in_module(
                    module,
                    machine,
                    result.place,
                ) == Some(result)
            })
        }
        OperationKind::EstablishElementView { .. } | OperationKind::ElementViewSubslice { .. } => {
            operation.result.structural().is_some_and(|result| {
                crate::validation::element_view::subslice::borrowed_result_in_module(
                    module,
                    machine,
                    result.place,
                ) == Some(result)
            })
        }
        OperationKind::EstablishMutableElementView { .. } => {
            operation.result.structural().is_some_and(|result| {
                crate::validation::element_view::write::mutable_result(
                    module,
                    machine,
                    result.place,
                ) == Some(result)
            })
        }
        // Content predicates observe the current bytes and produce a scalar;
        // they retain no custody on a backedge. Ordinary operand validation
        // still requires readable backing and its current dominating length.
        OperationKind::ByteSequencePredicate { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ElementViewLength { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldRead { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ElementViewRead { .. }
        | OperationKind::IntegerStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::BooleanStructuralField { .. } => operation.result.scalar().is_some(),
        OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::ElementViewWrite { .. }
        | OperationKind::ByteSequenceStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteViewFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. } => {
            operation.result == OperationResult::Unit
        }
        kind => operation.result.scalar().is_some() && pure_scalar(kind),
    }
}

/// The owned sum never becomes loop-carried custody. The same block either
/// establishes it or receives it as its own owned parameter, then inspects
/// it, and every selected edge disposes that exact whole root. The parameter
/// form is the dispatch hop `enum_step_kind(step)`: the predecessor moves a
/// fresh call result into the parameter, and the case edges retire it before
/// any successor runs. Ordinary operation formation, dominance and frontier
/// replay remain required.
fn confined_case_source(
    module: &TerminalModule,
    block: &terminal_psi::Block,
) -> Option<semantic_vocabulary::PlaceId> {
    let Terminator::StructuralCase { source, cases } = &block.terminator else {
        return None;
    };
    if cases.is_empty()
        || cases
            .iter()
            .any(|case| case.trivial_affine_discards != [*source])
    {
        return None;
    }
    let produced = block.operations.iter().find_map(|operation| {
        matches!(
            operation.kind,
            OperationKind::BoundaryCall { .. }
                | OperationKind::EstablishScalarCase { .. }
                | OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
        )
        .then(|| operation.result.structural())
        .flatten()
        .filter(|result| result.place == *source)
    });
    let structural_type = match produced {
        Some(result) => (result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
        .then_some(result.structural_type)?,
        None => {
            block
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *source)
                .filter(|parameter| {
                    !parameter.is_self
                        && parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Affine
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                })?
                .structural_type
        }
    };
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    cases
        .iter()
        .all(|case| {
            case.fields.iter().all(|field| {
                !field.relevance.is_erased()
                    && matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Scalar(_)
                            | terminal_psi::StructuralFieldType::BoundedInteger(_)
                    )
            })
        })
        .then_some(*source)
}

fn plain_owned(parameter: &StructuralParameterDeclaration) -> bool {
    !parameter.is_self
        && parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

/// A claim-free affine structural call result is an owned value, like the
/// boundary case results the scalar-value arm admits: the place enters
/// `owned_places` at the call, and the frontier replay then proves its custody
/// on every arrival. The result insert refuses a place that is still owned,
/// every arrival at a block must present that block's established entry
/// frontier, and every exit must dispose the exact owned roster. What the
/// replay cannot see is whether the operation is re-entered, so the disposal
/// clause depends on where the call runs. Outside every cycle it runs once per
/// activation and the exit rosters or a consumer dispose of it. Inside a cycle
/// some in-cycle edge must dispose of it before the back edge: a discard
/// roster, a whole move into an owned successor block parameter (the
/// parameter then carries the obligation, as any loop-carried parameter
/// does), or a window store that reseats a hole with it.
///
/// The call's borrowed arguments must close when the call returns. The
/// frontier records a loan that outlives a call only as a live reference
/// rooted in the argument, and only for a result type that contains a
/// reference; with a reference-free result and no claim transfers in either
/// direction, every `&` or `&mut` argument, whole or projected, is lent for
/// the call alone and no loan can cross a back edge. Ordinary call
/// validation still checks each loan's authority and overlap. An owned
/// argument must be a plain owned parameter or a copied payload, so the call
/// moves no other custody inside the cycle.
fn owned_call_result(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    cyclic_blocks: &BTreeSet<BlockId>,
    operation: &terminal_psi::Operation,
    result: &terminal_psi::StructuralOperationResult,
) -> bool {
    let (OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        ..
    }
    | OperationKind::CallStructuralWithScalarArguments {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        ..
    }) = &operation.kind
    else {
        return false;
    };
    let Some(callee) = module
        .machines
        .iter()
        .find(|candidate| candidate.id == *callee)
    else {
        return false;
    };
    result.multiplicity == StructuralMultiplicity::Affine
        && result.claims.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && !crate::validation::references::contains_reference(module, result.structural_type)
        && structural_arguments.len() == callee.structural_parameters.len()
        && structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .all(|(argument, parameter)| {
                argument.access != StructuralAccess::Owned
                    || owned_argument(module, machine, argument)
                    || copied_payload_argument(module, machine, argument, parameter)
            })
        && result_disposed_before_reentry(machine, block, cyclic_blocks, result.place)
}

/// Borrowing a place an earlier operation in the same machine produced lends
/// it for the call only; the loan releases on return, so loop-carried custody
/// is identical on every traversal. The frontier's owned-read check still
/// requires the frame to own an affine result at the loan.
fn borrows_operation_result(machine: &TerminalMachine, argument: &StructuralArgument) -> bool {
    argument.access != StructuralAccess::Owned
        && machine.structural_places.iter().any(|place| {
            place.id == argument.place
                && matches!(place.kind, StructuralPlaceKind::OperationResult { .. })
        })
}

/// A whole owned argument bound to an unrestricted owned parameter copies an
/// unrestricted plain payload, such as `MemoryAlignment::Alignment4`. The
/// frontier never retains such a source, and it consumes nothing for an
/// unrestricted parameter, so the call moves no custody on any traversal.
/// These are the same plain-source shapes the frontier's result insert
/// classifies as copy payloads; the source's availability is still proven by
/// the ordinary dominance walk.
fn copied_payload_argument(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    argument.access == StructuralAccess::Owned
        && argument.path.is_empty()
        && parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && crate::validation::structural::result_contracts::source_signature(
            machine,
            argument.place,
        )
        .is_some_and(|source| source.multiplicity == StructuralMultiplicity::Unrestricted)
        && (crate::validation::scalar::case::plain_return_source(module, machine, argument.place)
            || crate::validation::structural::case::plain_return_source(
                module,
                machine,
                argument.place,
            )
            || crate::validation::record::plain_return_source(module, machine, argument.place)
            || crate::validation::scalar::array::plain_return_source(
                module,
                machine,
                argument.place,
            ))
}

fn owned_argument(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    argument.access == StructuralAccess::Owned
        && argument.path.is_empty()
        && (machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .or_else(|| block_views::parameter(machine, argument.place))
            .is_some_and(plain_owned)
            // A whole member-produced scalar array is copied into the
            // callee on each traversal: its producer independently passes
            // this fence through the `EstablishScalarArray` arm, the
            // unrestricted result never becomes loop-carried custody, and
            // the argument's availability is still proven by the ordinary
            // dominance walk. Other produced kinds keep the parameter-only
            // rule until their cyclic custody is spelled out.
            || machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, OperationKind::EstablishScalarArray { .. })
                        && operation.result.structural().is_some_and(|result| {
                            result.place == argument.place
                                && result.multiplicity == StructuralMultiplicity::Unrestricted
                                && result.qualifications.is_empty()
                                && result.projected_qualifications.is_empty()
                                && result.claims.is_empty()
                        })
                        && crate::validation::scalar::array::shape(module, machine, operation).is_ok()
                }))
}

/// A boundary's borrowed byte view may loan one initialized inline field under
/// the same unrestricted subloan shape the ordinary argument validator admits.
/// The operand keeps its owning root and path; the loan grants no new storage,
/// extent replacement, or custody transfer inside the cycle.
fn byte_field_boundary_loan(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> bool {
    let Some(actual) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    else {
        return false;
    };
    if crate::validation::structural::operations::is_unrestricted_shared_subloan(
        module, machine, expected, argument,
    ) {
        return terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            actual.structural_type,
            argument,
            expected,
        )
        .is_some()
            || terminal_semantics::fixed_byte_array_extent(
                module.structural_types.iter(),
                actual,
                argument,
                expected,
            )
            .is_some();
    }
    let mutable_subloan =
        crate::validation::structural::operations::is_unrestricted_mutable_subloan(
            module, machine, expected, argument,
        );
    if mutable_subloan {
        return terminal_semantics::boundary_buffer_capacity(
            module.structural_types.iter(),
            actual.structural_type,
            argument,
            expected,
        )
        .is_some()
            || terminal_semantics::fixed_byte_array_extent(
                module.structural_types.iter(),
                actual,
                argument,
                expected,
            )
            .is_some();
    }
    false
}

/// Whether every outgoing edge of `block` commits an exact no-code affine
/// discard of `place`, so a produced value cannot cross the cycle backedge.
fn trivially_discarded_on_all_edges(block: &terminal_psi::Block, place: PlaceId) -> bool {
    match &block.terminator {
        Terminator::Jump {
            trivial_affine_discards,
            ..
        } => trivial_affine_discards.contains(&place),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .iter()
            .all(|edge| edge.trivial_affine_discards.contains(&place)),
        Terminator::StructuralCase { cases, .. } => {
            !cases.is_empty()
                && cases
                    .iter()
                    .all(|case| case.trivial_affine_discards.contains(&place))
        }
        _ => false,
    }
}

fn persistent_receiver(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    parameter.is_self
        && matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
}

/// Keep the admitted operation family explicit: scalar result shape alone
/// cannot admit calls, structural observations, or future effectful operations.
/// Scalar operations that move no custody. A `TrappingInteger` operation
/// owns its trap site, which crash-contract validation covers like the
/// guarded `Crash` terminator it replaced; its arithmetic re-arms nothing.
fn pure_scalar(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::IntegerConstant { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::IeeeFloatCompare { .. }
            | OperationKind::BooleanNot { .. }
            | OperationKind::BooleanEqual { .. }
            | OperationKind::IntegerEqual { .. }
            | OperationKind::IntegerLessThan { .. }
            | OperationKind::IntegerLessOrEqual { .. }
            | OperationKind::IntegerBitwiseNot { .. }
            | OperationKind::IntegerWiden { .. }
            | OperationKind::IntegerExactCast { .. }
            | OperationKind::IntegerBitwiseAnd { .. }
            | OperationKind::IntegerBitwiseOr { .. }
            | OperationKind::IntegerBitwiseXor { .. }
            | OperationKind::WrappingIntegerShiftLeft { .. }
            | OperationKind::WrappingIntegerShiftRight { .. }
            | OperationKind::ExactIntegerShiftLeft { .. }
            | OperationKind::ExactIntegerShiftRight { .. }
            | OperationKind::ExactIntegerAdd { .. }
            | OperationKind::ExactIntegerSubtract { .. }
            | OperationKind::ExactIntegerMultiply { .. }
            | OperationKind::ExactIntegerDivide { .. }
            | OperationKind::ExactIntegerRemainder { .. }
            | OperationKind::WrappingIntegerDivide { .. }
            | OperationKind::WrappingIntegerRemainder { .. }
            | OperationKind::SaturatingIntegerDivide { .. }
            | OperationKind::SaturatingIntegerRemainder { .. }
            | OperationKind::WrappingIntegerAdd { .. }
            | OperationKind::SaturatingIntegerAdd { .. }
            | OperationKind::WrappingIntegerSubtract { .. }
            | OperationKind::SaturatingIntegerSubtract { .. }
            | OperationKind::WrappingIntegerMultiply { .. }
            | OperationKind::SaturatingIntegerMultiply { .. }
            | OperationKind::TrappingInteger { .. }
            | OperationKind::FloatResultPolicy { .. }
    )
}
