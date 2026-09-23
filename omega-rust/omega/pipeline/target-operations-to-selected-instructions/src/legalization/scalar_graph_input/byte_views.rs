//! Exact borrowed structural headers for observations, writes and calls.
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, CallPlan,
    CallingPolicy, PsiOptimizationFunction, ScalarType, TargetFunction, ValueDefinitionSite,
    scalar_shape,
};
use crate::LegalizationError;

#[cfg(test)]
mod tests;

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
    let (call_plan, scalar_parameters, structural_parameters) =
        match (&abstracted.result, &target.mixed_structural_scalar_abi) {
            (AbstractFunctionResult::Unit, None) if target.graph.call_plan.result.is_none() => (
                &target.graph.call_plan,
                &target.graph.scalar_parameters,
                &target.graph.parameters,
            ),
            (AbstractFunctionResult::Scalar(result), Some(abi))
                if matches!(
                    result.scalar_type,
                    ScalarType::Boolean | ScalarType::Integer(_)
                ) && scalar_shape(result.scalar_type) == Some(abi.result.placement.shape)
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
        .map(
            |(semantic, target)| crate::structural_inputs::structural_unit_input::Parameter {
                semantic,
                target,
            },
        )
        .collect::<Vec<_>>();
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.scalar_abi.is_some()
        || (!crate::structural_inputs::structural_unit_input::accepts_borrowed_view(
            call_plan,
            &parameters,
            &plan.structural_types,
        ) && !crate::structural_inputs::structural_unit_input::accepts_borrowed_parameters(
            call_plan,
            &parameters,
            &plan.structural_types,
        ) && !crate::structural_inputs::structural_unit_input::accepts_shared_record(
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
        // Provider attachments are specialization witnesses, not declared
        // storage; they stay out of `declared_places` by contract (see
        // `reconstruct_declared_places` and `aggregate_results::roster`).
        || optimized.declared_places
            != optimized
                .structural_places
                .iter()
                .filter(|place| {
                    !matches!(
                        place.kind,
                        semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { .. }
                    )
                })
                .map(|place| place.id)
                .collect()
        || !optimized.structural_places.iter().all(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                optimized.attachment == Some(attachment)
            }
            _ => true,
        })
        || !(crate::structural_inputs::structural_unit_input::accepts_borrowed_view(
            call_plan,
            &parameters,
            &plan.structural_types,
        ) || crate::structural_inputs::structural_unit_input::accepts_borrowed_parameters(
            call_plan,
            &parameters,
            &plan.structural_types,
        ) || crate::structural_inputs::structural_unit_input::accepts_shared_record(
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
            }
            | AbstractOperation::ElementViewSubslice {
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
    let establishes = optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
            if let AbstractOperation::EstablishElementView {
                psi_operation,
                result,
                destination,
                ..
            } = &node.operation
            {
                Some((*psi_operation, result, *destination))
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
    if optimized
        .structural_places
        .iter()
        .filter(|place| {
            !matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { .. }
            )
        })
        .count()
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
                        AbstractOperation::BoundaryCall {
                            result: abstract_operations::AbstractBoundaryResult::Structural(_),
                            ..
                        }
                    )
                })
                .count()
            + optimized
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::EstablishPrimitiveLocal { .. }
                            | AbstractOperation::EstablishByteSequenceLiteral { .. }
                            | AbstractOperation::EstablishElementView { .. }
                    )
                })
                .count()
    {
        return Err(invalid);
    }
    for place in &optimized.structural_places {
        // Local immutable backing composes with borrowed inputs. Its exact
        // declaration/producer join is independent of the surrounding roster;
        // source-unit validation still owns dominance and availability.
        if super::literals::declaration_producer(optimized, place.id).is_some() {
            continue;
        }
        // Provider attachments are specialization witnesses, not declared
        // storage or runtime views. Unit custody checks their exact field,
        // boundary, and service authority; here they must name the function's
        // own attachment, matching `aggregate_results::roster`.
        if let semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { attachment, .. } =
            place.kind
        {
            if optimized.attachment == Some(attachment) {
                continue;
            }
            return Err(invalid);
        }
        if let Some((producer, result)) = super::structural_case::source_result(optimized, place.id)
            .ok()
            .filter(|(producer, _)| {
                // Element/byte-view producers admit through their own arms
                // below; the conventional byte-read layout applies only to
                // boundary results.
                !subslices.iter().any(|(operation, _)| operation == producer)
                    && !establishes
                        .iter()
                        .any(|(operation, _, _)| operation == producer)
            })
        {
            // A borrowed-view activation may also own a completed boundary
            // result. Keep its producer/type custody separate from descriptors;
            // target replay still validates the exact selected settlement.
            super::read_byte::layout(result, plan)?;
            if place.kind
                != (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                })
            {
                return Err(invalid);
            }
            continue;
        }
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
                        && abstracted.result == AbstractFunctionResult::Unit
                        && plan.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                                    ) | terminal_psi::StructuralTypeShape::ElementView { .. }
                                )
                        })))
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && !parameter.is_self
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        }) {
            continue;
        }
        if !(subslices.iter().any(|(operation, result)| {
            place.id == result.place
                && plan.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView,
                            ) | terminal_psi::StructuralTypeShape::ElementView { .. }
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
        }) || establishes.iter().any(|(operation, result, destination)| {
            place.id == *destination
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && place.kind
                    == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: *operation,
                        structural_type: result.structural_type,
                    })
        })) {
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
// restricts the native route to parameters and derived immutable views.
pub(super) fn contains_view(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    super::literals::declaration_producer(function, place).is_some()
        || function.structural_parameters.iter().any(|parameter| parameter.place == place)
        || function.blocks.iter().any(|block| block.structural_parameters.iter().any(|parameter| parameter.place == place))
        || function.blocks.iter().flat_map(|block| &block.nodes).any(|node|
            matches!(&node.operation, AbstractOperation::ByteSequenceSubslice { result, .. } | AbstractOperation::ElementViewSubslice { result, .. } if result.place == place)
            || matches!(&node.operation, AbstractOperation::EstablishElementView { destination, .. } if *destination == place))
}
