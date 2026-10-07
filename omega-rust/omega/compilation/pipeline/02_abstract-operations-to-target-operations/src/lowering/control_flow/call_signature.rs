//! Requirement and machine signatures share one argument-placement derivation.
//!
//! These invocation-local records contain no callee places. Direct declarations
//! and bodyless interface slots supply the same type/access facts; the selected
//! implementation never determines the caller's ABI.

use abstract_operations::{AbstractCallSignature, StructuralCallFormal};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValuePlacement, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::ScalarType;
use target::NativeTarget;

use crate::LoweringError;
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
    pub(super) call_plan: CallPlan,
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
    let mut shapes = scalar_types
        .iter()
        .map(|scalar_type| {
            crate::lowering::scalar_abi::fixed_native_scalar_shape(*scalar_type)
                .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut shape_cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    for position in 0..declaration.structural_parameter_count() {
        let parameter: StructuralCallFormal<'_> = declaration
            .structural_parameter(position)
            .ok_or(LoweringError::AbiParameterCountMismatch {
                expected: declaration.structural_parameter_count(),
                actual: position,
            })?;
        let referent = crate::lowering::structural_layout::structural_shape(
            parameter.structural_type,
            types,
            &mut shape_cache,
            &mut active,
        )?;
        shapes.push(
            crate::lowering::structural_layout::structural_parameter_shape(
                referent,
                parameter.access,
            ),
        );
    }
    let result = declaration
        .scalar_result()
        .map(|scalar_type| {
            crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar_type)
                .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
        })
        .transpose()?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: shapes.clone(),
            result,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
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
