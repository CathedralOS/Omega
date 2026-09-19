//! Eligibility for cyclic scalar work, owned inputs, locals, views, receivers,
//! unrestricted record establishments, affine empty-record establishments, and
//! entry claims pinned on owned machine parameters for the machine's whole
//! cyclic lifetime.

use super::super::{
    BTreeSet, ClaimId, OperationResult, PlaceId, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceKind, TerminalAffineCleanupAction,
};
use super::super::{block_views, byte_sequence_subslice, primitive_storage, scalar_array};
use super::{
    ModuleError, OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator,
};

/// Eligibility carries no proof or dominance authority. The caller runs the
/// ordinary operand, view, successor, and frontier checks after this fence.
pub(super) fn eligible(module: &TerminalModule, machine: &TerminalMachine) -> bool {
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
            && super::super::scalar_case::plain_type(module, result.structural_type)
    });
    if (machine.result.structural().is_some() && !scalar_case_result)
        || !claims_pinned_at_entry(machine, &claim_roots)
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
        || machine.contract.requires.iter().any(|requirement| {
            super::super::proposition_observes_places(
                requirement,
                &machine
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::MutableBorrow)
                    .map(|parameter| parameter.place)
                    .collect::<Vec<_>>(),
            )
        })
        || machine.structural_parameters.iter().any(|parameter| {
            !plain_owned(parameter)
                && !(parameter.access == StructuralAccess::Owned
                    && claim_roots.contains(&parameter.place))
                && (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !(persistent_receiver(module, parameter)
                        || ((parameter.access == StructuralAccess::SharedBorrow
                            || (parameter.access == StructuralAccess::MutableBorrow
                                && (machine.result == TerminalMachineResult::Unit
                                    || scalar_case_result)))
                            && module.structural_types.iter().any(|declaration| {
                                declaration.id == parameter.structural_type
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView
                                        )
                                    )
                            }))))
        })
        || machine.structural_places.iter().any(|place| {
            !(matches!(
                place.kind,
                StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::OperationResult { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
            ) || scalar_case_result && place.kind == StructuralPlaceKind::Result)
        })
    {
        return false;
    }
    machine.blocks.iter().all(|block| {
        let case_source = local_case_result(module, block);
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
                || scalar_case_result && matches!(&block.terminator,
                    Terminator::ReturnStructural { source, returned_claims, .. }
                    if returned_claims.is_empty()
                        && super::super::scalar_case::plain_return_source(module, machine, *source));
        let operations_eligible = block
            .operations
            .iter()
            .all(|operation| {
                cycle_operation_eligible(module, machine, block, case_source, operation)
            });
        terminator_eligible && operations_eligible
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
        OperationKind::ReleaseReference { source }
        | OperationKind::PrimitiveScalarRead { source, .. }
        | OperationKind::StructuralByteSequenceFieldLength { source, .. }
        | OperationKind::StructuralCaseMembership { source, .. }
        | OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. }
        | OperationKind::ByteSequenceSubslice { source, .. }
        | OperationKind::BooleanStructuralField { source, .. }
        | OperationKind::IntegerStructuralField { source, .. } => clears(*source),
        OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
        | OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::WriteOnlyIndexedPrimitiveStore { destination, .. }
        | OperationKind::StructuralScalarFieldStore { destination, .. }
        | OperationKind::EstablishByteSequenceLiteral { destination, .. }
        | OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::EstablishTrivialAffineLocal { destination } => clears(*destination),
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        } => clears(*destination) && clears(*source),
        OperationKind::EstablishRecord { fields } => {
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

fn cycle_operation_eligible(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    case_source: Option<semantic_vocabulary::PlaceId>,
    operation: &terminal_psi::Operation,
) -> bool {
    match &operation.kind {
        OperationKind::EstablishScalarCase { .. } => {
            super::super::scalar_case::fields(module, machine, operation).is_ok()
                && operation.result.structural().is_some_and(|result| {
                    matches!(&block.terminator, Terminator::ReturnStructural { source, .. }
                                    if *source == result.place)
                        || case_source == Some(result.place)
                })
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
                        && (super::super::scalar_case::plain_return_source(
                            module,
                            machine,
                            result.place,
                        ) || super::super::record::plain_return_source(
                            module,
                            machine,
                            result.place,
                        ) || scalar_array::plain_return_source(module, machine, result.place)))
            })
        }
        // Ordinary scalar calls retain their complete signature,
        // requirement, and crash checks after this eligibility fence.
        OperationKind::Call { .. } => operation.result.scalar().is_some(),
        OperationKind::PortWrite { .. } => operation.result == OperationResult::Unit,
        OperationKind::EstablishPrimitiveLocal { .. } => {
            primitive_storage::validate_establishment(module, machine, operation).is_ok()
        }
        // A complete unrestricted record establishment is pure structural
        // construction: `record::fields` proves the fresh `OperationResult`
        // place, the declaration-paired field initializers, and a claim-free
        // result, while the unrestricted result never carries a per-iteration
        // disposal obligation — scalar fields read values and structural
        // fields copy unrestricted sources, so re-establishing the same place
        // each iteration moves no custody. An empty-field affine
        // establishment is the composed-control spelling of a trivial affine
        // local: it carries no field custody, and its only disposal
        // obligation is the affine result place. The frontier replay
        // enforces that lifecycle exactly — the place enters `owned_places`
        // at the establishment, leaves it only through an edge's discard
        // roster, an owned-argument move, or a return's ordered cleanup, the
        // result insert refuses to produce an already-owned place, and the
        // fixed-point join demands identical custody on every arrival — so a
        // member-block establishment re-arms once per traversal while an
        // entry-block establishment can stay live for the whole cyclic
        // lifetime. An affine record with fields and any claim-bearing
        // result keep the fence closed; the ordinary operand, liveness, and
        // frontier checks still run after eligibility.
        OperationKind::EstablishRecord { fields } => {
            operation.result.structural().is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    || (result.multiplicity == StructuralMultiplicity::Affine && fields.is_empty())
            }) && super::super::record::fields(module, machine, operation).is_ok()
        }
        // A complete unrestricted scalar-array establishment is the record
        // arm's primitive-leaf sibling: `scalar_array::shape` proves the fresh
        // `OperationResult` place, the element count against the declared
        // leaf shape, and a claim-free result, while the unrestricted payload
        // never carries a per-iteration disposal obligation. Re-entering the
        // establishment hands the place a fresh payload each traversal — the
        // interpreter replaces the stored elements, and no custody moves.
        // The ordinary operand, availability, and frontier checks still run
        // after this fence.
        OperationKind::EstablishScalarArray { .. } => {
            super::super::scalar_array::shape(module, machine, operation).is_ok()
        }
        OperationKind::PrimitiveScalarRead { source, path } => {
            operation.result.scalar().is_some_and(|result| {
                primitive_storage::read_type(module, machine, operation.id, *source, path)
                    == Ok(result.scalar_type)
            })
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            operation.result == OperationResult::Unit
                && (primitive_storage::local_result(machine, *destination).is_some()
                    || !path.is_empty())
                && primitive_storage::store_type(module, machine, operation.id, *destination, path)
                    .is_ok()
        }
        OperationKind::WriteOnlyIndexedPrimitiveStore {
            destination, path, ..
        } => {
            operation.result == OperationResult::Unit
                && (primitive_storage::local_result(machine, *destination).is_some()
                    || !path.is_empty())
                && primitive_storage::indexed_store_shape(
                    module,
                    machine,
                    operation.id,
                    *destination,
                    path,
                )
                .is_ok()
        }
        // Requirement obligations and crash continuations carry no custody:
        // ordinary call validation still checks their arity, substitution and
        // caller coverage symbolically, and obligation reconstruction cuts
        // feedback edges rather than enumerating iterations. Claim transfers
        // stay fenced until cyclic machines admit claim-bearing custody.
        OperationKind::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => {
            operation.result.scalar().is_some()
                && !structural_arguments.is_empty()
                && structural_arguments.iter().all(|argument| {
                    argument.path.is_empty()
                        && ((argument.access != StructuralAccess::Owned
                            && primitive_storage::local_result(machine, argument.place).is_some())
                            || owned_argument(module, machine, argument))
                })
                && claim_transfers.is_empty()
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
            (operation.result == OperationResult::Unit
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
                                || byte_sequence_subslice::borrowed_result(
                                    machine,
                                    argument.place,
                                )
                                .is_some()))
                            || byte_field_boundary_loan(module, machine, argument, expected)
                    },
                )
        }
        OperationKind::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } => {
            operation.result == OperationResult::Unit
                && structural_arguments.iter().all(|argument| {
                    argument.path.is_empty()
                        && ((argument.access == StructuralAccess::MutableBorrow
                            && machine.structural_parameters.iter().any(|parameter| {
                                parameter.place == argument.place
                                    && persistent_receiver(module, parameter)
                            }))
                            || (argument.access != StructuralAccess::Owned
                                && primitive_storage::local_result(machine, argument.place)
                                    .is_some())
                            || (argument.access == StructuralAccess::SharedBorrow
                                && super::super::byte_sequence_length::validate_source(
                                    module,
                                    machine,
                                    operation,
                                    argument.place,
                                    || ModuleError::InvalidByteSequenceLengthSource {
                                        operation: operation.id,
                                        source: argument.place,
                                    },
                                )
                                .is_ok())
                            || owned_argument(module, machine, argument))
                })
                && claim_transfers.is_empty()
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            operation.result.structural().is_some_and(|result| {
                byte_sequence_subslice::borrowed_result(machine, result.place) == Some(result)
            })
        }
        OperationKind::ByteSequenceLength { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::IntegerStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::BooleanStructuralField { .. } => operation.result.scalar().is_some(),
        OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. } => {
            operation.result == OperationResult::Unit
        }
        kind => operation.result.scalar().is_some() && pure_scalar(kind),
    }
}

/// The owned sum never becomes loop-carried custody. The same block establishes
/// and inspects it, and every selected edge disposes that exact whole root.
/// Ordinary operation formation, dominance and frontier replay remain required.
fn local_case_result(
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
    let result = block.operations.iter().find_map(|operation| {
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
    })?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return None;
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?;
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
                        && scalar_array::shape(module, machine, operation).is_ok()
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
    if super::super::structural_operations::is_unrestricted_shared_subloan(
        machine, expected, argument,
    ) {
        return terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            actual.structural_type,
            argument,
            expected,
        )
        .is_some();
    }
    let mutable_subloan = super::super::structural_operations::is_unrestricted_mutable_subloan(
        machine, expected, argument,
    );
    if mutable_subloan {
        return terminal_semantics::boundary_buffer_capacity(
            module.structural_types.iter(),
            actual.structural_type,
            argument,
            expected,
        )
        .is_some()
            || terminal_semantics::mutable_fixed_byte_array_extent(
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
        && parameter.access == StructuralAccess::MutableBorrow
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
    )
}
