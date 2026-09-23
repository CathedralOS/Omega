//! One structural argument checked against the parameter it supplies.

use super::{
    CallShape, StructuralArgumentSourcePolicy, is_structural_call_result,
    is_unrestricted_mutable_subloan, is_unrestricted_shared_subloan,
    is_unrestricted_write_only_subloan, linear_call_result, structural_access_can_supply,
    structural_occurrence_carries_qualification,
};
use crate::validation::structural_operations::claim_transfers::is_record_loan;
use crate::validation::{
    ModuleError, OperationId, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceKind, TerminalMachine, TerminalModule, is_nonempty_field_path,
    is_partial_affine_path, partial_affine_root_type, resolve_structural_path,
};

/// One structural argument against its parameter: a reference projection
/// must match an exact primitive parameter; otherwise the argument's source
/// resolves to a known place whose type, access, multiplicity and
/// qualifications the parameter accepts, with the presentations the call
/// kind and source policy allow.
pub(super) fn validate_structural_argument(
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
                        crate::validation::structural_result_contracts::source_signature(caller, argument.place).map(|source| (
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
                            && (crate::validation::byte_sequence_subslice::borrowed_result(caller, argument.place).is_some()
                                || crate::validation::element_view_subslice::borrowed_result(caller, argument.place).is_some()) =>
                    {
                        let result = crate::validation::byte_sequence_subslice::borrowed_result(caller, argument.place)
                            .or_else(|| crate::validation::element_view_subslice::borrowed_result(caller, argument.place))
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
                                        | OperationKind::EstablishStructuralCase { .. } => {
                                            reference_result_call
                                                && argument.access == StructuralAccess::Owned
                                                && expected.access == argument.access
                                                && expected.multiplicity
                                                    == StructuralMultiplicity::Affine
                                                && !expected.is_self
                                                && expected.qualifications.is_empty()
                                                && expected.projected_qualifications.is_empty()
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
                                                && result.multiplicity
                                                    == StructuralMultiplicity::Affine
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
                        let result = caller
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| operation.id == producer)
                            .and_then(|operation| operation.result.structural())
                            .expect("producer structural result resolved above");
                        Some((
                            structural_type,
                            StructuralMultiplicity::Affine,
                            StructuralAccess::Owned,
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
    // A `RuntimeIndex` segment carries no authority of its own. Its selector
    // must name one direct scalar parameter of this caller at the dense
    // position the segment claims, that parameter must be a fixed-width
    // integer, and the inclusive bounds the segment spells must be exactly
    // what the caller's published evidence proves: either the catalog's
    // integer entry range row or, for callers that publish their contract
    // facts only as propositions, the `requires` conjuncts folded into the
    // same closed interval — with a nonnegative minimum. The
    // `maximum < extent` relation is replayed separately by
    // `resolve_structural_path` against the resolved fixed array.
    for segment in &argument.path {
        let StructuralPathSegment::RuntimeIndex {
            selector,
            minimum,
            maximum,
        } = segment
        else {
            continue;
        };
        let selector_bounded =
            caller
                .parameters
                .get(*selector as usize)
                .is_some_and(|declaration| {
                    let semantic_vocabulary::ScalarType::Integer(integer_type) =
                        declaration.scalar_type
                    else {
                        return false;
                    };
                    let roster_bounded = module
                        .scalar_qualifications
                        .integer_entry_ranges
                        .iter()
                        .any(|range| {
                            range.machine == caller.id
                                && range.parameter == declaration.id
                                && range.integer_type == integer_type
                                && range.minimum == *minimum
                                && range.maximum == *maximum
                        });
                    roster_bounded
                        || requires_bound_interval(caller, declaration, integer_type).is_some_and(
                            |(derived_minimum, derived_maximum)| {
                                derived_minimum == *minimum && derived_maximum == *maximum
                            },
                        )
                })
                && terminal_semantics::runtime_index_minimum_is_nonnegative(*minimum);
        if !selector_bounded {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        }
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
        || (borrowed_call && is_unrestricted_mutable_subloan(module, caller, expected, argument)))
        && terminal_semantics::boundary_buffer_capacity(
            module.structural_types.iter(),
            root_type,
            argument,
            expected,
        )
        .is_some();
    // The shared counterpart: a boundary reads the field's live bytes
    // through a borrowed view. The operand keeps its owning root and path;
    // the shared loan grants no mutation, storage, or extent replacement.
    let shared_buffer_presentation = (source_policy
        == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals
        || (borrowed_call && is_unrestricted_shared_subloan(module, caller, expected, argument)))
        && terminal_semantics::shared_boundary_buffer_capacity(
            module.structural_types.iter(),
            root_type,
            argument,
            expected,
        )
        .is_some();
    // A boundary lends the same exact initialized fixed-array range as an
    // ordinary call. Keep the array's real type and path: presentation
    // is not a type substitution, storage grant, or permission to resize.
    // Scalar-result calls lend the same read-only or mutable window: a
    // scalar return carries no custody, so the subloan shape is identical.
    let fixed_array_presentation = (borrowed_call
        || ordinary_call
        || source_policy == StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals)
        && caller.structural_parameters.iter().any(|actual| {
            (terminal_semantics::fixed_byte_array_extent(
                module.structural_types.iter(),
                actual,
                argument,
                expected,
            )
            .is_some()
                || terminal_semantics::fixed_element_array_extent(
                    module.structural_types.iter(),
                    actual,
                    argument,
                    expected,
                )
                .is_some())
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
        let Some(actual_type) = resolve_structural_path(module, root_type, &argument.path) else {
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
    // A shared view is unrestricted without changing the owning root's
    // affine multiplicity. Construction-local loans remain separate.
    let shared_affine_loan = argument.path.is_empty()
        && argument.access == StructuralAccess::SharedBorrow
        && expected.multiplicity == StructuralMultiplicity::Unrestricted
        && actual_access == StructuralAccess::Owned
        && actual_multiplicity == StructuralMultiplicity::Affine
        && (is_structural_call_result(caller, argument.place)
            || crate::validation::record::plain_return_source(module, caller, argument.place)
            || crate::validation::scalar_case::plain_return_source(module, caller, argument.place)
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
    Ok(())
}

/// The contract-fact evidence for a `RuntimeIndex` selector whose caller
/// publishes no integer entry-range row: the caller's `requires`
/// propositions fold into the closed inclusive interval they prove on this
/// parameter. Authored clauses publish merged into that proposition roster —
/// an `i <= K` conjunct is the same caller-discharged entry obligation the
/// checked admission folded — so the literal endpoints bound the selector
/// with exactly the strength a retained range row carries. An unsigned
/// carrier supplies its own `0 <=` half; a signed carrier owes an explicit
/// lower conjunct. Propositions this fold cannot read stay outside the
/// interval rather than declining it; a missing half or an interval that
/// cannot name a nonnegative element declines.
fn requires_bound_interval(
    caller: &TerminalMachine,
    declaration: &terminal_psi::ValueDeclaration,
    integer_type: semantic_vocabulary::IntegerType,
) -> Option<(
    semantic_vocabulary::IntegerValue,
    semantic_vocabulary::IntegerValue,
)> {
    let mut minimum: Option<i128> = match integer_type.sign() {
        semantic_vocabulary::IntegerSign::Unsigned => Some(0),
        semantic_vocabulary::IntegerSign::Signed => None,
    };
    let mut maximum = None;
    for proposition in &caller.contract.requires {
        fold_requires_bound(
            proposition,
            declaration,
            integer_type,
            &mut minimum,
            &mut maximum,
        );
    }
    let (minimum, maximum) = minimum.zip(maximum)?;
    if !(0 <= minimum && minimum <= maximum) {
        return None;
    }
    Some((
        integer_bound_value(integer_type, minimum)?,
        integer_bound_value(integer_type, maximum)?,
    ))
}

/// Spell one folded endpoint back in the selector's declared carrier so the
/// segment's `IntegerValue` bounds compare by exact identity, the same way
/// the retained range row's endpoints do.
fn integer_bound_value(
    integer_type: semantic_vocabulary::IntegerType,
    value: i128,
) -> Option<semantic_vocabulary::IntegerValue> {
    match integer_type.sign() {
        semantic_vocabulary::IntegerSign::Signed => {
            Some(semantic_vocabulary::IntegerValue::Signed(value))
        }
        semantic_vocabulary::IntegerSign::Unsigned => u128::try_from(value)
            .ok()
            .map(semantic_vocabulary::IntegerValue::Unsigned),
    }
}

/// Meet one `requires` proposition's literal bound on the subject into the
/// running interval. Published requires rows hold `Equal`, `LessThan` and
/// `LessOrEqual` scalar comparisons under `Conjunction` nesting: `p <= k`
/// with the subject on the left is the upper half and `k <= p` the lower.
/// Propositions over other parameters, composed terms, or non-literal
/// endpoints are contract facts this fold does not read, not a reason to
/// decline.
fn fold_requires_bound(
    proposition: &semantic_vocabulary::Proposition,
    declaration: &terminal_psi::ValueDeclaration,
    integer_type: semantic_vocabulary::IntegerType,
    minimum: &mut Option<i128>,
    maximum: &mut Option<i128>,
) {
    let (endpoint, subject_is_left) = match proposition {
        semantic_vocabulary::Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                fold_requires_bound(conjunct, declaration, integer_type, minimum, maximum);
            }
            return;
        }
        semantic_vocabulary::Proposition::Equal(left, right)
        | semantic_vocabulary::Proposition::LessOrEqual(left, right)
        | semantic_vocabulary::Proposition::LessThan(left, right) => {
            if conjunct_subject(left, declaration, integer_type) {
                (conjunct_literal(right, integer_type), true)
            } else if conjunct_subject(right, declaration, integer_type) {
                (conjunct_literal(left, integer_type), false)
            } else {
                return;
            }
        }
        _ => return,
    };
    let Some(endpoint) = endpoint else {
        return;
    };
    let (lower, upper) = match (proposition, subject_is_left) {
        (semantic_vocabulary::Proposition::Equal(..), _) => (Some(endpoint), Some(endpoint)),
        (semantic_vocabulary::Proposition::LessOrEqual(..), true) => (None, Some(endpoint)),
        (semantic_vocabulary::Proposition::LessOrEqual(..), false) => (Some(endpoint), None),
        (semantic_vocabulary::Proposition::LessThan(..), true) => (None, endpoint.checked_sub(1)),
        (semantic_vocabulary::Proposition::LessThan(..), false) => (endpoint.checked_add(1), None),
        _ => return,
    };
    if let Some(lower) = lower {
        *minimum = Some(minimum.map_or(lower, |bound| bound.max(lower)));
    }
    if let Some(upper) = upper {
        *maximum = Some(maximum.map_or(upper, |bound| bound.min(upper)));
    }
}

/// The conjunct subject is exactly the selector's own declared scalar
/// parameter: the `Value` term naming it carries the same fixed integer
/// carrier the parameter declares.
fn conjunct_subject(
    term: &semantic_vocabulary::ScalarTerm,
    declaration: &terminal_psi::ValueDeclaration,
    integer_type: semantic_vocabulary::IntegerType,
) -> bool {
    matches!(
        term,
        semantic_vocabulary::ScalarTerm::Value { id, scalar_type }
            if *id == declaration.id
                && *scalar_type == semantic_vocabulary::ScalarType::Integer(integer_type)
    )
}

/// A conjunct endpoint lands as a bound only when it is an integer literal
/// carried in the selector's own declared carrier — the same type identity
/// the retained range row checks on its endpoints.
fn conjunct_literal(
    term: &semantic_vocabulary::ScalarTerm,
    integer_type: semantic_vocabulary::IntegerType,
) -> Option<i128> {
    let semantic_vocabulary::ScalarTerm::Integer { scalar_type, value } = term else {
        return None;
    };
    if *scalar_type != integer_type {
        return None;
    }
    match value {
        semantic_vocabulary::IntegerValue::Signed(value) => Some(*value),
        semantic_vocabulary::IntegerValue::Unsigned(value) => i128::try_from(*value).ok(),
    }
}
