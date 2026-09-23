//! Independent aggregate layout and ordinary graph signature custody.
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, CallPlan,
    CallSignature, CallingPolicy, PsiOptimizationFunction, ScalarType, TargetFunction,
    TargetOperationPlan, ValueDefinitionSite, ValueShape, evaluate_call_plan, scalar_shape,
};
use crate::LegalizationError;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape};

mod borrowed_arguments;
mod owned_arguments;

pub(super) fn uses(function: &PsiOptimizationFunction, plan: &AbstractOperationPlan) -> bool {
    // Forwarding an incoming owned argument observes its payload even without
    // a local constructor or aggregate result. It needs aggregate ABI replay,
    // not the unused-owned-input path that deliberately emits no transport.
    function.result.structural().is_some()
        // Whole owned values retain the complete graph ABI even when unused.
        // Their signature must not depend on which scalar operations run beside them.
        || function.structural_parameters.iter().any(|parameter| {
            parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
                    parameter.structural_type,
                    &plan.structural_types,
                ).is_some()
        })
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::StructuralCaseMembership { .. }
                        | AbstractOperation::IntegerStructuralField { .. }
                        | AbstractOperation::StructuralByteSequenceFieldLength { .. }
                        | AbstractOperation::BooleanStructuralField { .. }
                        | AbstractOperation::EstablishScalarArray { .. }
                        | AbstractOperation::EstablishRecord { .. }
                        | AbstractOperation::EstablishReference { .. }
                        | AbstractOperation::ReleaseReference { .. }
                        | AbstractOperation::EstablishScalarCase { .. }
                        | AbstractOperation::StructuralLeafCopy { .. }
                        | AbstractOperation::CallStructural { .. }
                        | AbstractOperation::BoundaryCall { result: abstract_operations::AbstractBoundaryResult::Structural(_), .. }
                ) || matches!(&node.operation,
                    AbstractOperation::CallStructuralScalar { structural_arguments, .. }
                        | AbstractOperation::CallUnit { structural_arguments, .. }
                        if structural_arguments.iter().any(|argument|
                            argument.access == terminal_psi::StructuralAccess::Owned))
                    // Dispatching on an incoming parameter reads its tag and
                    // payloads, as a membership observation reads its tag.
                    || matches!(&node.operation,
                    AbstractOperation::StructuralCase { source, .. }
                        if function.structural_parameters.iter().any(|parameter| parameter.place == *source))
            })
}

pub(in crate::legalization) fn sum_layout(
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    sum_type_layout(result.structural_type, plan)
}

pub(in crate::legalization) fn sum_type_layout(
    structural_type: semantic_vocabulary::StructuralTypeId,
    plan: &AbstractOperationPlan,
) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let mut declarations = plan
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = declarations.next().ok_or(invalid.clone())?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid);
    };
    if declarations.next().is_some() {
        return Err(invalid);
    }
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .map(|field| {
                    if field.relevance.is_erased() {
                        return Err(invalid.clone());
                    }
                    let Some(ScalarType::Integer(integer)) = field.field_type.scalar_type() else {
                        return Err(invalid.clone());
                    };
                    scalar_shape(ScalarType::Integer(integer)).ok_or(invalid.clone())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).map_err(|_| invalid)
}

pub(super) fn roster(function: &PsiOptimizationFunction) -> bool {
    function.declared_places
        == function
            .structural_places
            .iter()
            .filter(|place| !matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            .map(|place| place.id)
            .collect::<std::collections::BTreeSet<_>>()
        && function.structural_places.iter().all(|place| {
            // Provider attachments are semantic specialization witnesses, not
            // receiver storage. Whole-unit custody checks the exact erased
            // field, boundary and service; they must compose with runtime
            // parameters and results without entering the declared-place set.
            if let StructuralPlaceKind::ProviderAttachment { attachment, .. } = place.kind {
                return function.attachment == Some(attachment);
            }
            if matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }) {
                return super::literals::declaration_producer(function, place.id).is_some();
            }
            // One graph may own primitive storage alongside aggregate results.
            // Keep each place joined to its exact producer; the primitive input
            // validator separately checks the declared type and initializer.
            if let Some((producer, result, _)) =
                super::primitive_locals::producer(function, place.id)
            {
                return place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    }
                    && super::primitive_locals::valid_result(function, producer, result);
            }
            if place.kind == StructuralPlaceKind::Result {
                return function
                    .result
                    .structural()
                    .is_some_and(|result| result.place == place.id);
            }
            if let Ok((producer, result)) =
                super::structural_case::source_result(function, place.id)
            {
                return place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    };
            }
            function.structural_parameters.iter().any(|parameter| {
                parameter.place == place.id
                    && place.kind
                        == StructuralPlaceKind::Parameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
                        }
            }) || function.blocks.iter().any(|block| {
                block.structural_parameters.iter().any(|parameter| {
                    parameter.place == place.id
                        && place.kind
                            == StructuralPlaceKind::BlockParameter {
                                block: block.id,
                                position: parameter.position,
                            }
                })
            })
        })
}

pub(super) fn cleanup(
    function: &PsiOptimizationFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> bool {
    let mut discarded = std::collections::BTreeSet::new();
    let mut residuals = std::collections::BTreeSet::new();
    let eligible_owner = |place: semantic_vocabulary::PlaceId| {
        if let Some(parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
        {
            // Exact source cleanup and the current ownership frontier are
            // checked independently. Observing a plain owned input does not
            // turn its no-code discard into a constructor-result requirement.
            return parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && function
                    .entry_claim_declarations
                    .iter()
                    .all(|claim| claim.input != place)
                && function
                    .content_entry_claims
                    .iter()
                    .all(|claim| claim.input.root != place);
        }
        // Selection transfers a fresh owner into a block parameter. Its final
        // discard owes the same whole affine cleanup as a direct producer;
        // requiring an operation result here would reject the completed join.
        super::structural_case::source_owner(function, place).is_ok_and(|owner| match owner {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                result, ..
            } => {
                result.multiplicity == StructuralMultiplicity::Affine
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
            }
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                declaration,
                ..
            } => {
                declaration.multiplicity == StructuralMultiplicity::Affine
                    && declaration.access == terminal_psi::StructuralAccess::Owned
                    && declaration.qualifications.is_empty()
                    && declaration.projected_qualifications.is_empty()
            }
            legalized_operations::LegalizedStructuralCaseSource::Parameter { .. } => false,
        })
    };
    actions.iter().all(|action| {
        match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => {
                discarded.insert(*place) && eligible_owner(*place)
            }
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                // A residual is a strictly projected plain subtree of a still
                // eligible root; overlapping boundaries or a discarded root
                // cannot be replayed twice.
                !discard.path.is_empty()
                    && !discarded.contains(&discard.place)
                    && !residuals.iter().any(
                        |(place, path): &(
                            semantic_vocabulary::PlaceId,
                            Vec<terminal_psi::StructuralPathSegment>,
                        )| {
                            *place == discard.place
                                && (path.starts_with(&discard.path)
                                    || discard.path.starts_with(path))
                        },
                    )
                    && eligible_owner(discard.place)
                    && residuals.insert((discard.place, discard.path.clone()))
            }
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_) => false,
        }
    })
}

pub(super) fn header(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let graph = &target.graph;
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || (!matches!(abstracted.result, AbstractFunctionResult::Scalar(_))
            && (target.scalar_abi.is_some() || target.mixed_structural_scalar_abi.is_some()))
        || optimized.result != abstracted.result
        || !roster(optimized)
        || optimized.structural_parameters != abstracted.structural_parameters
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.parameters.len() != optimized.parameters.len()
        || graph.scalar_parameters.len() != abstracted.parameters.len()
        || graph.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(invalid);
    }
    let result = match &abstracted.result {
        AbstractFunctionResult::Unit => None,
        AbstractFunctionResult::Scalar(result)
            if matches!(
                result.scalar_type,
                ScalarType::Boolean | ScalarType::Integer(_)
            ) =>
        {
            Some(scalar_shape(result.scalar_type).ok_or(invalid.clone())?)
        }
        AbstractFunctionResult::Structural(result) => Some(
            if plan.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Sum { .. } | StructuralTypeShape::FixedArray { .. }
                    )
            }) {
                home_layout(
                    &StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: result.place,
                        structural_type: result.structural_type,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        claims: Vec::new(),
                    },
                    plan,
                )?
                .shape()
            } else {
                if result.multiplicity == StructuralMultiplicity::Linear
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(invalid);
                }
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &plan.structural_types,
                )
                .ok_or(invalid.clone())?
            },
        ),
        _ => return Err(invalid),
    };
    let mut shapes = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(invalid.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    for parameter in &abstracted.structural_parameters {
        shapes.push(
            crate::structural_inputs::structural_reference_input::parameter_shape(
                parameter,
                &plan.structural_types,
            )
            .ok_or(invalid.clone())?,
        );
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: shapes,
            result,
        },
    )
    .map_err(|_| invalid.clone())?;
    if graph.call_plan != expected {
        return Err(invalid);
    }
    if let AbstractFunctionResult::Scalar(result) = abstracted.result {
        let retained = match (&target.scalar_abi, &target.mixed_structural_scalar_abi) {
            (Some(abi), None)
                if graph.parameters.is_empty()
                    && abi.call_plan == expected
                    && abi.parameters == graph.scalar_parameters =>
            {
                Some(&abi.result)
            }
            (None, Some(abi))
                if !graph.parameters.is_empty()
                    && abi.call_plan == expected
                    && abi.scalar_parameters == graph.scalar_parameters
                    && abi.structural_parameters == graph.parameters =>
            {
                Some(&abi.result)
            }
            // The graph retains the complete call plan and ordered parameters;
            // target replay independently checks each exact scalar return.
            (None, None) => None,
            _ => return Err(invalid),
        };
        if retained.is_some_and(|retained| {
            retained.value != result.value
                || retained.scalar_type != result.scalar_type
                || Some(&retained.placement) != expected.result.as_ref()
        }) {
            return Err(invalid);
        }
    }
    for (position, ((declared, actual), retained)) in abstracted
        .parameters
        .iter()
        .zip(&optimized.parameters)
        .zip(&graph.scalar_parameters)
        .enumerate()
    {
        if actual.value != declared.value
            || actual.scalar_type != declared.scalar_type
            || actual.site != ValueDefinitionSite::FunctionParameter(position as u32)
            || retained.value != declared.value
            || retained.scalar_type != declared.scalar_type
            || retained.placement != expected.parameters[position]
        {
            return Err(invalid);
        }
    }
    for (position, (declared, retained)) in abstracted
        .structural_parameters
        .iter()
        .zip(&graph.parameters)
        .enumerate()
    {
        if declared.position as usize != position
            || retained.place != declared.place
            || retained.structural_type != declared.structural_type
            || retained.access != declared.access
            || retained.multiplicity != declared.multiplicity
            || !retained.projected_qualifications.is_empty()
            || retained.shape != expected.parameters[abstracted.parameters.len() + position].shape
            || retained.placement != expected.parameters[abstracted.parameters.len() + position]
        {
            return Err(invalid);
        }
    }
    if optimized
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| {
            !byte_parameter(parameter, plan)
                && super::address_joins::parameter(optimized, parameter.place, plan).is_none()
                && block_home_layout(parameter, plan).is_err()
        })
    {
        return Err(invalid);
    }
    Ok(expected)
}

/// Arrival keeps the value's layout but establishes a distinct block-owned home.
/// Input admission, return replay and availability use this same type judgment;
/// exact origin, dominance and ownership transfer are checked by their callers.
pub(in crate::legalization) fn block_home_layout(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
    if !(!parameter.is_self
        && parameter.access == terminal_psi::StructuralAccess::Owned
        && parameter.multiplicity != StructuralMultiplicity::Linear
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::Sum { .. } | StructuralTypeShape::Record { .. }
                )
        }))
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    home_layout(
        &StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
            claims: Vec::new(),
        },
        plan,
    )
}

fn byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> bool {
    !parameter.is_self
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
        })
}

pub(in crate::legalization) fn call_argument(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let destination = callee
        .structural_parameters
        .get(position)
        .ok_or(invalid.clone())?;
    // `.., Referent` spellings resolve through replayed reference custody;
    // the transported value names the referent root, never the carrier.
    if matches!(
        argument.path.last(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        let target_caller = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .ok_or(invalid.clone())?;
        let ordinal = callee
            .parameters
            .len()
            .checked_add(position)
            .ok_or(invalid.clone())?;
        return super::reference_custody::referent_argument(
            argument,
            destination,
            call.parameters.get(ordinal).ok_or(invalid)?,
            caller,
            target_caller,
            custody,
            &plan.structural_types,
        );
    }
    if plan.structural_types.iter().any(|declaration| {
        declaration.id == destination.structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::Record { .. } | StructuralTypeShape::Sum { .. }
            )
    }) && argument.access != terminal_psi::StructuralAccess::Owned
    {
        return borrowed_arguments::reconstruct(
            argument,
            position,
            call_operation,
            caller,
            callee,
            call,
            native,
            plan,
        );
    }
    if argument.access == terminal_psi::StructuralAccess::Owned {
        return owned_arguments::reconstruct(
            argument,
            position,
            call_operation,
            caller,
            callee,
            call,
            native,
            plan,
        );
    }
    if super::primitive_locals::scalar(&plan.structural_types, destination.structural_type)
        .is_some()
    {
        // A projected scalar leaf borrows a subtree of its source's storage;
        // the destination shape stays scalar while the projection lives on the
        // source side, so reconstruct it against the home carrying the root.
        if !argument.path.is_empty() {
            return borrowed_arguments::reconstruct(
                argument,
                position,
                call_operation,
                caller,
                callee,
                call,
                native,
                plan,
            );
        }
        return super::structural_call::primitive_argument(
            argument,
            caller,
            destination,
            call,
            callee
                .parameters
                .len()
                .checked_add(position)
                .ok_or(invalid)?,
            native,
            plan,
        );
    }
    // Non-block byte views use the same exact literal, parameter and projected
    // fixed-array argument reconstruction as scalar- and Unit-result calls.
    if !caller.blocks.iter().any(|block| {
        block
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == argument.place)
    }) {
        return super::structural_call::argument_at(
            argument,
            position,
            call_operation,
            caller,
            callee,
            call,
            native,
            plan,
            custody,
        );
    }
    let caller_target = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(invalid.clone())?;
    let (source, binding) = if let Some(source) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        let retained = super::structural_parameters(caller_target)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
            })
            .ok_or(invalid.clone())?;
        (source, retained.placement.clone().into())
    } else {
        let (block, source) = caller
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .map(|parameter| (block.id, parameter))
            })
            .ok_or(invalid.clone())?;
        (
            source,
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block,
                place: source.place,
            },
        )
    };
    if !argument.path.is_empty()
        || !byte_parameter(source, plan)
        || !byte_parameter(destination, plan)
        || source.structural_type != destination.structural_type
        || source.access != argument.access
        || argument.access != destination.access
    {
        return Err(invalid);
    }
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(invalid.clone())?;
    if placement.shape != ValueShape::borrowed_reference(16, 8) {
        return Err(invalid);
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: source.structural_type,
        structural_type: source.structural_type,
        shape: placement.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: binding,
        destination: placement.clone(),
    })
}

pub(super) fn result_home(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeRequirement, LegalizationError> {
    let (origin, layout) = match super::structural_case::source_owner(function, place)? {
        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation,
            result,
        } => {
            let layout = home_layout(&result, plan)?;
            (
                target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result,
                },
                layout,
            )
        }
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration,
        } => {
            let layout = block_home_layout(&declaration, plan)?;
            (
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block,
                    declaration,
                },
                layout,
            )
        }
        // A function parameter is an arrival, not an activation-local home.
        legalized_operations::LegalizedStructuralCaseSource::Parameter { .. } => {
            return Err(LegalizationError::SourceCustodyMismatch);
        }
    };
    Ok(target_operations::TargetStructuralHomeRequirement { origin, layout })
}

pub(in crate::legalization) fn home_layout(
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
    if plan.structural_types.iter().any(|declaration| {
        declaration.id == result.structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::Reference { .. }
            )
    }) {
        // Qualifications ride the home origin verbatim as custody evidence;
        // the layout is the carrier's physical shape alone.
        if result.multiplicity == StructuralMultiplicity::Linear || !result.claims.is_empty() {
            return Err(LegalizationError::SourceCustodyMismatch);
        }
        return Ok(target_operations::TargetStructuralHomeLayout::Aggregate(
            crate::structural_inputs::structural_reference_input::primitive_array_shape(
                result.structural_type,
                &plan.structural_types,
            )
            .or_else(|| {
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &plan.structural_types,
                )
            })
            .ok_or(LegalizationError::SourceCustodyMismatch)?,
        ));
    }
    Ok(target_operations::TargetStructuralHomeLayout::Sum(
        sum_layout(result, plan)?,
    ))
}
