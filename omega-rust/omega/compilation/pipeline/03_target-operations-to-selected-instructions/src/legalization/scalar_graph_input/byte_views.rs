//! Exact borrowed structural headers for observations, writes and calls.
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, CallPlan,
    CallingPolicy, PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, scalar_shape,
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
    let (call_plan, scalar_parameters, structural_parameters) =
        match (&abstracted.result, &target.mixed_structural_scalar_abi) {
            (AbstractFunctionResult::Unit, None) if target.graph.call_plan.result.is_none() => (
                &target.graph.call_plan,
                &target.graph.scalar_parameters,
                &target.graph.parameters,
            ),
            (AbstractFunctionResult::Scalar(result), Some(abi))
                if scalar_shape(result.scalar_type) == Some(abi.result.placement.shape)
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
            _ => return Err(LegalizationError::custody()),
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
        || !crate::structural_inputs::structural_unit_input::accepts_graph(
            call_plan,
            &parameters,
            &plan.structural_types,
        )
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
        // Structural declarations compose independently of this byte-view ABI:
        // one shared inventory rejoins every parameter, local and result to its
        // exact producer. Operation replay owns each result's meaning.
        || !super::place_declarations::matches_producers(optimized)
    {
        return Err(LegalizationError::custody());
    }
    if abstracted
        .structural_parameters
        .iter()
        .any(|parameter| parameter.is_self && target.attachment != Some(parameter.structural_type))
    {
        return Err(LegalizationError::custody());
    }
    if optimized
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| {
            // State arrivals choose their own storage from declared access and
            // layout, independently of the entry's borrowed ABI. Exact incoming
            // bindings and ownership are replayed by target control flow; owned
            // arrivals use the ordinary structural block-parameter homes.
            crate::structural_inputs::structural_reference_input::parameter_shape(
                parameter,
                &plan.structural_types,
            )
            .is_none()
                || (parameter.is_self && target.attachment != Some(parameter.structural_type))
        })
    {
        return Err(LegalizationError::custody());
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

/// Reconstruct the declared element from the input's own view producer or
/// parameter, never from a target expression's claimed layout.
pub(super) fn element_type(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let view = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find_map(|node| match &node.operation {
                    AbstractOperation::EstablishElementView { result, .. }
                    | AbstractOperation::ElementViewSubslice { result, .. }
                        if result.place == place =>
                    {
                        Some(result.structural_type)
                    }
                    _ => None,
                })
        })?;
    match types
        .iter()
        .find(|declaration| declaration.id == view)?
        .shape
    {
        terminal_psi::StructuralTypeShape::ElementView { element } => Some(element),
        _ => None,
    }
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

pub(in crate::legalization) fn owned_byte_result(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralTypeId,
)> {
    let (producer, result) = super::structural_case::source_result(function, place).ok()?;
    (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        }))
    .then_some((producer, result.structural_type))
}
/// Whether a byte place reads the owner layout (length word at offset 0,
/// bytes from offset 8): a completed owned byte result in this activation, or a
/// bounded owner arrival. Access controls observations, not the field offsets.
pub(in crate::legalization) fn reads_owner_layout(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    owned_byte_result(function, place, types).is_some()
        || function.arrival_parameter(place).is_some_and(|parameter| {
            types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
        })
}

pub(super) fn contains_readable_bytes(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    contains_view(function, place) || owned_byte_result(function, place, types).is_some()
}
