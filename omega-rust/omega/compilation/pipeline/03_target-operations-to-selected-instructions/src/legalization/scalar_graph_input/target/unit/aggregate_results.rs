//! Rejoin aggregate constructors to their exact source operations.
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit, Source,
    TargetUnitOperation, ValueId,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &[(ValueId, Source)],
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    match (target, abstracted) {
        (
            TargetUnitOperation::EstablishStructuralArray {
                psi_operation,
                result_home,
                elements,
            },
            AbstractOperation::EstablishStructuralArray {
                psi_operation: expected_operation,
                result,
                elements: expected_elements,
            },
        ) => {
            let declaration = plan
                .structural_type(result.structural_type)
                .ok_or(LegalizationError::custody())?;
            let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                declaration.shape
            else {
                return Err(LegalizationError::custody());
            };
            if psi_operation != expected_operation
                || elements != expected_elements
                || u64::try_from(elements.len()).ok() != Some(length)
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
            {
                return Err(LegalizationError::custody());
            }
            for argument in elements {
                let actual =
                    owned_projected_field_source(optimized, argument, &unit.structural_types)?;
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || argument.place == result.place
                    || actual != element
                {
                    return Err(LegalizationError::custody());
                }
            }
        }

        (
            TargetUnitOperation::EstablishRecord {
                psi_operation,
                result_home,
                fields,
            },
            AbstractOperation::EstablishRecord {
                psi_operation: expected_operation,
                result,
                fields: expected_fields,
            },
        ) => {
            let declaration = plan
                .structural_type(result.structural_type)
                .ok_or(LegalizationError::custody())?;
            let terminal_psi::StructuralTypeShape::Record {
                fields: declarations,
                ..
            } = &declaration.shape
            else {
                return Err(LegalizationError::custody());
            };
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased());
            if psi_operation != expected_operation
                || fields != expected_fields
                || fields.len() != declarations.clone().count()
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || fields.iter().zip(declarations).any(|(field, declaration)| {
                    field.field != declaration.id
                        || declaration.relevance.is_erased()
                        || match &field.value {
                            terminal_psi::RecordFieldValue::Scalar {
                                value: operand,
                                range_obligation,
                            } => {
                                terminal_semantics::scalar_field_requires_evidence(
                                    &declaration.field_type,
                                ) != range_obligation.is_some()
                                    || !sources.iter().any(|(value, source)| {
                                        *value == *operand
                                            && declaration.field_type.scalar_type()
                                                == Some(source.scalar_type())
                                    })
                            }
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                validate_owned_field_operand(
                                    optimized,
                                    result.place,
                                    argument,
                                    &declaration.field_type,
                                    &unit.structural_types,
                                )
                                .is_err()
                            }
                        }
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::EstablishScalarArray {
                psi_operation,
                result_home,
                elements,
            },
            AbstractOperation::EstablishScalarArray {
                psi_operation: expected_operation,
                result,
                elements: expected_elements,
            },
        ) => {
            let (scalar_type, count, _) = scalar_graph_input::scalar_arrays::shape(result, plan)?;
            if psi_operation != expected_operation
                || elements != expected_elements
                || u64::try_from(elements.len()).ok() != Some(count)
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || elements.iter().any(|element| {
                    !sources.iter().any(|(value, source)| {
                        value == element && source.scalar_type() == scalar_type
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::StructuralLeafCopy {
                psi_operation,
                result_home,
                source,
                path,
                byte_offset,
                indices,
            },
            AbstractOperation::StructuralLeafCopy {
                psi_operation: expected_operation,
                result,
                source: expected_source,
                path: expected_path,
            },
        ) => {
            let (expected_offset, expected_shape, expected_indices) =
                scalar_graph_input::structural_case::leaf_copy_layout(
                    optimized,
                    *expected_operation,
                    *expected_source,
                    expected_path,
                    result,
                    plan,
                )?;
            // Each runtime index must reference the selector's dominating
            // definition — parameter, block parameter, or computed value —
            // the same source the lowered operand already proved; custody
            // compares value, integer type and stride, not the operand
            // variant the producer dispatched.
            let expected_indices = expected_indices
                .into_iter()
                .map(|element| {
                    let scalar_type = crate::legalization::runtime_indices::defined_type(
                        optimized,
                        element.index,
                    )
                    .filter(|scalar| {
                        matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                            if integer.bits() <= 64)
                    })
                    .ok_or(LegalizationError::custody())?;
                    Ok::<_, LegalizationError>((element.index, scalar_type, element.stride))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if psi_operation != expected_operation
                || source != expected_source
                || path != expected_path
                || *byte_offset != expected_offset
                || indices.len() != expected_indices.len()
                || indices.iter().zip(&expected_indices).any(
                    |(index, (value, scalar_type, stride))| {
                        index.stride != *stride
                            || index.operand.source_value() != *value
                            || index.operand.scalar_type() != *scalar_type
                    },
                )
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || result_home.layout.shape() != expected_shape
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::EstablishCase {
                psi_operation,
                result_home,
                result_case,
                fields,
            },
            AbstractOperation::EstablishCase {
                psi_operation: expected_operation,
                result,
                result_case: expected_case,
                fields: expected_fields,
            },
        ) => {
            if psi_operation != expected_operation
                || result_case != expected_case
                || fields != expected_fields
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
            {
                return Err(LegalizationError::custody());
            }
            let declaration = plan
                .structural_type(result.structural_type)
                .ok_or(LegalizationError::custody())?;
            // A mixed case binds its runtime common fields, then the selected
            // payload, in the order Terminal verification checked.
            let (common, cases) = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                    (fields.as_slice(), cases)
                }
                _ => return Err(LegalizationError::custody()),
            };
            let case = cases
                .iter()
                .find(|case| case.id == *result_case)
                .ok_or(LegalizationError::custody())?;
            let roster = common
                .iter()
                .chain(&case.fields)
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if fields.len() != roster.len() {
                return Err(LegalizationError::custody());
            }
            for (field, declared) in fields.iter().zip(roster) {
                if field.field != declared.id || declared.relevance.is_erased() {
                    return Err(LegalizationError::custody());
                }
                match &field.value {
                    terminal_psi::RecordFieldValue::Scalar { value, range_obligation } => {
                        if !sources.iter().any(|(source_value, source)| *source_value == *value
                            && declared.field_type.scalar_type() == Some(source.scalar_type()))
                            || terminal_semantics::scalar_field_requires_evidence(&declared.field_type) != range_obligation.is_some()
                            || range_obligation.is_some_and(|obligation|
                                !unit.accepted_obligation_facts.iter().any(|fact| fact.machine == optimized.machine
                                    && fact.owner == optimization_unit::PsiProvenance::Operation(*psi_operation) && fact.obligation == obligation)
                                || !optimized.facts.iter().any(|fact| matches!(fact,
                                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: retained, support }
                                    if *retained == obligation && support == psi_operation)))
                        { return Err(LegalizationError::custody()); }
                    }
                    terminal_psi::RecordFieldValue::Structural(argument) => {
                        validate_owned_field_operand(
                            optimized, result.place, argument,
                            &declared.field_type, &unit.structural_types,
                        )?;
                    }
                }
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}

/// The receiving field's carrier is independent of its record/case namespace.
/// Rejoin the exact completed source before comparing physical storage; semantic
/// predicates and backing custody remain on the source operation's evidence.
fn validate_owned_field_operand(
    function: &PsiOptimizationFunction,
    destination: semantic_vocabulary::PlaceId,
    argument: &terminal_psi::StructuralArgument,
    field: &terminal_psi::StructuralFieldType,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    if argument.access != terminal_psi::StructuralAccess::Owned || argument.place == destination {
        return Err(LegalizationError::custody());
    }
    if let Some(expected) = field.as_structural() {
        let actual = owned_projected_field_source(function, argument, types)?;
        return if actual == expected {
            Ok(())
        } else {
            Err(LegalizationError::custody())
        };
    }
    if !argument.path.is_empty() {
        return Err(LegalizationError::custody());
    }
    if let Some(carrier) = field.byte_sequence_carrier()
        && carrier.is_borrowed()
    {
        // An inline field stores the same descriptor value as a structural
        // child. Rejoin its exact canonical carrier and ordinary value source;
        // readable versus exclusive backing never changes descriptor geometry.
        // Copy/transfer and permission remain the retained source judgments.
        let expected = terminal_semantics::byte_carrier_field_declaration(types.iter(), carrier)
            .ok_or(LegalizationError::custody())?;
        return if owned_projected_field_source(function, argument, types)? == expected.id {
            Ok(())
        } else {
            Err(LegalizationError::custody())
        };
    }
    // Whole byte operands also include literal descriptors and qualified shared
    // parameters. Read their common declaration/producer contract rather than
    // reconstructing another list of permitted constructor source shapes.
    let declaration = function
        .structural_place(argument.place)
        .ok_or(LegalizationError::custody())?;
    let actual = optimization_unit::declared_structural_type(function, declaration)
        .ok_or(LegalizationError::custody())?;
    let access = optimization_unit_semantics::structural_place_access(function, argument.place)
        .ok_or(LegalizationError::custody())?;
    let actual = &terminal_psi::find_structural_type(types, actual)
        .ok_or(LegalizationError::custody())?
        .shape;
    let compatible = match field.byte_sequence_carrier() {
        Some(expected @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => {
            access == terminal_psi::StructuralAccess::Owned
                && *actual == terminal_psi::StructuralTypeShape::ByteSequence(expected)
        }
        _ => false,
    };
    if !compatible {
        return Err(LegalizationError::custody());
    }
    Ok(())
}

/// Structural payloads retain their exact owned producer or incoming binding.
/// This source/target correspondence audit checks the selected storage, not
/// whether carrier disposal multiplicity grants copying. Shared references can
/// be copied even when their disposal occurrence is affine. Source ownership
/// and the independent reference audit retain permission and claim checks.
fn owned_field_source(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
    let parameters = function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .filter(|parameter| parameter.place == place)
        .map(|parameter| {
            (
                parameter.structural_type,
                parameter.access == terminal_psi::StructuralAccess::Owned,
            )
        });
    let result = scalar_graph_input::structural_case::source_result(function, place)
        .ok()
        .map(|(_, result)| (result.structural_type, true));
    let mut sources = parameters.chain(result);
    let (structural_type, owned) = sources.next().ok_or(LegalizationError::custody())?;
    // A completed owned value retains its claims in the home origin. Moving
    // its bytes into a field uses the checked constructor operand, not a new
    // claim-free-copy rule based on which aggregate receives it.
    if sources.next().is_some() || !owned {
        return Err(LegalizationError::custody());
    }
    Ok(structural_type)
}

pub(super) fn owned_projected_field_source(
    function: &PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
    // A descriptor is a value whose backing remains borrowed, whether it was
    // introduced by a parameter, literal, or completed operation. Rejoin the
    // common source contract and exact declared type, not a producer roster.
    // This identity query does not grant copying or mutable backing access;
    // source ownership and reference-custody replay retain those obligations.
    if argument.path.is_empty()
        && argument.access == terminal_psi::StructuralAccess::Owned
        && optimization_unit_semantics::structural_place_access(function, argument.place).is_some()
        && let Some(declaration) = function.structural_place(argument.place)
        && let Some(actual) = optimization_unit::declared_structural_type(function, declaration)
        && terminal_semantics::descriptor_results::is_descriptor(types.iter(), actual)
    {
        // This is source identity, not predicate establishment. Qualified
        // descriptor parameters retain their memberships and receiving proofs;
        // they do not become different pointer/extent layouts.
        return Ok(actual);
    }
    let root = owned_field_source(function, argument.place)?;
    if argument.path.is_empty() {
        return Ok(root);
    }
    if argument.access != terminal_psi::StructuralAccess::Owned {
        return Err(LegalizationError::custody());
    }
    let (_, selected) =
        terminal_semantics::canonical_static_projection(types.iter(), root, &argument.path)
            .ok_or(LegalizationError::custody())?;
    Ok(selected.id)
}
