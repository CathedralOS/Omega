//! Activation-local primitive backing and fresh storage observations.
use super::{KnownUnitInteger, LiveDefinitions};
use crate::lowering::function_signature::PreparedFunctionSignature;
use crate::lowering::shared::*;
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};

pub(super) fn is_primitive_reference(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    !parameter.is_self
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.access != StructuralAccess::Owned
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(scalar)
                if native_shape(scalar).is_some())
            })
}

pub(super) fn shape(
    scalar: abstract_operations::AbstractResult,
) -> Result<ValueShape, LoweringError> {
    native_shape(scalar.scalar_type).ok_or(LoweringError::ValueTypeMismatch(scalar.value))
}

fn native_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Integer(integer) => {
            crate::lowering::scalar_abi::fixed_native_integer_shape(integer)
        }
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Some(ValueShape::float(4)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Some(ValueShape::float(8)),
    }
}

pub(super) fn retain_result(
    operation: OperationId,
    result: abstract_operations::AbstractResult,
    live: &mut LiveDefinitions,
) -> Result<(), LoweringError> {
    let home = TargetUnitScalarHomeRequirement {
        defining_operation: operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: shape(result)?,
    };
    if matches!(result.scalar_type, ScalarType::Integer(_)) {
        crate::lowering::unit::scalar_call::insert_known_unit_integer(
            &mut live.integers,
            result.value,
            KnownUnitInteger::Home(home),
        )
    } else if live.scalar_homes.insert(result.value, home).is_some() {
        Err(LoweringError::DuplicateValue(result.value))
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    prepared: &PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let scalar_matches = |identity, scalar_type| {
        types.get(&identity).is_some_and(|declaration|
            matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(expected) if expected == scalar_type))
    };
    let (identity, lowered) = match operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
        } => {
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !scalar_matches(result.structural_type, value.scalar_type)
                || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                    != value.scalar_type
            {
                return Err(invalid());
            }
            let shape = shape(*value)?;
            if live
                .structural_homes
                .insert(
                    result.place,
                    TargetStructuralHomeRequirement {
                        defining_operation: *psi_operation,
                        result: result.clone(),
                        layout: TargetStructuralHomeLayout::Aggregate(shape),
                    },
                )
                .is_some()
            {
                return Err(invalid());
            }
            (
                *psi_operation,
                TargetUnitOperation::EstablishPrimitiveLocal {
                    psi_operation: *psi_operation,
                    result: result.clone(),
                    value: *value,
                    shape,
                },
            )
        }
        AbstractOperation::PrimitiveLocalStore {
            psi_operation,
            destination,
            value,
        } => {
            let home = live.structural_homes.get(destination).ok_or_else(invalid)?;
            if !scalar_matches(home.result.structural_type, value.scalar_type)
                || super::scalar_sources::source(value.value, function, live)?.scalar_type()
                    != value.scalar_type
            {
                return Err(invalid());
            }
            (
                *psi_operation,
                TargetUnitOperation::PrimitiveLocalStore {
                    psi_operation: *psi_operation,
                    destination: *destination,
                    value: *value,
                },
            )
        }
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
        } => {
            let identity = if let Some(home) = live.structural_homes.get(source) {
                home.result.structural_type
            } else {
                prepared
                    .parameters
                    .iter()
                    .find(|parameter| {
                        parameter.place == *source
                            && matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    })
                    .ok_or_else(invalid)?
                    .structural_type
            };
            if !scalar_matches(identity, result.scalar_type) {
                return Err(invalid());
            }
            retain_result(*psi_operation, *result, live)?;
            (
                *psi_operation,
                TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation: *psi_operation,
                    result: *result,
                    source: *source,
                },
            )
        }
        _ => return Err(invalid()),
    };
    operations.push(lowered);
    provenance.operations.push(identity);
    Ok(())
}
