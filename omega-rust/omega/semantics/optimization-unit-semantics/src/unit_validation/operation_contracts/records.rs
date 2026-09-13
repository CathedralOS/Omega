//! Reconstruct complete record construction from the current executable inputs.
//!
//! Earlier operations own evaluation order. This operation binds their finished
//! values to declaration identities and transfers whole children atomically.
//! Source admission alone cannot authorize substituted fields or child homes in
//! a rewritten optimization unit; dominance and current ownership are checked
//! separately against the same operands.
use super::*;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape,
};

/// A plain record result is governed by its signature and custody, independent
/// of the operations used to produce it. Argument access, current availability,
/// and the callee's return operands are validated by their ordinary owners.
pub(crate) fn plain_record_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
        ..
    } = operation
    else {
        return false;
    };
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = &callee.verified_contract else {
        return false;
    };
    matches!(
        signature.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && selected_evidence.is_empty()
        && callee.entry_claims.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.crash_routes.is_empty()
        && contract.outcome_specific_ensures.is_empty()
        && callee.structural_parameters.iter().all(|parameter| {
            matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            ) && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && plain_record(types, signature.structural_type)
}

pub(crate) fn record_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishRecord {
        psi_operation,
        result,
        fields,
    } = operation
    else {
        return false;
    };
    let exact_result_place = function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == *psi_operation && structural_type == result.structural_type)
    });
    if !exact_result_place
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !plain_record(types, result.structural_type)
    {
        return false;
    }
    let Some(terminal_psi::StructuralTypeDeclaration {
        shape: StructuralTypeShape::Record {
            fields: declarations,
        },
        ..
    }) = types.get(&result.structural_type).copied()
    else {
        return false;
    };
    if declarations.len() != fields.len() {
        return false;
    }
    declarations
        .iter()
        .zip(fields)
        .all(|(declaration, initializer)| {
            if declaration.id != initializer.field {
                return false;
            }
            match (&declaration.field_type, &initializer.value) {
                (
                    StructuralFieldType::Structural(expected),
                    RecordFieldValue::Structural(argument),
                ) => {
                    let Some(source) = structural_source_contract(function, argument.place, false)
                    else {
                        return false;
                    };
                    argument.access == StructuralAccess::Owned
                        && argument.path.is_empty()
                        && source.access == StructuralAccess::Owned
                        && source.structural_type == *expected
                        && source.is_unqualified()
                        && !source_has_claims(function, argument.place)
                        && argument.place != result.place
                        && matches!(
                            source.multiplicity,
                            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
                        )
                        && (result.multiplicity != StructuralMultiplicity::Unrestricted
                            || source.multiplicity == StructuralMultiplicity::Unrestricted)
                }
                (
                    field_type,
                    RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    },
                ) => {
                    field_type.scalar_type().is_some_and(|scalar_type| {
                        function
                            .parameters
                            .iter()
                            .chain(function.blocks.iter().flat_map(|block| {
                                block
                                    .parameters
                                    .iter()
                                    .chain(block.nodes.iter().flat_map(|node| &node.definitions))
                            }))
                            .any(|definition| {
                                definition.value == *value && definition.scalar_type == scalar_type
                            })
                    }) && matches!(field_type, StructuralFieldType::BoundedInteger(_))
                        == range_obligation.is_some()
                }
                _ => false,
            }
        })
}

fn source_has_claims(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                let result = match &node.operation {
                    O::EstablishScalarArray { result, .. }
                    | O::EstablishScalarCase { result, .. }
                    | O::EstablishRecord { result, .. }
                    | O::CallStructural { result, .. }
                    | O::BoundaryCall {
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } => result,
                    _ => return false,
                };
                result.place == place && !result.claims.is_empty()
            })
}

fn plain_record(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![(root, false)];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some((current, exiting)) = pending.pop() {
        if exiting {
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        if complete.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return false;
        }
        let Some(terminal_psi::StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields },
            ..
        }) = types.get(&current).copied()
        else {
            return false;
        };
        pending.push((current, true));
        for field in fields {
            if field.relevance.is_erased() {
                return false;
            }
            match field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child) => pending.push((child, false)),
                _ => return false,
            }
        }
    }
    true
}

/// Reconstruct the completed owner, not merely a same-shaped source contract.
/// Borrowed receivers retain this owner's storage and do not change its disposal debt.
pub(crate) fn completed_record_source<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    let mut matches = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::EstablishRecord { result, .. } | O::CallStructural { result, .. }
                if result.place == place =>
            {
                Some(result)
            }
            _ => None,
        });
    let result = matches.next()?;
    (matches.next().is_none()
        && matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && plain_record(types, result.structural_type))
    .then_some(result)
}

pub(crate) fn record_loan(
    function: &PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    argument.access != StructuralAccess::Owned
        && argument.access == parameter.access
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && completed_record_source(function, argument.place, types)
            .map(|result| result.structural_type)
            .or_else(|| {
                // Whole and projected loans borrow the selected block value. This
                // checks the current signature; declaration, dominance, and
                // frontier replay still own that value's availability.
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters)
                    .find(|source| {
                        source.place == argument.place
                            && argument.access == StructuralAccess::SharedBorrow
                            && (argument.path.is_empty() || is_nonempty_field_path(&argument.path))
                            && source.access == StructuralAccess::Owned
                            && matches!(
                                source.multiplicity,
                                StructuralMultiplicity::Affine
                                    | StructuralMultiplicity::Unrestricted
                            )
                            && source.qualifications.is_empty()
                            && source.projected_qualifications.is_empty()
                            && plain_record(types, source.structural_type)
                    })
                    .map(|source| source.structural_type)
            })
            .is_some_and(|root_type| {
                resolve_structural_path(types, root_type, &argument.path)
                    == Some(parameter.structural_type)
            })
}
