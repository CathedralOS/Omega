//! Independent aggregate layout and ordinary graph signature custody.
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, CallPlan,
    CallSignature, CallingPolicy, PsiOptimizationFunction, TargetFunction, TargetOperationPlan,
    ValueDefinitionSite, ValueShape, evaluate_call_plan, scalar_shape,
};
use crate::LegalizationError;
use abstract_operations::AbstractStructuralCallResult;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape};

pub(super) mod borrowed_arguments;
mod owned_arguments;
pub(super) use owned_arguments::{node_site, precedes};

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
        // Target lowering suppresses owned arrival homes only for bodies the
        // unused-owned-input route replays (`unobserved_owned::body`, the
        // mirror of its `unobserved_owned::accepts`). Any other owned
        // arrival, such as an attached selected-operator realization, was
        // lowered through the ordinary graph with its complete ABI.
        || (function.structural_parameters.iter().any(|parameter| {
            parameter.access == terminal_psi::StructuralAccess::Owned
        }) && !super::unobserved_owned::body(function))
        || function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::StructuralCaseMembership { .. }
                        | AbstractOperation::StructuralByteSequenceFieldLength { .. }
                        | AbstractOperation::StructuralByteSequenceFieldRead { .. }
                        | AbstractOperation::StructuralScalarFieldRead { .. }
                        | AbstractOperation::EstablishStructuralArray { .. }
                        | AbstractOperation::EstablishScalarArray { .. }
                        | AbstractOperation::EstablishByteSequenceConcatenation { .. }
                        | AbstractOperation::EstablishRecord { .. }
                        | AbstractOperation::EstablishReference { .. }
                        | AbstractOperation::ReleaseReference { .. }
                        | AbstractOperation::EstablishCase { .. }
                        | AbstractOperation::StructuralLeafCopy { .. }
                        | AbstractOperation::MoveStructuralField { .. }
                        | AbstractOperation::StoreStructuralField { .. }
                        | AbstractOperation::EstablishTrivialAffineLocal { .. }
                        | AbstractOperation::EstablishZeroedStructural { .. }
                        | AbstractOperation::EstablishStructuralLocal { .. }
                        | AbstractOperation::CallStructural {
                            result: AbstractStructuralCallResult::Structural { .. },
                            ..
                        }
                        | AbstractOperation::BoundaryCall { result: abstract_operations::AbstractBoundaryResult::Structural(_), .. }
                ) || matches!(&node.operation,
                    AbstractOperation::CallStructural {
                        structural_arguments,
                        dynamic_arguments,
                        result:
                            AbstractStructuralCallResult::Unit
                            | AbstractStructuralCallResult::Scalar(_),
                        ..
                    }
                        if dynamic_arguments.is_empty()
                            && structural_arguments.iter().any(|argument|
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
    if result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    sum_type_layout(result.structural_type, plan)
}

pub(in crate::legalization) fn sum_type_layout(
    structural_type: semantic_vocabulary::StructuralTypeId,
    plan: &AbstractOperationPlan,
) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
    let mut declarations = plan
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = declarations.next().ok_or(LegalizationError::custody())?;
    let (common, cases) = match &declaration.shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
        _ => return Err(LegalizationError::custody()),
    };
    if declarations.next().is_some() {
        return Err(LegalizationError::custody());
    }
    let shape_of = |field: &terminal_psi::StructuralFieldDeclaration| {
        crate::structural_inputs::structural_reference_input::field_shape(
            &field.field_type,
            &plan.structural_types,
            &mut Vec::new(),
        )
        .ok_or(LegalizationError::custody())
    };
    let common = common
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(shape_of)
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(shape_of)
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
        .map_err(|_| LegalizationError::custody())
}

pub(super) fn cleanup(
    function: &PsiOptimizationFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let mut disposed: std::collections::BTreeSet<(
        semantic_vocabulary::PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
    )> = std::collections::BTreeSet::new();
    // This reconstructs the retained owner declaration, not permission to
    // dispose it. Unit custody independently checks the action's exact live
    // claim frontier and authorized nominal hook before native lowering.
    let owner = |place: semantic_vocabulary::PlaceId| {
        if let Some((_, structural_type, _)) =
            owned_arguments::trivial_affine_local(function, place)
        {
            return Some((structural_type, StructuralMultiplicity::Affine));
        }
        if function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
        {
            return None;
        }
        if let Some(parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
        {
            return (parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && (parameter.multiplicity != StructuralMultiplicity::Affine
                    || function
                        .entry_claim_declarations
                        .iter()
                        .all(|claim| claim.input != place)))
            .then_some((parameter.structural_type, parameter.multiplicity));
        }
        match super::structural_case::source_owner(function, place).ok()? {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                result, ..
            } => (result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && (result.multiplicity != StructuralMultiplicity::Affine
                    || result.claims.is_empty()))
            .then_some((result.structural_type, result.multiplicity)),
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                declaration,
                ..
            } => (declaration.access == terminal_psi::StructuralAccess::Owned
                && declaration.qualifications.is_empty()
                && declaration.projected_qualifications.is_empty())
            .then_some((declaration.structural_type, declaration.multiplicity)),
            legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
            | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => None,
        }
    };
    let discardable = |place| {
        owner(place).is_some_and(|(_, multiplicity)| multiplicity == StructuralMultiplicity::Affine)
    };
    actions.iter().all(|action| {
        let (place, path) = match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => (*place, &[][..]),
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                (discard.place, discard.path.as_slice())
            }
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                (cleanup.place, cleanup.path.as_slice())
            }
        };
        if disposed.iter().any(|(previous_place, previous_path)| {
            *previous_place == place
                && (previous_path.starts_with(path) || path.starts_with(previous_path))
        }) {
            return false;
        }
        disposed.insert((place, path.to_vec()));
        let selected_type = |root| {
            terminal_semantics::canonical_static_projection(structural_types.iter(), root, path)
                .map(|(_, declaration)| declaration.id)
        };
        match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => discardable(*place),
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                // A residual is a strictly projected plain subtree of a still
                // eligible root; overlapping boundaries or a discarded root
                // cannot be replayed twice.
                !discard.path.is_empty()
                    && discardable(discard.place)
                    && owner(discard.place).and_then(|(root, _)| selected_type(root))
                        == Some(discard.structural_type)
            }
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                owner(cleanup.place).is_some_and(|(structural_type, multiplicity)| {
                    // A projected owner's multiplicity belongs to the selected
                    // obligation, not its containing root. Semantic custody
                    // checks its exact claim roster before target lowering.
                    let whole_claim_contract = match multiplicity {
                        StructuralMultiplicity::Affine => cleanup.consumed_claims.is_empty(),
                        StructuralMultiplicity::Linear => !cleanup.consumed_claims.is_empty(),
                        StructuralMultiplicity::Unrestricted => false,
                    };
                    selected_type(structural_type) == Some(cleanup.structural_type)
                        && (!cleanup.path.is_empty() || whole_claim_contract)
                }) && cleanup
                    .consumed_claims
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                    && cleanup.requirement_obligations.is_empty()
            }
        }
    })
}

/// Content qualifications do not add runtime arguments or alter aggregate ABI
/// layout. Whole-unit ownership reconstruction consumes their entry claims before
/// this signature join; rejecting their presence here would forbid otherwise
/// ordinary owned arguments, including the image/storage roots of an entry.
pub(super) fn header(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<CallPlan, LegalizationError> {
    let graph = &target.graph;
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || (target.attachment != abstracted.attachment
            || target.nominal_cleanup_subject != abstracted.nominal_cleanup_subject)
        || (target.attachment != optimized.attachment
            || target.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
        || (!matches!(abstracted.result, AbstractFunctionResult::Scalar(_))
            && (target.scalar_abi.is_some() || target.mixed_structural_scalar_abi.is_some()))
        || optimized.result != abstracted.result
        || !super::place_declarations::matches_producers(optimized)
        || optimized.structural_parameters != abstracted.structural_parameters
        || abstracted.entry_claims != optimized.entry_claim_declarations
        || abstracted.entry_claims.len() != optimized.entry_claims.len()
        || abstracted
            .entry_claims
            .iter()
            .any(|claim| !optimized.entry_claims.contains(&claim.claim))
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.parameters.len() != optimized.parameters.len()
        || graph.scalar_parameters.len() != abstracted.parameters.len()
        || graph.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(LegalizationError::custody());
    }
    let result = match &abstracted.result {
        AbstractFunctionResult::Unit => None,
        // Aggregate operands do not restrict a scalar result's carrier. Use
        // the same ABI shape judgment as scalar parameters; this preserves
        // floating-register placement instead of an integer-only whitelist.
        AbstractFunctionResult::Scalar(result) => {
            Some(scalar_shape(result.scalar_type).ok_or(LegalizationError::custody())?)
        }
        AbstractFunctionResult::Structural(result) => Some(
            if plan.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::FixedArray { .. }
                            | StructuralTypeShape::Mixed { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
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
                    || (!result.qualifications.is_empty()
                        && !plan.structural_types.iter().any(|declaration| {
                            declaration.id == result.structural_type
                                && declaration.shape.supports_result_qualifications()
                        }))
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(LegalizationError::custody());
                }
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &plan.structural_types,
                )
                .ok_or(LegalizationError::custody())?
            },
        ),
    };
    let mut shapes = abstracted
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.scalar_type).ok_or(LegalizationError::custody()))
        .collect::<Result<Vec<_>, _>>()?;
    for parameter in &abstracted.structural_parameters {
        shapes.push(
            crate::structural_inputs::structural_reference_input::parameter_shape(
                parameter,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?,
        );
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: shapes,
            result,
        },
    )
    .map_err(|_| LegalizationError::custody())?;
    if graph.call_plan != expected {
        return Err(LegalizationError::custody());
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
            _ => return Err(LegalizationError::custody()),
        };
        if retained.is_some_and(|retained| {
            retained.value != result.value
                || retained.scalar_type != result.scalar_type
                || Some(&retained.placement) != expected.result.as_ref()
        }) {
            return Err(LegalizationError::custody());
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
            return Err(LegalizationError::custody());
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
            return Err(LegalizationError::custody());
        }
    }
    if optimized
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| {
            !borrowed_view_parameter(parameter, plan)
                && super::address_joins::parameter(optimized, parameter.place, plan).is_none()
                && block_home_layout(parameter, plan).is_err()
        })
    {
        return Err(LegalizationError::custody());
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
    let owned_bounded_bytes =
        abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
            parameter,
            &plan.structural_types,
        );
    if !(parameter.access == terminal_psi::StructuralAccess::Owned
        && (parameter.qualifications.is_empty() || owned_bounded_bytes)
        && parameter.projected_qualifications.is_empty())
    {
        return Err(LegalizationError::custody());
    }
    // An edge changes the owning declaration, not the carrier's geometry.
    // Share the aggregate storage judgment with other arrivals without
    // imposing call-copy eligibility on already-established nested contents.
    crate::structural_inputs::structural_reference_input::owned_storage_shape(
        parameter.structural_type,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
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

/// A block may receive a borrowed byte or element view descriptor. The
/// extent is either carried dynamically or fixed by its declared type;
/// the view's own reads, lengths and subslices replay against that same
/// parameter place. Ordinary call arguments keep the byte-only rule below.
fn borrowed_view_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> bool {
    parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && (plan.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && (declaration.shape.is_borrowed_byte_carrier()
                    || matches!(declaration.shape, StructuralTypeShape::ElementView { .. }))
        }) || abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan_in(
            parameter,
            &plan.structural_types,
        ))
}

fn byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> bool {
    parameter.multiplicity == StructuralMultiplicity::Unrestricted
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
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
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
    let destination = callee
        .structural_parameters
        .get(position)
        .ok_or(LegalizationError::custody())?;
    // Reference arguments load the exact live descriptor in their carrier.
    if argument
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
    {
        let target_caller = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .ok_or(LegalizationError::custody())?;
        let ordinal = callee
            .parameters
            .len()
            .checked_add(position)
            .ok_or(LegalizationError::custody())?;
        return super::reference_custody::referent_argument(
            argument,
            destination,
            call.parameters
                .get(ordinal)
                .ok_or(LegalizationError::custody())?,
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
                StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. }
                    | StructuralTypeShape::FixedArray { .. }
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
            call_operation,
            caller,
            destination,
            call,
            callee
                .parameters
                .len()
                .checked_add(position)
                .ok_or(LegalizationError::custody())?,
            native,
            plan,
        );
    }
    // An address join lends a projection or fixed-array window of its joined
    // referent through the same replay scalar- and Unit-result calls use.
    if super::address_joins::parameter(caller, argument.place, plan).is_some() {
        return super::address_joins::call_argument(
            argument,
            destination,
            call_operation,
            caller,
            call,
            callee
                .parameters
                .len()
                .checked_add(position)
                .ok_or(LegalizationError::custody())?,
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
        .ok_or(LegalizationError::custody())?;
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
            .ok_or(LegalizationError::custody())?;
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
            .ok_or(LegalizationError::custody())?;
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
        return Err(LegalizationError::custody());
    }
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(LegalizationError::custody())?;
    if placement.shape != ValueShape::borrowed_reference(16, 8) {
        return Err(LegalizationError::custody());
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
        indices: Vec::new(),
        source: binding,
        destination: placement.clone(),
    })
}

pub(super) fn result_home(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeRequirement, LegalizationError> {
    let source = super::structural_case::source_owner(function, place).or_else(|_| {
        let mut matching = function.blocks.iter().flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .filter(move |parameter| parameter.place == place)
                .map(move |parameter| (block.id, parameter))
        });
        let (block, declaration) = matching.next().ok_or(LegalizationError::custody())?;
        if matching.next().is_some()
            || !abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
                declaration,
                &plan.structural_types,
            )
        {
            return Err(LegalizationError::custody());
        }
        Ok(
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration: declaration.clone(),
            },
        )
    })?;
    let (origin, layout) = match source {
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
        legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
        | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => {
            return Err(LegalizationError::custody());
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
                    | StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. }
            )
    }) {
        // Claims and qualifications ride the home origin verbatim as custody
        // evidence; the layout is the carrier's physical shape alone.
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
            .ok_or(LegalizationError::custody())?,
        ));
    }
    Ok(target_operations::TargetStructuralHomeLayout::Sum(
        sum_layout(result, plan)?,
    ))
}
