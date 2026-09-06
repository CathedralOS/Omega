//! Whole-input affine identity calls inside a Unit body.

use super::super::shared::*;
use super::super::structural::require_direct_structural_fragments;
use super::super::structural_layout::structural_shape;
use super::scalar_call::KnownUnitInteger;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_structural_result_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallStructural {
        psi_operation,
        result,
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
    } = operation
    else {
        unreachable!("structural-result Unit lowering receives only its exact role")
    };
    let [source_argument] = structural_arguments.as_slice() else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let source_parameter = parameters_by_place
        .get(&source_argument.place)
        .copied()
        .ok_or(LoweringError::UnknownStructuralArgumentPlace {
            machine: function.machine,
            place: source_argument.place,
        })?;
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let ([callee_structural], [callee_operation]) = (
        callee_function.structural_parameters.as_slice(),
        callee_function.operations.as_slice(),
    ) else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let Some(callee_result) = callee_function.result.structural() else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let AbstractOperation::ReturnStructural {
        source: callee_return_source,
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
        ..
    } = callee_operation
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let scalar = match (arguments.as_slice(), callee_function.parameters.as_slice()) {
        ([], []) => None,
        ([source_value], [callee_scalar]) => {
            let ScalarType::Integer(callee_scalar_type) = callee_scalar.scalar_type else {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            };
            let Some(shape) =
                super::super::scalar_abi::fixed_native_integer_shape(callee_scalar_type)
            else {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            };
            let known = scalar_values
                .get(source_value)
                .copied()
                .ok_or(LoweringError::UnknownValue(*source_value))?;
            if known.scalar_type() != callee_scalar_type {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            Some((*source_value, known, shape))
        }
        _ => {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
    };
    let result_declaration = structural_types
        .get(&result.structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(result.structural_type))?;
    let supported_shape = if scalar.is_none() {
        matches!(
            &result_declaration.shape,
            StructuralTypeShape::Record { .. }
                | StructuralTypeShape::FixedArray { length: 1.., .. }
        )
    } else {
        matches!(
            &result_declaration.shape,
            StructuralTypeShape::Record { fields }
                if matches!(
                    fields.as_slice(),
                    [field]
                        if matches!(
                            field.field_type,
                            StructuralFieldType::Scalar(ScalarType::Integer(integer))
                                if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                    && integer.bits() == 64
                        )
                )
        )
    };
    let aggregate_shape = structural_shape(
        result.structural_type,
        structural_types,
        shape_cache,
        active,
    )?;
    if !supported_shape
        || (scalar.is_some() && aggregate_shape != ValueShape::integer(8, 8))
        || !function.published_service_ceiling.is_empty()
        || result.place == source_argument.place
        || callee_result.place == callee_structural.place
        || source_parameter.structural_type != result.structural_type
        || source_parameter.shape != aggregate_shape
        || source_parameter.multiplicity != StructuralMultiplicity::Affine
        || source_parameter.access != StructuralAccess::Owned
        || !source_parameter.projected_qualifications.is_empty()
        || !source_argument.path.is_empty()
        || source_argument.access != StructuralAccess::Owned
        || callee_structural.position != 0
        || callee_structural.is_self
        || callee_structural.structural_type != result.structural_type
        || callee_structural.multiplicity != StructuralMultiplicity::Affine
        || callee_structural.access != StructuralAccess::Owned
        || !callee_structural.qualifications.is_empty()
        || !callee_structural.projected_qualifications.is_empty()
        || *callee_return_source != callee_structural.place
        || callee_result.structural_type != result.structural_type
        || callee_result.multiplicity != StructuralMultiplicity::Affine
        || !callee_result.qualifications.is_empty()
        || !callee_result.projected_qualifications.is_empty()
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !callee_function.entry_claims.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || !returned_claims.is_empty()
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !selected_evidence.is_empty()
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar
                .iter()
                .map(|(_, _, shape)| *shape)
                .chain(std::iter::once(aggregate_shape))
                .collect(),
            result: Some(aggregate_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let source_index = usize::from(scalar.is_some());
    if call_plan.parameters.len() != source_index + 1 {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: source_index + 1,
            actual: call_plan.parameters.len(),
        });
    }
    require_direct_structural_fragments(function.machine, &call_plan.parameters[source_index])?;
    let result_placement = call_plan
        .result
        .as_ref()
        .ok_or(LoweringError::UnsupportedStructuralReturn(function.machine))?;
    require_direct_structural_fragments(function.machine, result_placement)?;
    let needs_home = function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallUnit { structural_arguments, .. }
            if structural_arguments.iter().any(|argument| argument.place == result.place
                && !argument.path.is_empty()))
    });
    if needs_home
        && (scalar.is_some()
            || !matches!(
                aggregate_shape,
                ValueShape {
                    byte_size: 8 | 16,
                    alignment: 8,
                    class: ValueClass::Integer
                }
            )
            || !operations.is_empty())
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let result_home = needs_home.then(|| target_operations::TargetStructuralHomeRequirement {
        defining_operation: *psi_operation,
        result: result.clone(),
        layout: target_operations::TargetStructuralHomeLayout::Aggregate(aggregate_shape),
    });
    operations.push(TargetUnitOperation::StructuralResultCall {
        psi_operation: *psi_operation,
        result: result.clone(),
        callee: *callee,
        callee_result: callee_result.clone(),
        result_home,
        call_plan: call_plan.clone(),
        scalar_arguments: scalar
            .into_iter()
            .map(|(source_value, known, _)| TargetUnitScalarCallArgument {
                parameter_index: 0,
                source: known.into_target_source(source_value),
                placement: call_plan.parameters[0].clone(),
            })
            .collect(),
        arguments: vec![TargetStructuralArgument {
            place: source_argument.place,
            access: source_argument.access,
            path: Vec::new(),
            root_structural_type: source_parameter.structural_type,
            structural_type: source_parameter.structural_type,
            shape: aggregate_shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: source_parameter.placement.clone(),
            destination: call_plan.parameters[source_index].clone(),
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
