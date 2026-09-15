//! Structural argument sources, qualifications, paths and subloans.

use crate::validation::structural_operations::claim_transfers::is_record_loan;
use crate::validation::{
    BTreeSet, ModuleError, OperationId, OperationKind, PlaceId, StructuralAccess,
    StructuralArgument, StructuralDomainId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalModule, is_nonempty_field_path,
    is_partial_affine_path, partial_affine_root_type, resolve_structural_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralArgumentSourcePolicy {
    OnlyParameters,
    /// Whole linear call results retain their exact completed qualification and
    /// claim occurrence; the frontier separately requires that occurrence live.
    ParametersOrLinearCallResults,
    /// Boundary calls accept borrowed byte literals and whole affine call results,
    /// but not construction-local establishments.
    ParametersOrBoundaryActuals,
    /// Unit and scalar-result calls retain construction locals and whole
    /// ordinary and boundary affine results, whole owned primitive arrays,
    /// and whole immutable byte views.
    ParametersOrAffineLocalsAndCallResults,
    /// Whole record establishments and claim-free affine call results.
    /// Frontier validation separately requires their producer to have run.
    ParametersOrAffineOperationResults,
}

pub(crate) fn is_structural_call_result(caller: &TerminalMachine, place: PlaceId) -> bool {
    caller.structural_places.iter().any(|declaration| {
        declaration.id == place
            && matches!(declaration.kind,
            StructuralPlaceKind::OperationResult { producer, .. }
                if caller.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
                    operation.id == producer && matches!(operation.kind,
                        OperationKind::CallStructuralWithScalarArguments { .. }
                            | OperationKind::BoundaryCall { .. })
                }))
    })
}

/// This is a static occurrence lookup, not availability evidence. The same
/// claim can visit several results, so select by exact place and producer, never
/// by a machine-wide search for the first occurrence of that claim identity.
pub(crate) fn linear_call_result(
    caller: &TerminalMachine,
    place: PlaceId,
) -> Option<(
    &terminal_psi::Operation,
    &terminal_psi::StructuralOperationResult,
)> {
    let mut declarations = caller
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let mut operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some()
        || !matches!(
            operation.kind,
            OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
        )
    {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Linear
        && !result.claims.is_empty())
    .then_some((operation, result))
}

pub(crate) fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
    source_policy: StructuralArgumentSourcePolicy,
) -> Result<(), ModuleError> {
    let call_kind = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| candidate.id == operation)
        .map(|candidate| &candidate.kind);
    let unit_call = matches!(call_kind, Some(OperationKind::CallUnit { .. }));
    let borrowed_call = matches!(
        call_kind,
        Some(
            OperationKind::CallUnit { .. }
                | OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
        )
    );
    let ordinary_call = matches!(
        call_kind,
        Some(OperationKind::CallUnit { .. } | OperationKind::CallStructuralScalar { .. })
    ) || (source_policy
        == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
        && matches!(
            call_kind,
            Some(
                OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        ));
    let result_projection = allow_projected && unit_call;
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        if argument.path.contains(&StructuralPathSegment::Referent) {
            if matches!(call_kind, Some(OperationKind::BoundaryCall { .. })) {
                return Err(crate::validation::references::invalid(
                    caller,
                    "reference projections at host boundaries are not yet supported",
                ));
            }
            if crate::validation::references::source_type(module, caller, argument)
                != Some(expected.structural_type)
                || argument.access != expected.access
                || expected.multiplicity != StructuralMultiplicity::Unrestricted
                || !expected.qualifications.is_empty()
                || !expected.projected_qualifications.is_empty()
            {
                return Err(crate::validation::references::invalid(
                    caller,
                    "reference argument does not match its exact primitive parameter",
                ));
            }
            continue;
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
                            crate::validation::structural_result_contracts::source_signature(caller, argument.place).map(|source| (
                                source.structural_type, source.multiplicity, StructuralAccess::Owned,
                                source.qualifications, source.projected_qualifications,
                            ))
                        }
                        StructuralPlaceKind::OperationResult { .. }
                            if source_policy == StructuralArgumentSourcePolicy::ParametersOrLinearCallResults
                                && argument.path.is_empty() && argument.access == StructuralAccess::Owned =>
                        {
                            let (_, result) = linear_call_result(caller, argument.place)?;
                            Some((result.structural_type, result.multiplicity, StructuralAccess::Owned,
                                result.qualifications.as_slice(), result.projected_qualifications.as_slice()))
                        }
                        StructuralPlaceKind::OperationResult { .. }
                            if !matches!(source_policy, StructuralArgumentSourcePolicy::OnlyParameters
                                | StructuralArgumentSourcePolicy::ParametersOrLinearCallResults)
                                && source_policy != StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                && (argument.path.is_empty() || argument.access != StructuralAccess::Owned)
                                && crate::validation::record::completed_source(module, caller, argument.place).is_some() =>
                        {
                            let result = crate::validation::record::completed_source(module, caller, argument.place)?;
                            Some((result.structural_type, result.multiplicity, StructuralAccess::Owned, &[][..], &[][..]))
                        }
                        StructuralPlaceKind::OperationResult { .. }
                            if ordinary_call
                                && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                                && argument.path.is_empty()
                                && matches!(argument.access, StructuralAccess::Owned | StructuralAccess::SharedBorrow)
                                && crate::validation::scalar_case::plain_return_source(module, caller, argument.place) =>
                        {
                            // Atomic case establishment supplies the same complete
                            // owned value at calls as at returns. Keep its actual
                            // multiplicity; frontier replay still requires the
                            // exact producer to dominate and transfer only once.
                            crate::validation::structural_result_contracts::source_signature(caller, argument.place).map(|source| (
                                source.structural_type, source.multiplicity, StructuralAccess::Owned,
                                source.qualifications, source.projected_qualifications,
                            ))
                        }
                        StructuralPlaceKind::OperationResult { structural_type, .. }
                            if ordinary_call
                                && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults
                                && argument.path.is_empty() && argument.access == StructuralAccess::Owned
                                && crate::validation::scalar_array::plain_return_source(module, caller, argument.place) =>
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
                                || (allow_projected && ordinary_call
                                    && argument.access == StructuralAccess::SharedBorrow
                                    && is_nonempty_field_path(&argument.path)
                                    && is_record_loan(module, caller, expected, argument)))
                                && (argument.access == StructuralAccess::SharedBorrow
                                    || (ordinary_call && argument.access == StructuralAccess::Owned)
                                    || (ordinary_call && argument.access == StructuralAccess::MutableBorrow
                                        && crate::validation::block_views::parameter(caller, argument.place)
                                            .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)))
                                && (source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                    || (ordinary_call && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults)) =>
                        {
                            crate::validation::block_views::parameter(caller, argument.place).map(|parameter| (
                                parameter.structural_type, parameter.multiplicity, parameter.access,
                                parameter.qualifications.as_slice(), parameter.projected_qualifications.as_slice(),
                            ))
                        }
                        StructuralPlaceKind::OperationResult { .. }
                            if argument.path.is_empty()
                                && argument.access == StructuralAccess::SharedBorrow
                                && (source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
                                    || (ordinary_call && source_policy == StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults))
                                && crate::validation::byte_sequence_subslice::borrowed_result(caller, argument.place).is_some() =>
                        {
                            let result = crate::validation::byte_sequence_subslice::borrowed_result(caller, argument.place)
                                .expect("exact borrowed result checked above");
                            Some((result.structural_type, StructuralMultiplicity::Unrestricted,
                                StructuralAccess::SharedBorrow, &[][..], &[][..]))
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
                                &[][..],
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
                            && caller
                                .blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .any(|operation| {
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
                                                && expected.qualifications.is_empty()
                                                && expected.projected_qualifications.is_empty()
                                        }
                                        _ => false,
                                    };
                                    operation.id == producer && admitted_producer
                                        && operation.result.structural().is_some_and(|result| {
                                            result.place == argument.place
                                                && result.structural_type == structural_type
                                                && result.multiplicity
                                                    == StructuralMultiplicity::Affine
                                                && result.qualifications.is_empty()
                                                && result.projected_qualifications.is_empty()
                                                && result.claims.is_empty()
                                        })
                                }) =>
                        {
                            Some((
                                structural_type,
                                StructuralMultiplicity::Affine,
                                StructuralAccess::Owned,
                                &[][..],
                                &[][..],
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
        let root_type = actual_type;
        if crate::validation::scalar_array::owned_payload_source(module, caller, argument.place)
            && (expected.multiplicity != StructuralMultiplicity::Unrestricted
                || expected.access != StructuralAccess::Owned
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
            || (borrowed_call && is_unrestricted_mutable_subloan(caller, expected, argument)))
            && terminal_semantics::boundary_buffer_capacity(module, root_type, argument, expected)
                .is_some();
        // The shared counterpart: a boundary reads the field's live bytes
        // through a borrowed view. The operand keeps its owning root and path;
        // the shared loan grants no mutation, storage, or extent replacement.
        let shared_buffer_presentation = (source_policy
            == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
            || (borrowed_call && is_unrestricted_shared_subloan(caller, expected, argument)))
            && terminal_semantics::shared_boundary_buffer_capacity(
                module, root_type, argument, expected,
            )
            .is_some();
        // A boundary lends the same exact initialized fixed-array range as an
        // ordinary call. Keep the array's real type and path: presentation
        // is not a type substitution, storage grant, or permission to resize.
        let fixed_array_presentation = (borrowed_call
            || source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals)
            && caller.structural_parameters.iter().any(|actual| {
                terminal_semantics::mutable_fixed_byte_array_extent(
                    module, actual, argument, expected,
                )
                .is_some()
                    && !caller
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == argument.place)
                    && !caller
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == argument.place)
            });
        if !buffer_presentation && !shared_buffer_presentation && !fixed_array_presentation {
            let Some(actual_type) = resolve_structural_path(module, root_type, &argument.path)
            else {
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
            is_unrestricted_shared_subloan(caller, expected, argument);
        let unrestricted_mutable_field_subloan =
            is_unrestricted_mutable_subloan(caller, expected, argument);
        // A shared view is unrestricted without changing the owning root's
        // affine multiplicity. Construction-local loans remain separate.
        let shared_affine_loan = argument.path.is_empty()
            && argument.access == StructuralAccess::SharedBorrow
            && expected.multiplicity == StructuralMultiplicity::Unrestricted
            && actual_access == StructuralAccess::Owned
            && actual_multiplicity == StructuralMultiplicity::Affine
            && (is_structural_call_result(caller, argument.place)
                || crate::validation::record::plain_return_source(module, caller, argument.place)
                || crate::validation::scalar_case::plain_return_source(
                    module,
                    caller,
                    argument.place,
                )
                || crate::validation::block_views::parameter(caller, argument.place).is_some()
                || caller
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == argument.place));
        let actual_multiplicity =
            if shared_affine_loan || is_record_loan(module, caller, expected, argument) {
                StructuralMultiplicity::Unrestricted
            } else if argument.path.is_empty() {
                actual_multiplicity
            } else if unrestricted_write_only_field_subloan
                || unrestricted_shared_field_subloan
                || unrestricted_mutable_field_subloan
                || shared_buffer_presentation
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
        for qualification in &expected.qualifications {
            if !structural_occurrence_carries_qualification(
                actual_qualifications,
                actual_projected_qualifications,
                &argument.path,
                *qualification,
            ) {
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
    }
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            // Both arguments receive independent unrestricted array values;
            // neither takes an exclusive loan or moves the caller's payload.
            let copied_array = ordinary_call
                && left.access == StructuralAccess::Owned
                && right.access == StructuralAccess::Owned
                && left.path.is_empty()
                && right.path.is_empty()
                && crate::validation::scalar_array::plain_return_source(module, caller, left.place);
            // A complete unrestricted case is copied, not moved. Its owned
            // actual may coexist with a whole shared observation, but this
            // grants neither exclusive borrowing nor projected custody.
            let copied_case = ordinary_call
                && matches!(
                    left.access,
                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                )
                && matches!(
                    right.access,
                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                )
                && left.path.is_empty()
                && right.path.is_empty()
                && crate::validation::scalar_case::plain_return_source(module, caller, left.place)
                && crate::validation::structural_result_contracts::source_signature(
                    caller, left.place,
                )
                .is_some_and(|source| source.multiplicity == StructuralMultiplicity::Unrestricted);
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && !copied_array
                && !copied_case
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access)
                    || ((left.access == StructuralAccess::Owned
                        || right.access == StructuralAccess::Owned)
                        && caller.structural_places.iter().any(|place| {
                            place.id == left.place
                                && (matches!(
                                    place.kind,
                                    StructuralPlaceKind::OperationResult { .. }
                                ) || caller
                                    .structural_parameters
                                    .iter()
                                    .chain(
                                        caller
                                            .blocks
                                            .iter()
                                            .flat_map(|block| &block.structural_parameters),
                                    )
                                    .any(|parameter| {
                                        parameter.place == place.id
                                            && parameter.access == StructuralAccess::Owned
                                            && parameter.multiplicity
                                                == StructuralMultiplicity::Affine
                                    }))
                        })))
            {
                return Err(ModuleError::OverlappingExclusiveStructuralArguments {
                    operation,
                    first_argument: first as u32,
                    second_argument: second as u32,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn structural_occurrence_carries_qualification(
    root: &[StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
    path: &[StructuralPathSegment],
    domain: StructuralDomainId,
) -> bool {
    if path.is_empty() {
        root.contains(&domain)
    } else {
        projected
            .iter()
            .any(|qualification| qualification.path == path && qualification.domain == domain)
    }
}

fn structural_access_can_supply(source: StructuralAccess, presented: StructuralAccess) -> bool {
    match source {
        StructuralAccess::Owned => true,
        StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow => matches!(
            presented,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
    }
}

fn structural_access_is_exclusive(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    )
}

pub(crate) fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.iter().zip(right).all(|(left, right)| left == right)
}

fn literal_index_path(path: &[StructuralPathSegment]) -> Option<&[StructuralPathSegment]> {
    let field_count = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))?;
    let (fields, indexes) = path.split_at(field_count);
    (!indexes.is_empty()
        && fields
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        && indexes
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_))))
    .then_some(fields)
}

fn is_direct_literal_index_path(path: &[StructuralPathSegment]) -> bool {
    literal_index_path(path).is_some_and(|fields| fields.is_empty())
}

pub(crate) fn is_literal_indexed_field_path(path: &[StructuralPathSegment]) -> bool {
    literal_index_path(path).is_some_and(|fields| !fields.is_empty())
}

pub(crate) fn is_admitted_unit_call_argument_path(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> bool {
    argument.path.is_empty()
        || crate::validation::references::is_reference_projection(module, caller, argument)
        || is_nonempty_field_path(&argument.path)
        || is_literal_indexed_field_path(&argument.path)
        || is_direct_literal_index_path(&argument.path)
        || (matches!(argument.access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
            && is_static_borrow_path(&argument.path))
        // Structural paths already retain only fields and literal indexes;
        // write-only subloans may interleave them. Resolution checks each hop.
        || argument.access == StructuralAccess::WriteOnlyBorrow
        || (argument.access == StructuralAccess::Owned
            && partial_affine_root_type(caller, argument.place).is_some_and(|structural_type| {
                is_partial_affine_path(module, structural_type, &argument.path)
            }))
}

// Borrowing a statically located element does not transfer its owner. Shape
// recognition must not manufacture linear custody for a deeper array path;
// the receiving check separately resolves every field and fixed-index bound.
// First-class reference paths retain their own authority rules.

fn is_static_borrow_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            StructuralPathSegment::Field(identity) => !identity.is_empty(),
            StructuralPathSegment::FixedIndex(_) => true,
            StructuralPathSegment::Referent => false,
        })
}

fn is_material_write_only_type(module: &TerminalModule, structural_type: StructuralTypeId) -> bool {
    // Foundation establishes an acyclic, closed graph without bounding depth.
    // Traverse iteratively and inspect each shared declaration only once.
    let mut pending = vec![structural_type];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == current)
        else {
            return false;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => {}
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_) => {}
                        StructuralFieldType::Structural(next) => pending.push(next),
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::Erased { .. } => return false,
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => return false,
        }
    }
    true
}

pub(crate) fn is_unrestricted_write_only_subloan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    let indexed_path_is_material = !argument
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        || (is_material_write_only_type(module, actual.structural_type)
            && resolve_structural_path(module, actual.structural_type, &argument.path)
                .is_some_and(|leaf| {
                    module.structural_types.iter().any(|declaration| {
                        declaration.id == leaf
                            && matches!(
                                &declaration.shape,
                                StructuralTypeShape::PrimitiveScalar(_)
                                    | StructuralTypeShape::Record { .. }
                            )
                    })
                }));

    !argument.path.is_empty()
        && argument.access == StructuralAccess::WriteOnlyBorrow
        && expected.access == StructuralAccess::WriteOnlyBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            actual.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
        && indexed_path_is_material
}

pub(crate) fn is_unrestricted_shared_subloan(
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    (terminal_psi::is_bounded_structural_scalar_store_path(&argument.path)
        || (matches!(
            actual.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) && is_static_borrow_path(&argument.path)))
        && argument.access == StructuralAccess::SharedBorrow
        && expected.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}

pub(crate) fn is_unrestricted_mutable_subloan(
    caller: &TerminalMachine,
    expected: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    let Some(actual) = caller
        .structural_parameters
        .iter()
        .find(|actual| actual.place == argument.place)
    else {
        return false;
    };
    is_static_borrow_path(&argument.path)
        && argument.access == StructuralAccess::MutableBorrow
        && expected.access == StructuralAccess::MutableBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual.access == StructuralAccess::MutableBorrow
        && actual.multiplicity == StructuralMultiplicity::Unrestricted
}
