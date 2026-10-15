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

pub(super) fn uses(function: &PsiOptimizationFunction) -> bool {
    // Owned arrivals use ordinary layout and ABI custody even when their
    // payload is unread. Storage follows each declaration, not an operation
    // whitelist for the enclosing function. Zero-byte values remain a genuine
    // per-place layout distinction in selection.
    function.result.structural().is_some()
        || function.structural_parameters.iter()
            .chain(function.block_structural_parameters())
            .any(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
        || function.block_nodes()
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::StructuralCaseMembership { .. }
                        | abstract_operations::byte_len_or_field_read_pattern!(..)
                        | AbstractOperation::StructuralScalarFieldRead { .. }
                        | AbstractOperation::EstablishStructuralArray { .. }
                        | AbstractOperation::EstablishScalarArray { .. }
                        | AbstractOperation::EstablishByteSequenceConcatenation { .. }
                        | AbstractOperation::EstablishRecord { .. }
                        | AbstractOperation::EstablishReference { .. }
                        | AbstractOperation::EstablishPlacedReference { .. }
                        | AbstractOperation::ReleaseReference { .. }
                        | AbstractOperation::EstablishCase { .. }
                        | AbstractOperation::StructuralLeafCopy { .. }
                        | abstract_operations::move_or_store_structural_field_pattern!()
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
                        result:
                            AbstractStructuralCallResult::Unit
                            | AbstractStructuralCallResult::Scalar(_),
                        ..
                    }
                        if structural_arguments.iter().any(|argument|
                                argument.access == terminal_psi::StructuralAccess::Owned))
                    // Dispatching on an incoming parameter reads its tag and
                    // payloads, as a membership observation reads its tag.
                    || matches!(&node.operation,
                    AbstractOperation::StructuralCase { source, .. }
                        if function.structural_parameter(*source).is_some())
            })
}

pub(in crate::legalization) fn sum_layout(
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<calling_conventions::ConventionalSumLayout, LegalizationError> {
    // Claims, qualifications and multiplicity remain in the retained home
    // origin; they do not change tag or payload bytes. Layout reconstructs
    // geometry, never another permission to copy or dispose the sum.
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
    // This reconstructs the retained owner declaration, not permission to
    // dispose it. Source checking selects the exact claim dispositions and
    // nominal hook; claims and qualifications do not change the retained
    // storage declaration or grant disposal authority.
    let owner = |place: semantic_vocabulary::PlaceId| {
        if let Some((_, structural_type, _)) =
            owned_arguments::trivial_affine_local(function, place)
        {
            return Some((structural_type, StructuralMultiplicity::Affine));
        }
        if let Some(parameter) = function.arrival_parameter(place) {
            // A descriptor arrival owns its carrier lifetime even when its
            // payload is a loan. Reconstruct that declaration as for a produced
            // descriptor; this grants no ownership of its borrowed backing.
            return (parameter.access == terminal_psi::StructuralAccess::Owned
                || (parameter.is_unqualified_root()
                    && terminal_semantics::view_descriptor_parameter(
                        parameter,
                        structural_types.iter(),
                    )))
            .then_some((parameter.structural_type, parameter.multiplicity));
        }
        let (_, result) = super::structural_case::source_result(function, place).ok()?;
        Some((result.structural_type, result.multiplicity))
    };
    let discardable = |place| {
        owner(place).is_some_and(|(_, multiplicity)| multiplicity == StructuralMultiplicity::Affine)
    };
    actions.iter().all(|action| {
        let path = action.path();
        // Parent hooks precede descendant hooks in the checked action stream.
        // Their receiver regions overlap by design; storage geometry cannot
        // replace the source checker's obligation and ordering decisions.
        let selected_type = |root| {
            terminal_semantics::runtime_structural_path_tip(structural_types.iter(), root, path)
                .map(|declaration| declaration.id)
        };
        match action {
            terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => owner(*place).is_some(),
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => discardable(*place),
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                // Disposal selects an exact region; home retirement is a
                // separate action even when that region covers the root.
                // A linear parent can contain this residual after its hook
                // settles its claim. Rejoin the selected region's geometry,
                // not the containing owner's original disposal category.
                owner(discard.place).and_then(|(root, _)| selected_type(root))
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
        || (match abstracted.result {
            AbstractFunctionResult::Unit => target
                .scalar_abi
                .as_ref()
                .is_some_and(|abi| abi.result.is_some()),
            AbstractFunctionResult::Structural(_) => target.scalar_abi.is_some(),
            AbstractFunctionResult::Scalar(_) => false,
        })
        || (!matches!(abstracted.result, AbstractFunctionResult::Scalar(_))
            && target.mixed_structural_scalar_abi.is_some())
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
        AbstractFunctionResult::Structural(result) => {
            Some(type_home_layout(result.structural_type, plan)?.shape())
        }
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
    if graph.call_plan != expected
        || target.scalar_abi.as_ref().is_some_and(|abi| {
            !graph.parameters.is_empty()
                || abi.call_plan != expected
                || abi.parameters != graph.scalar_parameters
        })
    {
        return Err(LegalizationError::custody());
    }
    if let AbstractFunctionResult::Scalar(result) = abstracted.result {
        let retained = match (&target.scalar_abi, &target.mixed_structural_scalar_abi) {
            (Some(abi), None) => Some(abi.result.as_ref().ok_or(LegalizationError::custody())?),
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
            || retained.projected_qualifications != declared.projected_qualifications
            || retained.shape != expected.parameters[abstracted.parameters.len() + position].shape
            || retained.placement != expected.parameters[abstracted.parameters.len() + position]
        {
            return Err(LegalizationError::custody());
        }
    }
    if optimized.block_structural_parameters().any(|parameter| {
        !borrowed_view_parameter(parameter, plan)
            && super::address_joins::parameter(optimized, parameter.place, plan).is_none()
            && block_home_layout(parameter, plan).is_err()
    }) {
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
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(LegalizationError::custody());
    }
    // Arrival changes the owning declaration, not the stored value's geometry.
    // In particular a transferred reference owns its descriptor, not the
    // referent aggregate. Use the same layout judgment as operation results;
    // aggregate call-copy eligibility cannot decide an edge's storage. The
    // callers independently replay exact transfer and origin availability.
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
    terminal_semantics::view_descriptor_parameter(parameter, plan.structural_types.iter())
        || abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan_in(
            parameter,
            &plan.structural_types,
        )
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
    callee: abstract_operations::AbstractCallSignature<'_>,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let declaration = super::call_signature::parameter(callee, position, argument.place)?;
    let destination = &declaration;
    // Reference arguments load the exact live descriptor in their carrier.
    if argument
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
    {
        let ordinal = callee
            .scalar_parameter_count()
            .checked_add(position)
            .ok_or(LegalizationError::custody())?;
        return super::reference_custody::referent_argument(
            argument,
            destination,
            call.parameters
                .get(ordinal)
                .ok_or(LegalizationError::custody())?,
            caller,
            native,
            call_operation,
            custody,
            &plan.structural_types,
        );
    }
    if argument.access != terminal_psi::StructuralAccess::Owned
        && crate::structural_inputs::structural_reference_input::owned_storage_shape(
            destination.structural_type,
            &plan.structural_types,
        )
        .is_some()
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
                .scalar_parameter_count()
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
                .scalar_parameter_count()
                .checked_add(position)
                .ok_or(LegalizationError::custody())?,
            native,
            plan,
        );
    }
    // Non-block byte views use the same exact literal, parameter and projected
    // fixed-array argument reconstruction as scalar- and Unit-result calls.
    if !caller
        .blocks
        .iter()
        .any(|block| block.structural_parameter(argument.place).is_some())
    {
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
        .function(caller.machine)
        .ok_or(LegalizationError::custody())?;
    let (source, binding) = if let Some(source) = caller.structural_parameter(argument.place) {
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
                    .structural_parameter(argument.place)
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
        .get(callee.scalar_parameter_count() + position)
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

/// A borrowed block arrival keeps its own address or descriptor, independently
/// of the call's ABI. Consumers verify availability and projection at their
/// call site; an arrival declaration does not manufacture owned backing.
pub(in crate::legalization) fn borrowed_block_arrival<'function>(
    function: &'function PsiOptimizationFunction,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Option<(
    semantic_vocabulary::BlockId,
    &'function terminal_psi::StructuralParameterDeclaration,
)> {
    super::address_joins::parameter(function, place, plan)
        .or_else(|| borrowed_arguments::view_arrival(function, place, &plan.structural_types))
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
    let layout = type_home_layout(result.structural_type, plan)?;
    // Geometry extraction is also used for borrowed roots. A result home
    // retains the existing sum-custody admission, independently of geometry.
    if matches!(
        layout,
        target_operations::TargetStructuralHomeLayout::Sum(_)
    ) {
        sum_layout(result, plan)?;
    }
    Ok(layout)
}

pub(in crate::legalization) fn type_home_layout(
    structural_type: semantic_vocabulary::StructuralTypeId,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralHomeLayout, LegalizationError> {
    if plan.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(
                declaration.shape,
                terminal_psi::record_or_fixed_array_shape_pattern!()
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
                structural_type,
                &plan.structural_types,
            )
            .or_else(|| {
                crate::structural_inputs::structural_reference_input::shape(
                    structural_type,
                    &plan.structural_types,
                )
            })
            .ok_or(LegalizationError::custody())?,
        ));
    }
    Ok(target_operations::TargetStructuralHomeLayout::Sum(
        sum_type_layout(structural_type, plan)?,
    ))
}
