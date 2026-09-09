//! Exact borrowed structural headers for observations, writes and calls.
use super::*;

pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    if abstracted
        .structural_parameters
        .iter()
        .any(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
    {
        return super::unobserved_owned::validate(target, abstracted, optimized, native, plan);
    }
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (call_plan, scalar_parameters, structural_parameters) = match (
        &abstracted.result,
        &target.operation,
        &target.mixed_structural_scalar_abi,
    ) {
        (AbstractFunctionResult::Unit, TargetOperation::UnitBody(body), None)
            if optimized.blocks.len() == 1 && body.call_plan.result.is_none() =>
        {
            (&body.call_plan, &body.scalar_parameters, &body.parameters)
        }
        (AbstractFunctionResult::Unit, TargetOperation::ControlGraph(graph), None)
            if graph.call_plan.result.is_none() =>
        {
            (
                &graph.call_plan,
                &graph.scalar_parameters,
                &graph.parameters,
            )
        }
        (AbstractFunctionResult::Scalar(result), _, Some(abi))
            if result.scalar_type == ScalarType::Integer(u64_type())
                && abi.result.value == result.value
                && abi.result.scalar_type == result.scalar_type
                && Some(&abi.result.placement) == abi.call_plan.result.as_ref() =>
        {
            (
                &abi.call_plan,
                &abi.scalar_parameters,
                &abi.structural_parameters,
            )
        }
        _ => return Err(invalid),
    };
    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(structural_parameters)
        .map(|(semantic, target)| crate::structural_unit_input::Parameter { semantic, target })
        .collect::<Vec<_>>();
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.scalar_abi.is_some()
        || (matches!(target.operation, TargetOperation::ControlGraph(_))
            && !crate::structural_unit_input::accepts_borrowed_view(
                call_plan,
                &parameters,
                &plan.structural_types,
            )
            && !crate::structural_unit_input::accepts_write_borrow(
                call_plan,
                &parameters,
                &plan.structural_types,
            ))
        || abstracted.parameters.len() != optimized.parameters.len()
        || scalar_parameters.len() != abstracted.parameters.len()
        || call_plan.parameters.len()
            != abstracted.parameters.len() + abstracted.structural_parameters.len()
        || scalar_parameters
            .iter()
            .zip(&abstracted.parameters)
            .zip(&optimized.parameters)
            .zip(&call_plan.parameters)
            .enumerate()
            .any(|(position, (((actual, declared), optimized), placement))| {
                actual.value != declared.value
                    || scalar_shape(actual.scalar_type).is_none()
                    || declared.scalar_type != actual.scalar_type
                    || optimized.value != declared.value
                    || optimized.scalar_type != declared.scalar_type
                    || optimized.site != ValueDefinitionSite::FunctionParameter(position as u32)
                    || scalar_shape(actual.scalar_type) != Some(placement.shape)
                    || actual.placement != *placement
            })
        || structural_parameters.is_empty()
        || structural_parameters.len() != abstracted.structural_parameters.len()
        || optimized.structural_parameters != abstracted.structural_parameters
        || optimized.result != abstracted.result
        || call_plan.policy != CallingPolicy::native_for_target(native)
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || optimized.declared_places
            != optimized
                .structural_places
                .iter()
                .map(|place| place.id)
                .collect()
        || !(crate::structural_unit_input::accepts_borrowed_view(
            call_plan,
            &parameters,
            &plan.structural_types,
        ) || crate::structural_unit_input::accepts_write_borrow(
            call_plan,
            &parameters,
            &plan.structural_types,
        ))
    {
        return Err(invalid);
    }
    if abstracted
        .structural_parameters
        .iter()
        .any(|parameter| parameter.is_self && target.attachment != Some(parameter.structural_type))
    {
        return Err(invalid);
    }
    let subslices = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
            if let AbstractOperation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            } = &node.operation
            {
                Some((*psi_operation, result))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let block_parameters = optimized
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(move |parameter| (block.id, parameter))
        })
        .collect::<Vec<_>>();
    if optimized.structural_places.len()
        != abstracted.structural_parameters.len()
            + block_parameters.len()
            + subslices.len()
            + optimized
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::EstablishPrimitiveLocal { .. }
                    )
                })
                .count()
    {
        return Err(invalid);
    }
    for place in &optimized.structural_places {
        if let Some((operation, result, _)) = super::primitive_locals::producer(optimized, place.id)
        {
            if !super::primitive_locals::valid_result(optimized, operation, result) {
                return Err(invalid);
            }
            continue;
        }
        if abstracted.structural_parameters.iter().any(|parameter| {
            place.id == parameter.place
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    })
        }) {
            continue;
        }
        if block_parameters.iter().any(|(block, parameter)| {
            place.id == parameter.place
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block: *block,
                        position: parameter.position,
                    })
                && (parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                    || (parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                        && abstracted.result == AbstractFunctionResult::Unit))
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && !parameter.is_self
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && plan.structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && declaration.shape
                            == terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView,
                            )
                })
        }) {
            continue;
        }
        if !subslices.iter().any(|(operation, result)| {
            place.id == result.place
                && plan.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && declaration.shape
                            == terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView,
                            )
                })
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: *operation,
                        structural_type: result.structural_type,
                    })
        }) {
            return Err(invalid);
        }
    }
    Ok(call_plan.clone())
}

pub(super) fn mutable_parameter(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| {
            parameter.place == place
                && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && !parameter.is_self
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
}

// Whole-unit validation checks the exact producer and dominance. This predicate
// restricts the native route to parameters and derived immutable byte views.
pub(super) fn contains_view(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    function.structural_parameters.iter().any(|parameter| parameter.place == place)
        || function.blocks.iter().any(|block| block.structural_parameters.iter().any(|parameter| parameter.place == place))
        || function.blocks.iter().flat_map(|block| &block.nodes).any(|node|
            matches!(&node.operation, AbstractOperation::ByteSequenceSubslice { result, .. } if result.place == place))
}
