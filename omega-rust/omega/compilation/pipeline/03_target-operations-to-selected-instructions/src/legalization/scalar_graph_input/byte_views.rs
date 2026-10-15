//! Exact borrowed structural headers for observations, writes and calls.
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperationPlan, CallPlan, CallingPolicy,
    PsiOptimizationFunction, TargetFunction, ValueDefinitionSite, scalar_shape,
};
use crate::LegalizationError;

pub(super) fn validate(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
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
        || (target.attachment != abstracted.attachment || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
        || (target.attachment != optimized.attachment || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
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
    if optimized.block_structural_parameters().any(|parameter| {
        // State arrivals choose their own storage from declared access and
        // layout, independently of the entry's borrowed ABI. Exact incoming
        // bindings and ownership are replayed by target control flow; owned
        // arrivals use the ordinary structural block-parameter homes.
        crate::structural_inputs::structural_reference_input::parameter_shape(
            parameter,
            &plan.structural_types,
        )
        .is_none()
    }) {
        return Err(LegalizationError::custody());
    }
    Ok(call_plan.clone())
}

/// A write needs a live byte home, not a particular establishment family.
/// Resolve the declaration and actual access independently: an owned bounded
/// home and a mutable byte descriptor both write their original backing.
/// Predicate preservation and currentness remain the operation's obligations.
pub(super) fn writable_bytes(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let (structural_type, multiplicity, projected) =
        if let Some(parameter) = function.arrival_parameter(place) {
            (
                parameter.structural_type,
                parameter.multiplicity,
                &parameter.projected_qualifications,
            )
        } else {
            let Ok((_, result)) = super::structural_case::source_result(function, place) else {
                return false;
            };
            if !result.claims.is_empty() || !result.qualification_establishments.is_empty() {
                return false;
            }
            (
                result.structural_type,
                result.multiplicity,
                &result.projected_qualifications,
            )
        };
    if !projected.is_empty() {
        return false;
    }
    let Some(declaration) = terminal_psi::find_structural_type(types, structural_type) else {
        return false;
    };
    match optimization_unit_semantics::structural_place_backing_access(
        function,
        place,
        types.iter(),
    ) {
        Some(terminal_psi::StructuralAccess::MutableBorrow) => {
            // A locally derived mutable descriptor is affine; an incoming
            // loan can be unrestricted. Neither multiplicity grants access:
            // the shared lineage query above must establish MutableBorrow.
            matches!(
                multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            ) && matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ByteSequence(_)
            )
        }
        Some(terminal_psi::StructuralAccess::Owned) => {
            multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && declaration.shape.is_bounded_owned_byte_sequence()
        }
        _ => false,
    }
}

/// Geometry follows the exact current declaration. Producer contracts,
/// dominance and permissions are independently checked by operation replay;
/// the shape query must not recognize a second list of source families.
pub(super) fn view_type(
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
    let declaration = terminal_psi::find_structural_type(types, identity)?;
    (declaration.shape.is_borrowed_byte_carrier()
        || matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::ElementView { .. }
        ))
    .then_some(identity)
}

/// Return the descriptor value, independent of the producer's storage family.
/// This joins the exact declaration and empty custody; whole-program descriptor
/// origin validation separately proves that the backing survives the return.
pub(in crate::legalization) fn descriptor_return(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let Some(declared) = function.result.structural() else {
        return false;
    };
    if view_type(function, place, types) != Some(declared.structural_type)
        || !matches!(
            declared.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        || !terminal_semantics::view_descriptor_projected_qualifications(
            types.iter(),
            declared.structural_type,
            &declared.projected_qualifications,
        )
        || !terminal_semantics::descriptor_results::is_descriptor(
            types.iter(),
            declared.structural_type,
        )
    {
        return false;
    }
    if let Some(parameter) = function.arrival_parameter(place) {
        return matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) && (declared.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || parameter.access != terminal_psi::StructuralAccess::SharedBorrow)
            && (parameter.multiplicity == declared.multiplicity
                || declared.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                    && parameter.multiplicity
                        == terminal_psi::StructuralMultiplicity::Unrestricted)
            && terminal_semantics::view_descriptor_projected_qualifications(
                types.iter(),
                parameter.structural_type,
                &parameter.projected_qualifications,
            );
    }
    if let Ok((_, result)) = super::structural_case::source_result(function, place) {
        return result.multiplicity == declared.multiplicity
            && result.claims.is_empty()
            && terminal_semantics::view_descriptor_projected_qualifications(
                types.iter(),
                result.structural_type,
                &result.projected_qualifications,
            )
            && result.qualification_establishments.is_empty();
    }
    declared.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && super::literals::declaration_producer(function, place).is_some()
}

pub(in crate::legalization) fn element_type(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let identity = view_type(function, place, types)?;
    match terminal_psi::find_structural_type(types, identity)?.shape {
        terminal_psi::StructuralTypeShape::ElementView { element, .. } => Some(element),
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
                && declaration.shape.is_bounded_owned_byte_sequence()
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
                    && declaration.shape.is_bounded_owned_byte_sequence()
            })
        })
}

/// Readable byte storage: a descriptor view, a bounded view over a fixed
/// array (`&[u8; N]`, a bare pointer whose length target lowering supplies
/// as the static capacity), or an owner read through its layout.
pub(super) fn contains_readable_bytes(
    function: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    view_type(function, place, types).is_some_and(|identity| {
        types
            .iter()
            .any(|row| row.id == identity && row.shape.is_borrowed_byte_carrier())
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
    (matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) && terminal_semantics::view_descriptor_projected_qualifications(
        types.iter(),
        result.structural_type,
        &result.projected_qualifications,
    ) && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && terminal_semantics::descriptor_results::is_descriptor(
            types.iter(),
            result.structural_type,
        )
        && function.block_nodes().any(|node| {
            let materialized = node.operation.materialized_structural_result();
            materialized == Some((producer, result))
        }))
    .then_some((producer, result.structural_type))
}

/// Derive the current call's complete backing roster, rather than trusting the
/// physical result home or a signature-only lifetime claim.
pub(in crate::legalization) fn call_descriptor_origins(
    unit: &optimization_unit::PsiOptimizationUnit,
    caller: semantic_vocabulary::MachineId,
    result: &terminal_psi::StructuralOperationResult,
    native: &target_operations::TargetOperationPlan,
    plan: &abstract_operations::AbstractOperationPlan,
) -> Result<Vec<legalized_operations::LegalizedDescriptorOrigin>, LegalizationError> {
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) {
        return Ok(Vec::new());
    }
    let inventory = terminal_semantics::descriptor_results::descriptor_slot_inventory(
        &unit.structural_types,
        result.place,
        result.structural_type,
    )
    .ok_or(LegalizationError::custody())?;
    // Stored references have their own target-producer origin rows. No known
    // byte/element descriptors means there is nothing to add to that roster;
    // it does not assert that opaque referents are view-free. When descriptors
    // are present, their complete result summary must remain available.
    if inventory.slots.is_empty() {
        return Ok(Vec::new());
    }
    if !inventory.opaque_referents.is_empty() {
        return Err(LegalizationError::custody());
    }
    // Source and installed calls share one origin replay over current bodies.
    // Authenticate selection with the existing call-origin join before exposing
    // its callee; unresolved foreign calls retain their boundary semantics.
    let graph = optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        |function, node| {
            super::call_origin::selected_boundary_operation(node, function, native, plan)
                .ok()
                .map(|selected| match selected {
                    Some((operation, _)) => std::borrow::Cow::Owned(operation),
                    None => std::borrow::Cow::Borrowed(&node.operation),
                })
        },
    )
    .ok_or(LegalizationError::custody())?;
    terminal_semantics::descriptor_results::reconstruct_call_origins(
        &unit.structural_types,
        &graph,
        caller,
        result.place,
    )
    .ok_or(LegalizationError::custody())?
    .into_iter()
    .map(|origin| match origin {
        terminal_semantics::descriptor_results::DescriptorCallOrigin::PlacedFormation {
            ..
        } => Err(LegalizationError::custody()),
        terminal_semantics::descriptor_results::DescriptorCallOrigin::Empty => {
            Ok(legalized_operations::LegalizedDescriptorOrigin::Empty)
        }
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
