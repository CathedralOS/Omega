//! One structural argument checked against the parameter it supplies.

use super::{
    CallShape, StructuralArgumentSourcePolicy, is_runtime_indexed_borrow_path,
    is_structural_call_result, is_unrestricted_mutable_subloan, is_unrestricted_shared_subloan,
    is_unrestricted_write_only_subloan, linear_call_result, structural_access_can_supply,
    structural_occurrence_carries_qualification,
};
use crate::validation::structural::operations::claim_transfers::is_record_loan;
use crate::validation::{
    ModuleError, OperationId, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceKind, TerminalMachine, TerminalModule, is_nonempty_field_path,
    is_partial_affine_path, partial_affine_root_type,
};

/// One structural argument against its parameter: a reference projection
/// must match an exact primitive parameter; otherwise the argument's source
/// resolves to a known place whose type, access, multiplicity and
/// qualifications the parameter accepts, with the presentations the call
/// kind and source policy allow.
pub(in crate::validation) fn validate_structural_argument(
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: OperationId,
    allow_projected: bool,
    source_policy: StructuralArgumentSourcePolicy,
    shape: &CallShape<'_>,
    index: usize,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Result<(), ModuleError> {
    let CallShape {
        call_kind,
        borrowed_call,
        ordinary_call,
        result_projection,
        reference_result_call,
    } = *shape;
    let boundary_byte_view_call = source_policy
        == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
        && matches!(call_kind, Some(OperationKind::BoundaryCall { .. }))
        && module.structural_types.iter().any(|row| {
            row.id == expected.structural_type
                && matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow)
                        }
                    )
                )
        });
    if argument.path.contains(&StructuralPathSegment::Referent) {
        if matches!(call_kind, Some(OperationKind::BoundaryCall { .. })) {
            return Err(crate::validation::references::invalid(
                caller,
                "reference projections at host boundaries are not yet supported",
            ));
        }
        if crate::validation::references::call_source_type(
            module,
            caller,
            argument,
            ordinary_call && !reference_result_call,
        ) != Some(expected.structural_type)
            || argument.access != expected.access
            || expected.multiplicity != StructuralMultiplicity::Unrestricted
            || !expected.qualifications.is_empty()
            || !expected.projected_qualifications.is_empty()
        {
            return Err(crate::validation::references::invalid(
                caller,
                "reference argument does not match its exact parameter",
            ));
        }
        return Ok(());
    }
    let Some((
        actual_type,
        actual_multiplicity,
        actual_access,
        actual_qualifications,
        actual_projected_qualifications,
    )) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| {
            (
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access,
                parameter.qualifications.as_slice(),
                parameter.projected_qualifications.as_slice(),
            )
        })
        .or_else(|| {
            caller.structural_places.iter().find_map(|place| {
                if place.id != argument.place {
                    return None;
                }
                match place.kind {
                    StructuralPlaceKind::OperationResult { .. }
                        if ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                            && argument.path.is_empty()
                            && crate::validation::references::argument_owns_references(module, caller, argument) =>
                    {
                        // Stored-reference carriers are not plain payloads.
                        // Their exact producer and live leaf transfer are
                        // checked independently by reference/frontier replay.
                        crate::validation::structural::result_contracts::source_signature(caller, argument.place).map(|source| (
                            source.structural_type, source.multiplicity, StructuralAccess::Owned,
                            source.qualifications, source.projected_qualifications,
                        ))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if matches!(
                            source_policy,
                            StructuralArgumentSourcePolicy::ParametersOrLinearCallResults
                                | StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                        ) && argument.path.is_empty() && argument.access == StructuralAccess::Owned
                            && linear_call_result(caller, argument.place).is_some() =>
                    {
                        // The guard keeps claim-free affine call results on the
                        // general arm below: selecting this arm for them would
                        // end the place lookup on a missing linear result.
                        let (_, result) = linear_call_result(caller, argument.place)
                            .expect("guard established a linear call result");
                        Some((result.structural_type, result.multiplicity, StructuralAccess::Owned,
                            result.qualifications.as_slice(), result.projected_qualifications.as_slice()))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                            && argument.path.is_empty()
                            && argument.access == StructuralAccess::Owned
                            && linear_call_result(caller, argument.place).is_some() =>
                    {
                        // A completed linear call result carries its claim
                        // frontier on the result itself. Admitting the place
                        // admits no claim: the call's transfer roster still
                        // resolves each occurrence through the caller's claim
                        // table — a result-minted binding or an entry claim —
                        // against the callee's entry roster.
                        let (_, result) = linear_call_result(caller, argument.place)
                            .expect("guard established a linear call result");
                        Some((result.structural_type, result.multiplicity, StructuralAccess::Owned,
                            result.qualifications.as_slice(), result.projected_qualifications.as_slice()))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if !matches!(source_policy, StructuralArgumentSourcePolicy::OnlyParameters
                            | StructuralArgumentSourcePolicy::ParametersOrLinearCallResults)
                            && (source_policy != StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                || argument.access != StructuralAccess::Owned)
                            && (argument.path.is_empty() || argument.access != StructuralAccess::Owned)
                            && (crate::validation::record::completed_source(module, caller, argument.place).is_some()
                                || (argument.access != StructuralAccess::Owned
                                    && crate::validation::record::loan_source(module, caller, argument.place).is_some())) =>
                    {
                        // Call-scoped loans retain the same completed backing
                        // at ordinary and boundary calls. The producer checks
                        // establishment; record/frontier replay checks its
                        // availability here, and the common argument checks
                        // below preserve projection, access and qualification.
                        // This does not admit owned boundary transfers.
                        let result = if argument.access == StructuralAccess::Owned {
                            crate::validation::record::completed_source(module, caller, argument.place)?
                        } else {
                            crate::validation::record::loan_source(module, caller, argument.place)?
                        };
                        Some((result.structural_type, result.multiplicity, StructuralAccess::Owned, &[][..], &[][..]))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                            && argument.path.is_empty()
                            && matches!(argument.access, StructuralAccess::Owned | StructuralAccess::SharedBorrow)
                            && (crate::validation::scalar::case::plain_return_source(module, caller, argument.place) || crate::validation::structural::case::plain_return_source(module, caller, argument.place)) =>
                    {
                        // Scalar and nested-payload case establishment supply the
                        // same complete owned value at calls as at returns. Keep its actual
                        // multiplicity; frontier replay still requires the
                        // exact producer to dominate and transfer only once.
                        crate::validation::structural::result_contracts::source_signature(caller, argument.place).map(|source| (
                            source.structural_type, source.multiplicity, StructuralAccess::Owned,
                            source.qualifications, source.projected_qualifications,
                        ))
                    }
                    StructuralPlaceKind::OperationResult { structural_type, .. }
                        if ((ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)
                            || (matches!(call_kind, Some(OperationKind::BoundaryCall { .. }))
                                && source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals))
                            && (argument.path.is_empty() || argument.access != StructuralAccess::Owned)
                            && matches!(argument.access, StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
                            && crate::validation::scalar::array::plain_return_source(module, caller, argument.place) =>
                    {
                        Some((structural_type, StructuralMultiplicity::Unrestricted,
                            StructuralAccess::Owned, &[][..], &[][..]))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                            && argument.path.is_empty()
                            && argument.access != StructuralAccess::Owned
                            && crate::validation::primitive_storage::local_result(caller, argument.place).is_some() =>
                    {
                        let result = crate::validation::primitive_storage::local_result(caller, argument.place)?;
                        crate::validation::primitive_storage::scalar_type(module, result.structural_type)?;
                        Some((result.structural_type, StructuralMultiplicity::Unrestricted,
                            StructuralAccess::Owned, &[][..], &[][..]))
                    }
                    StructuralPlaceKind::BlockParameter { .. }
                        if (argument.path.is_empty()
                            // An owned state arrival retains the same finite
                            // affine projection contract as an entry parameter.
                            // Its remaining siblings stay in the live frontier;
                            // this source lookup grants no disposal or claims.
                            || (allow_projected && ordinary_call
                                && argument.access == StructuralAccess::Owned
                                && partial_affine_root_type(caller, argument.place)
                                    .is_some_and(|root| is_partial_affine_path(module, root, &argument.path)))
                            || (allow_projected && ordinary_call
                                && argument.access == StructuralAccess::SharedBorrow
                                && is_nonempty_field_path(&argument.path)
                                && is_record_loan(module, caller, expected, argument))
                            // Rebinding a loan at a state edge does not remove
                            // its ability to lend a subtree. Resolve the same
                            // subloan contract as for a machine parameter;
                            // projected type, access, availability and overlap
                            // remain independently checked below and by the
                            // block-view and argument-pair replays.
                            || (allow_projected
                                && (is_unrestricted_shared_subloan(module, caller, expected, argument)
                                    || is_unrestricted_mutable_subloan(module, caller, expected, argument)
                                    || is_unrestricted_write_only_subloan(module, caller, expected, argument)))
                            // A runtime-indexed borrow of a block parameter is
                            // the same per-element loan the machine-parameter
                            // arm admits: the path predicate keeps Referent and
                            // byte ranges out, and the argument checks replay
                            // the selector's published bound.
                            || (allow_projected
                                && matches!(
                                    argument.access,
                                    StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                )
                                && is_runtime_indexed_borrow_path(&argument.path)))
                            // A state arrival retains its declared authority;
                            // it is not a separate kind of call operand. The
                            // common access check below rejects widening, and
                            // availability and argument-pair replay retain loan
                            // lifetime and exclusivity. Opaque boundaries still
                            // cannot receive owned or write-only authority.
                            && (ordinary_call
                                || (matches!(call_kind, Some(OperationKind::BoundaryCall { .. }))
                                    && matches!(argument.access,
                                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)))
                            && (source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                || (ordinary_call && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)) =>
                    {
                        crate::validation::block_views::parameter(caller, argument.place).map(|parameter| (
                            parameter.structural_type, parameter.multiplicity, parameter.access,
                            parameter.qualifications.as_slice(), parameter.projected_qualifications.as_slice(),
                        ))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if ((ordinary_call
                                && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                                && matches!(argument.access,
                                    StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow))
                            || (source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                && argument.path.is_empty()
                                && matches!(argument.access,
                                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)))
                            && crate::validation::element_view::view_loan_result_in_module(
                                module, caller,
                                argument.place,
                            )
                            .is_some() =>
                    {
                        // A live view lends its descriptor or a projected element.
                        // Recover the producer's actual authority for both: affine
                        // carrier custody alone cannot amplify a shared source.
                        // Availability and loan interference replay remain separate.
                        let (result, access) = crate::validation::element_view::view_loan_result_in_module(
                            module, caller,
                            argument.place,
                        )
                        .expect("view result checked above");
                        Some((result.structural_type, result.multiplicity, access,
                            result.qualifications.as_slice(), result.projected_qualifications.as_slice()))
                    }
                    StructuralPlaceKind::OperationResult { .. }
                        if ((ordinary_call
                            && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)
                            || boundary_byte_view_call)
                            && argument.path.is_empty()
                            && (argument.access == StructuralAccess::SharedBorrow
                                || (ordinary_call && argument.access == StructuralAccess::Owned))
                            && terminal_semantics::byte_owned_result_predicates(module, caller, argument.place).is_some() =>
                    {
                        // The completed bounded owner keeps its exact storage
                        // signature whether it was constructed or returned.
                        // Its operation validates establishment; dominance and
                        // live-loan replay validate this use. Lending a shared
                        // view does not erase the owner's declared predicates.
                        crate::validation::structural::result_contracts::source_signature(caller, argument.place)
                            .map(|source| (source.structural_type, source.multiplicity,
                                StructuralAccess::Owned, source.qualifications,
                                source.projected_qualifications))
                    }
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if argument.path.is_empty()
                        && argument.access == StructuralAccess::SharedBorrow
                        && (source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                            || (ordinary_call && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)) =>
                    {
                        Some((
                            structural_type,
                            StructuralMultiplicity::Unrestricted,
                            StructuralAccess::SharedBorrow,
                            super::literal_qualifications(caller, argument.place)
                                .unwrap_or(&[][..]),
                            &[][..],
                        ))
                    }
                    StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    } if source_policy
                        == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                        && argument.path.is_empty() =>
                    {
                        Some((
                            structural_type,
                            StructuralMultiplicity::Affine,
                            StructuralAccess::Owned,
                            &[][..],
                            &[][..],
                        ))
                    }
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if matches!(
                        source_policy,
                        StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                            | StructuralArgumentSourcePolicy::ParametersOrAffineOperationResults
                            | StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                    )
                        && (argument.path.is_empty()
                            || (result_projection && argument.access == StructuralAccess::Owned
                                && partial_affine_root_type(caller, argument.place) == Some(structural_type)
                                && is_partial_affine_path(module, structural_type, &argument.path)))
                        && {
                            let producer_result = caller
                                .blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .find(|operation| operation.id == producer)
                                .and_then(|operation| {
                                    // A call result carries the callee's
                                    // established qualifications: the
                                    // callee's authored `ensures result in
                                    // <domain>` already lowered onto the
                                    // operation result, and the required-set
                                    // check below decides whether those
                                    // carried domains satisfy this formal.
                                    // EstablishRecord keeps the empty-
                                    // qualification admission it always had.
                                    let qualifications_allowed = matches!(
                                        operation.kind,
                                        OperationKind::CallStructuralWithScalarArguments { .. }
                                            | OperationKind::BoundaryCall { .. }
                                    );
                                    let view_descriptor = matches!(
                                        operation.kind,
                                        OperationKind::EstablishElementView { .. }
                                            | OperationKind::EstablishReference { .. }
                                    );
                                    let admitted_producer = match operation.kind {
                                        OperationKind::CallStructuralWithScalarArguments { .. }
                                        | OperationKind::BoundaryCall { .. } => {
                                            matches!(argument.access,
                                                StructuralAccess::Owned | StructuralAccess::SharedBorrow)
                                                && expected.access == argument.access
                                                && expected.multiplicity == if argument.access == StructuralAccess::SharedBorrow {
                                                    StructuralMultiplicity::Unrestricted
                                                } else {
                                                    StructuralMultiplicity::Affine
                                                }
                                                && !expected.is_self
                                        }
                                        // A constructed record reaches a
                                        // reference-result call through one
                                        // projected owned subtree: the call's
                                        // result source map rejoins the exact
                                        // leaf loans under that edge.
                                        OperationKind::EstablishRecord { .. }
                                        | OperationKind::EstablishStructuralCase { .. }
                                        | OperationKind::EstablishStructuralLocal { .. } => {
                                            (reference_result_call
                                                && argument.access == StructuralAccess::Owned
                                                && expected.access == argument.access
                                                && expected.multiplicity
                                                    == StructuralMultiplicity::Affine
                                                && !expected.is_self
                                                && expected.qualifications.is_empty()
                                                && expected.projected_qualifications.is_empty())
                                                // A shared loan may observe a
                                                // locally constructed record:
                                                // its relocated descriptors are
                                                // copies the callee reads but
                                                // never moves.
                                                || (matches!(
                                                    operation.kind,
                                                    OperationKind::EstablishRecord { .. }
                                                        | OperationKind::EstablishStructuralLocal {
                                                            ..
                                                        }
                                                ) && argument.access
                                                    == StructuralAccess::SharedBorrow
                                                    && expected.access == argument.access
                                                    && expected.multiplicity
                                                        == StructuralMultiplicity::Unrestricted
                                                    && !expected.is_self)
                                        }
                                        // A view-establishment result names
                                        // a relocated descriptor: a shared
                                        // loan may hand the descriptor on by
                                        // copy, so it keeps Unrestricted
                                        // multiplicity rather than affine.
                                        OperationKind::EstablishElementView { .. }
                                        | OperationKind::EstablishReference { .. } => {
                                            argument.access
                                                == StructuralAccess::SharedBorrow
                                                && expected.access == argument.access
                                                && expected.multiplicity
                                                    == StructuralMultiplicity::Unrestricted
                                                && !expected.is_self
                                        }
                                        _ => false,
                                    };
                                    operation
                                        .result
                                        .structural()
                                        .filter(|result| {
                                            admitted_producer
                                                && result.place == argument.place
                                                && result.structural_type == structural_type
                                                && (result.multiplicity
                                                    == StructuralMultiplicity::Affine
                                                    || (view_descriptor
                                                        && result.multiplicity
                                                            == StructuralMultiplicity::Unrestricted))
                                                && (qualifications_allowed
                                                    || (result.qualifications.is_empty()
                                                        && result
                                                            .projected_qualifications
                                                            .is_empty()))
                                                && result.claims.is_empty()
                                        })
                                });
                            producer_result.is_some()
                        } =>
                    {
                        let operation = caller
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| operation.id == producer)
                            .expect("producer operation resolved above");
                        let view_descriptor = matches!(
                            operation.kind,
                            OperationKind::EstablishElementView { .. }
                                | OperationKind::EstablishReference { .. }
                        );
                        let result = operation
                            .result
                            .structural()
                            .expect("producer structural result resolved above");
                        Some((
                            structural_type,
                            if view_descriptor {
                                StructuralMultiplicity::Unrestricted
                            } else {
                                StructuralMultiplicity::Affine
                            },
                            if view_descriptor {
                                StructuralAccess::SharedBorrow
                            } else {
                                StructuralAccess::Owned
                            },
                            result.qualifications.as_slice(),
                            result.projected_qualifications.as_slice(),
                        ))
                    }
                    _ => None,
                }
            })
        })
    else {
        return Err(ModuleError::UnknownStructuralArgument {
            operation,
            argument_index: index as u32,
            place: argument.place,
        });
    };
    if !allow_projected && !argument.path.is_empty() {
        return Err(ModuleError::InvalidStructuralArgumentPath {
            operation,
            argument_index: index as u32,
        });
    }
    // A `RuntimeIndex` segment carries no authority of its own: its selector
    // is an operand of this call and its bound is the obligation the call
    // owns, reconstructed with every other runtime index of the operation
    // (`runtime_indexes`). Only the projection's type is resolved here.
    let root_type = actual_type;
    // A plain array payload keeps owned backing across a call-scoped loan.
    // Availability is checked by scalar-array replay and exclusive overlap by
    // argument-pair validation; neither is implied by matching the carrier.
    let array_payload_loan = matches!(
        argument.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && actual_access == StructuralAccess::Owned
        && crate::validation::scalar::array::plain_return_source(module, caller, argument.place);
    if crate::validation::scalar::array::owned_payload_source(module, caller, argument.place)
        && (expected.multiplicity != StructuralMultiplicity::Unrestricted
            // A shared or exclusive result loan views the established payload
            // without transferring it. Projection replay below determines the
            // exact lent leaf; the source remains the complete owned array.
            || !(expected.access == StructuralAccess::Owned
                || (array_payload_loan && expected.access == argument.access)
                || (expected.access == StructuralAccess::SharedBorrow
                    && argument.access == StructuralAccess::SharedBorrow))
            || !expected.qualifications.is_empty()
            || !expected.projected_qualifications.is_empty())
    {
        return Err(ModuleError::ScalarArrayResultMismatch(operation));
    }
    // Inline byte fields retain their owner/path instead of acquiring a
    // fictitious structural type identity. Ordinary calls admit only
    // an exact mutable field subloan; this grants no extent replacement.
    let buffer_presentation = (source_policy
        == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
        || (borrowed_call && is_unrestricted_mutable_subloan(module, caller, expected, argument)))
        && terminal_semantics::boundary_buffer_capacity(
            module.structural_types.iter(),
            root_type,
            argument,
            expected,
        )
        .is_some();
    // Shared presentation observes whole or projected bounded storage under
    // the already resolved source contract. Neither a call's result kind nor
    // an entry-only/nonempty-field recognizer establishes this authority.
    // Access attenuation, exact multiplicity and qualification checks below,
    // together with live-loan replay, remain independent obligations.
    let shared_buffer_presentation = expected.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            actual_multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        && terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            root_type,
            argument,
            expected,
        )
        .is_some();
    // A stored shared view field lends its own descriptor to a shared view
    // formal. Its declared predicates hold at every point, so they are the
    // qualification evidence; they must imply every predicate the formal's
    // qualifications name.
    let view_field_presentation = terminal_semantics::stored_view_field_presentation(
        module.structural_types.iter(),
        root_type,
        argument,
        expected,
    )
    .is_some_and(|declared| {
        terminal_semantics::byte_parameter_predicates(module, expected)
            .is_some_and(|required| declared.implies(required))
    });
    // Collection presentation consumes the source contract already resolved
    // above. Parameter, state arrival and completed owned homes lend the same
    // exact backing; their distinct availability checks do not change its
    // geometry. The source retains its actual access and multiplicity;
    // presenting its elements never changes the owner's disposal custody.
    let fixed_array_presentation = (borrowed_call
        || ordinary_call
        || source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals)
        && structural_access_can_supply(actual_access, argument.access)
        && !caller
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
        && !caller
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == argument.place)
        && {
            let backing = StructuralParameterDeclaration {
                place: argument.place,
                position: expected.position,
                is_self: false,
                structural_type: root_type,
                multiplicity: actual_multiplicity,
                access: actual_access,
                qualifications: actual_qualifications.to_vec(),
                projected_qualifications: actual_projected_qualifications.to_vec(),
            };
            terminal_semantics::fixed_byte_array_extent(
                module.structural_types.iter(),
                &backing,
                argument,
                expected,
            )
            .is_some()
                || terminal_semantics::fixed_element_array_extent(
                    module.structural_types.iter(),
                    &backing,
                    argument,
                    expected,
                )
                .is_some()
        };
    // A copied bounded field is a fresh owned snapshot, not an owner-to-view
    // presentation. Keep its shape admission separate from the qualification
    // judgment below: readable bytes alone do not prove a byte domain.
    let copied_field_predicates = if ordinary_call
        && argument.path.is_empty()
        && argument.access == StructuralAccess::Owned
        && expected.access == StructuralAccess::Owned
        && actual_multiplicity == StructuralMultiplicity::Unrestricted
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && root_type == expected.structural_type
        && !expected.is_self
        && expected.projected_qualifications.is_empty()
        && !caller
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
        && !caller
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == argument.place)
    {
        terminal_semantics::byte_copied_field_predicates_at_call(
            module,
            caller,
            argument.place,
            operation,
        )
    } else {
        None
    };
    let copied_field_qualifications = copied_field_predicates.is_some_and(|captured| {
        terminal_semantics::byte_parameter_predicates(module, expected)
            .is_some_and(|required| captured.implies(required))
    });
    // A state arrival names the same owner under its incoming declaration.
    // Resolve that exact storage role before checking the common capacity and
    // writable-predicate contract; machine-entry spelling grants no authority.
    let bounded_owner_presentation = (ordinary_call
        && terminal_semantics::arrival_parameter(caller, argument.place).is_some_and(|actual| {
            terminal_semantics::bounded_byte_owner_capacity(module, actual, argument, expected)
                .is_some()
                && !caller
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == argument.place)
                && !caller
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == argument.place)
        }))
        || copied_field_predicates.is_some()
        || ((ordinary_call || boundary_byte_view_call)
            && owned_bounded_owner_presentation(module, caller, argument, expected));
    // A whole owner cannot bypass its predicate-preserving loan contract by
    // falling through to raw type equality for an unprojected argument.
    if ordinary_call
        && module.structural_types.iter().any(|row| {
            row.id == expected.structural_type
                && matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
        && !bounded_owner_presentation
    {
        return Err(ModuleError::InvalidStructuralArgumentPath {
            operation,
            argument_index: index as u32,
        });
    }
    if !buffer_presentation
        && !shared_buffer_presentation
        && !fixed_array_presentation
        && !bounded_owner_presentation
        && !view_field_presentation
    {
        let Some(actual_type) = crate::validation::foundation::resolve_runtime_projection(
            module,
            root_type,
            &argument.path,
        ) else {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        };
        if actual_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual_type,
            });
        }
    }
    if argument.access != expected.access {
        return Err(ModuleError::StructuralArgumentAccessMismatch {
            operation,
            argument_index: index as u32,
            expected: expected.access,
            actual: argument.access,
        });
    }
    if !structural_access_can_supply(actual_access, argument.access) {
        return Err(ModuleError::StructuralArgumentAccessExceedsSource {
            operation,
            argument_index: index as u32,
            source: actual_access,
            presented: argument.access,
        });
    }
    let unrestricted_write_only_field_subloan =
        is_unrestricted_write_only_subloan(module, caller, expected, argument);
    let unrestricted_shared_field_subloan =
        is_unrestricted_shared_subloan(module, caller, expected, argument);
    let unrestricted_mutable_field_subloan =
        is_unrestricted_mutable_subloan(module, caller, expected, argument);
    // A loan rooted at a view binding presents unrestricted for the
    // same reason a parameter subloan does: nothing moves, the descriptor
    // stays the caller's custody, and each borrow kind stays inside the
    // access the establishing operation minted the view with.
    let view_loan =
        crate::validation::element_view::view_loan_result_in_module(module, caller, argument.place)
            .is_some_and(|(_, source_access)| {
                argument.access != StructuralAccess::Owned
                    && structural_access_can_supply(source_access, argument.access)
                    && expected.access == argument.access
                    && expected.multiplicity == StructuralMultiplicity::Unrestricted
                    && (argument.path.is_empty()
                        || super::is_static_borrow_path(&argument.path)
                        || is_runtime_indexed_borrow_path(&argument.path))
            });
    // A shared view is unrestricted without changing the owning root's
    // affine multiplicity. Construction-local loans remain separate.
    let shared_affine_loan = argument.path.is_empty()
        && argument.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual_access == StructuralAccess::Owned
        && actual_multiplicity == StructuralMultiplicity::Affine
        && (is_structural_call_result(caller, argument.place)
            || crate::validation::record::plain_return_source(module, caller, argument.place)
            || crate::validation::record::constructed_source(module, caller, argument.place)
                .is_some()
            || (crate::validation::scalar::case::plain_return_source(
                module,
                caller,
                argument.place,
            ) || crate::validation::structural::case::plain_return_source(
                module,
                caller,
                argument.place,
            ))
            || crate::validation::block_views::parameter(caller, argument.place).is_some()
            || caller
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == argument.place));
    let actual_multiplicity = if shared_affine_loan
        || view_loan
        || fixed_array_presentation
        || is_record_loan(module, caller, expected, argument)
    {
        StructuralMultiplicity::Unrestricted
    } else if argument.path.is_empty() {
        actual_multiplicity
    } else if unrestricted_write_only_field_subloan
        || unrestricted_shared_field_subloan
        || unrestricted_mutable_field_subloan
        || shared_buffer_presentation
        || view_field_presentation
        || bounded_owner_presentation
        || (buffer_presentation
            && actual_access == StructuralAccess::MutableBorrow
            && actual_multiplicity == StructuralMultiplicity::Unrestricted)
    {
        StructuralMultiplicity::Unrestricted
    } else if expected.multiplicity == StructuralMultiplicity::Affine
        && is_partial_affine_path(module, root_type, &argument.path)
        && actual_multiplicity == StructuralMultiplicity::Affine
    {
        StructuralMultiplicity::Affine
    } else {
        StructuralMultiplicity::Linear
    };
    if actual_multiplicity != expected.multiplicity {
        return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
            operation,
            argument_index: index as u32,
            expected: expected.multiplicity,
            actual: actual_multiplicity,
        });
    }
    // Primitive byte membership is a property of the exact live contents.
    // Reconstruct it through ordinary producers (including subslices), rather
    // than requiring a consumer-selected domain annotation on their results.
    let byte_content_qualifications = argument.path.is_empty()
        && argument.access == StructuralAccess::SharedBorrow
        && expected.access == StructuralAccess::SharedBorrow
        && module.structural_types.iter().any(|row| {
            row.id == expected.structural_type
                && matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(_)
                )
        })
        && terminal_semantics::byte_parameter_predicates(module, expected)
            .zip(terminal_semantics::byte_sequence_source_predicates(
                module,
                caller,
                argument.place,
            ))
            .is_some_and(|(required, available)| available.implies(required));
    for qualification in &expected.qualifications {
        if (!bounded_owner_presentation || copied_field_predicates.is_some())
            && !copied_field_qualifications
            && !view_field_presentation
            && !byte_content_qualifications
            && !structural_occurrence_carries_qualification(
                actual_qualifications,
                actual_projected_qualifications,
                &argument.path,
                *qualification,
            )
        {
            return Err(ModuleError::StructuralArgumentMissingQualification {
                operation,
                argument_index: index as u32,
                domain: *qualification,
            });
        }
    }
    for qualification in &expected.projected_qualifications {
        let mut source_path = argument.path.clone();
        source_path.extend(qualification.path.iter().cloned());
        if !structural_occurrence_carries_qualification(
            actual_qualifications,
            actual_projected_qualifications,
            &source_path,
            qualification.domain,
        ) {
            return Err(ModuleError::StructuralArgumentMissingQualification {
                operation,
                argument_index: index as u32,
                domain: qualification.domain,
            });
        }
    }
    Ok(())
}

/// A whole established byte local supplies its exact owned carrier, or lends
/// that carrier/a shared byte view. The source keeps its identity and live extent;
/// current predicates, not stale initializer octets, supply formal membership.
fn owned_bounded_owner_presentation(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> bool {
    if !argument.path.is_empty()
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow | StructuralAccess::Owned
        )
        || expected.access != argument.access
        || expected.multiplicity != StructuralMultiplicity::Unrestricted
        || expected.is_self
        || !expected.projected_qualifications.is_empty()
        || caller
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place)
        || caller
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == argument.place)
        || terminal_semantics::byte_owned_result_predicates(module, caller, argument.place)
            .is_none()
    {
        return false;
    }
    let Some(result) =
        crate::validation::structural::result_contracts::source_signature(caller, argument.place)
    else {
        return false;
    };
    let admitted_carrier = module.structural_types.iter().any(|row| {
        row.id == expected.structural_type
            && match row.shape {
                crate::validation::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => result.structural_type == expected.structural_type,
                crate::validation::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    },
                ) => argument.access == StructuralAccess::SharedBorrow,
                _ => false,
            }
    });
    let Some(required) = terminal_semantics::byte_parameter_predicates(module, expected) else {
        return false;
    };
    admitted_carrier
        && terminal_semantics::byte_sequence_source_predicates(module, caller, argument.place)
            .is_some_and(|established| established.implies(required))
}
