//! Requirement and machine signatures share one argument-placement derivation.
//!
//! These invocation-local records contain no callee places. Direct declarations
//! and bodyless interface slots supply the same type/access facts; the selected
//! implementation never determines the caller's ABI.

use abstract_operations::{AbstractCallSignature, StructuralCallFormal};
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::ScalarType;
use target::NativeTarget;

use crate::LoweringError;
use crate::lowering::structural_signature::{StructuralCallPlan, StructuralCallSignature};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;

pub(super) struct PreparedScalarCallFormal {
    pub(super) scalar_type: ScalarType,
    pub(super) placement: ValuePlacement,
}

pub(super) struct PreparedStructuralCallFormal {
    pub(super) shape: ValueShape,
    pub(super) placement: ValuePlacement,
}

pub(super) struct PreparedCallSignature {
    pub(super) call_plan: StructuralCallPlan,
    pub(super) scalar_parameters: Vec<PreparedScalarCallFormal>,
    pub(super) parameters: Vec<PreparedStructuralCallFormal>,
}

pub(super) fn prepare(
    machine: semantic_vocabulary::MachineId,
    declaration: AbstractCallSignature<'_>,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<PreparedCallSignature, LoweringError> {
    let scalar_types = (0..declaration.scalar_parameter_count())
        .map(|position| {
            declaration
                .scalar_parameter(position)
                .ok_or(LoweringError::AbiParameterCountMismatch {
                    expected: declaration.scalar_parameter_count(),
                    actual: position,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scalar_shapes = scalar_types
        .iter()
        .map(|scalar_type| {
            crate::lowering::scalar_abi::fixed_native_scalar_shape(*scalar_type)
                .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let formals = (0..declaration.structural_parameter_count())
        .map(|position| {
            declaration
                .structural_parameter(position)
                .map(|parameter: StructuralCallFormal<'_>| {
                    (parameter.structural_type, parameter.access)
                })
                .ok_or(LoweringError::AbiParameterCountMismatch {
                    expected: declaration.structural_parameter_count(),
                    actual: position,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = if let Some(result) = declaration.structural_result() {
        Some(super::aggregate_results::value_home_layout(result.structural_type, types)?.shape())
    } else {
        declaration
            .scalar_result()
            .map(|scalar_type| {
                crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar_type)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
            })
            .transpose()?
    };
    let signature = StructuralCallSignature::derive(
        &scalar_shapes,
        formals,
        result,
        types,
        &mut std::collections::BTreeMap::new(),
        &mut std::collections::BTreeSet::new(),
    )?;
    let call_plan = signature.plan(target)?;
    let shapes = [scalar_shapes.as_slice(), signature.structural_shapes()].concat();
    if call_plan.parameters.len() != shapes.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: shapes.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let scalar_parameters = scalar_types
        .into_iter()
        .zip(&call_plan.parameters)
        .map(|(scalar_type, placement)| PreparedScalarCallFormal {
            scalar_type,
            placement: placement.clone(),
        })
        .collect();
    let parameters = shapes
        .into_iter()
        .zip(&call_plan.parameters)
        .skip(declaration.scalar_parameter_count())
        .map(|(shape, placement)| PreparedStructuralCallFormal {
            shape,
            placement: placement.clone(),
        })
        .collect();
    Ok(PreparedCallSignature {
        call_plan,
        scalar_parameters,
        parameters,
    })
}

impl From<crate::lowering::function_signature::PreparedFunctionSignature>
    for PreparedCallSignature
{
    fn from(signature: crate::lowering::function_signature::PreparedFunctionSignature) -> Self {
        Self {
            call_plan: signature.call_plan,
            scalar_parameters: signature
                .scalar_parameters
                .into_iter()
                .map(|parameter| PreparedScalarCallFormal {
                    scalar_type: parameter.scalar_type,
                    placement: parameter.placement,
                })
                .collect(),
            parameters: signature
                .parameters
                .into_iter()
                .map(|parameter| PreparedStructuralCallFormal {
                    shape: parameter.shape,
                    placement: parameter.placement,
                })
                .collect(),
        }
    }
}
