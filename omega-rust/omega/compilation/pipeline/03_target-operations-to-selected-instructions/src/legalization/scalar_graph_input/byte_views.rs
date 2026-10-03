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

/// Geometry follows the exact current declaration. Producer contracts,
/// dominance and permissions are independently checked by operation replay;
/// the shape query must not recognize a second list of source families.
fn view_type(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let identity = function
        .arrival_parameter(place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            super::structural_case::source_result(function, place)
                .ok()
                .map(|(_, result)| result.structural_type)
        })
        .or_else(|| {
            super::literals::declaration_producer(function, place).map(|(_, identity)| identity)
        })?;
    let declaration = types.iter().find(|row| row.id == identity)?;
    (declaration.shape.is_borrowed_byte_view()
        || matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::ElementView { .. }
        ))
    .then_some(identity)
}

/// Return the descriptor value, independent of the producer's storage family.
/// This joins the exact declaration and empty custody; whole-program descriptor
/// origin validation separately proves that the backing survives the return.
pub(in crate::legalization) fn shared_return(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let Some(declared) = function.result.structural() else {
        return false;
    };
    if view_type(function, place, types) != Some(declared.structural_type)
        || declared.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !declared.qualifications.is_empty()
        || !declared.projected_qualifications.is_empty()
        || !terminal_semantics::descriptor_results::is_shared_descriptor(
            types.iter(),
            declared.structural_type,
        )
    {
        return false;
    }
    if let Some(parameter) = function.arrival_parameter(place) {
        return matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::SharedBorrow
        ) && parameter.multiplicity == declared.multiplicity
            && !parameter.is_self
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty();
    }
    if let Ok((_, result)) = super::structural_case::source_result(function, place) {
        return result.multiplicity == declared.multiplicity
            && result.claims.is_empty()
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty();
    }
    super::literals::declaration_producer(function, place).is_some()
}

pub(super) fn element_type(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let identity = view_type(function, place, types)?;
    match types.iter().find(|row| row.id == identity)?.shape {
        terminal_psi::StructuralTypeShape::ElementView { element } => Some(element),
        _ => None,
    }
}

pub(super) fn contains_view(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    view_type(function, place, types).is_some()
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
    view_type(function, place, types).is_some_and(|identity| {
        types
            .iter()
            .any(|row| row.id == identity && row.shape.is_borrowed_byte_view())
    }) || reads_owner_layout(function, place, types)
}

/// Exact completed descriptor home; it carries no ownership of borrowed bytes.
pub(in crate::legalization) fn descriptor_result(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralTypeId,
)> {
    let (producer, result) = super::structural_case::source_result(function, place).ok()?;
    (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && terminal_semantics::descriptor_results::is_shared_descriptor(types.iter(), result.structural_type)
        && function.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
            AbstractOperation::CallStructural { psi_operation, result: abstract_operations::AbstractStructuralCallResult::Structural { result: actual, .. }, .. }
                if *psi_operation == producer && actual == result)))
        .then_some((producer, result.structural_type))
}

/// Derive the current call's complete backing roster, rather than trusting the
/// physical result home or a signature-only lifetime claim.
pub(in crate::legalization) fn call_descriptor_origins(
    unit: &optimization_unit::PsiOptimizationUnit,
    caller: semantic_vocabulary::MachineId,
    result: &terminal_psi::StructuralOperationResult,
) -> Result<Vec<legalized_operations::LegalizedDescriptorOrigin>, LegalizationError> {
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !terminal_semantics::descriptor_results::is_shared_descriptor(
            unit.structural_types.iter(),
            result.structural_type,
        )
    {
        return Ok(Vec::new());
    }
    optimization_unit_semantics::descriptor_results::call_origins(unit, caller, result.place)
        .ok_or_else(LegalizationError::custody)?
        .into_iter()
        .map(|origin| match origin {
            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument {
                source,
                projection,
            } => {
                let projection = projection.without_runtime_coordinates();
                Ok(legalized_operations::LegalizedDescriptorOrigin::Argument {
                    source: terminal_psi::StructuralArgument {
                        place: source,
                        path: projection.path,
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                    exact: projection.exact,
                    indirect: projection.indirect,
                })
            }
            terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal {
                machine,
                place,
            } => Ok(legalized_operations::LegalizedDescriptorOrigin::Literal { machine, place }),
        })
        .collect()
}
