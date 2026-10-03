//! Direct calls resolve one callee contract, then each operand's actual storage.
//! A sibling operand and the call's result kind never change argument transport.
//! Published scalar calls retain their independently checked standalone ABI;
//! ordinary calls use the declared signature and live reference custody.
//! Structural results establish their own homes and returned reference leaves
//! only after argument preparation succeeds. Descriptor-forwarding calls remain
//! with the dynamic-call owner.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::{PreparedFunctionSignature, prepare_function_signature};
use crate::lowering::scalar::byte_views::is_byte_parameter;
use crate::lowering::scalar::element_views::is_element_view_parameter;
use crate::lowering::scalar_abi::fixed_native_scalar_shape;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use crate::lowering::unit::roster_arguments::lower_structural_argument;
use abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractResult, AbstractStructuralCallResult,
};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    MachineId, ObligationId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    ScalarAbiValue, ScalarFunctionAbi, TargetCallResult, TargetStructuralArgument,
    TargetStructuralHomeRequirement, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use terminal_psi::{
    ClaimTransfer, CrashRouteBucket, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
};

/// Only the published scalar operation requires a separately published ABI.
/// This distinction never selects an operand transport algorithm.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CallContract {
    PublishedScalar,
    Ordinary,
}

/// One direct call's operands, whichever abstract operation spelled it.
struct DirectCall<'a> {
    psi_operation: OperationId,
    callee: MachineId,
    scalar_arguments: &'a [ValueId],
    structural_arguments: &'a [StructuralArgument],
    dynamic_arguments: &'a [AbstractDynamicDescriptorArgument],
    claim_transfers: &'a [ClaimTransfer],
    requirement_obligations: &'a [ObligationId],
    crash_continuations: &'a [CrashRouteBucket],
    result: &'a AbstractStructuralCallResult,
}

/// Lower one direct call to its `TargetUnitOperation::Call` row.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_direct_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    prepared: &PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    // The scalar-only `Call` is a scalar-result call without structural
    // operands; it borrows the same result vocabulary.
    let published_result;
    let (call, contract) = match operation {
        AbstractOperation::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            result,
            requirement_obligations,
            crash_continuations,
        } => {
            let call = DirectCall {
                psi_operation: *psi_operation,
                callee: *callee,
                scalar_arguments: arguments,
                structural_arguments,
                dynamic_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                result,
            };
            (call, CallContract::Ordinary)
        }
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => {
            published_result = AbstractStructuralCallResult::Scalar(AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            });
            let call = DirectCall {
                psi_operation: *psi_operation,
                callee: *callee,
                scalar_arguments: arguments,
                structural_arguments: &[],
                dynamic_arguments: &[],
                claim_transfers: &[],
                requirement_obligations,
                crash_continuations,
                result: &published_result,
            };
            (call, CallContract::PublishedScalar)
        }
        _ => return Err(unsupported()),
    };
    let callee_function = functions
        .get(&call.callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(call.callee))?;
    let (signature, structural_home) = match contract {
        CallContract::PublishedScalar => (
            published_scalar_contract(&call, callee_function, target, scalar_abis)?,
            None,
        ),
        CallContract::Ordinary => {
            ordinary_contract(&call, callee_function, function, target, types)?
        }
    };
    // Check the whole roster before preparing any argument. Carrier moves
    // commit only after all operands have retained their exact storage.
    let moved =
        super::references::call_arguments(function, types, live, call.structural_arguments)?;
    let scalar_arguments = call
        .scalar_arguments
        .iter()
        .zip(&signature.scalar_parameters)
        .enumerate()
        .map(|(position, (value, parameter))| {
            scalar_argument(&call, position, *value, parameter, function, live)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let arguments = call
        .structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(&signature.parameters)
        .map(|((argument, declaration), destination)| {
            structural_argument(
                argument,
                declaration,
                destination,
                call.callee,
                function,
                types,
                prepared,
                parameters_by_place,
                live,
                operations,
                &mut shape_cache,
                &mut active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    super::references::commit_argument_moves(live, &moved);
    let result = land_result(
        contract,
        &call,
        callee_function,
        structural_home,
        &moved,
        function,
        types,
        live,
    )?;
    operations.push(TargetUnitOperation::Call {
        origin: target_operations::NativeCallOrigin::Authored,
        psi_operation: call.psi_operation,
        callee: call.callee,
        call_plan: signature.call_plan,
        result,
        scalar_arguments,
        arguments,
        claim_transfers: call.claim_transfers.to_vec(),
        requirement_obligations: call.requirement_obligations.to_vec(),
        crash_continuations: call.crash_continuations.to_vec(),
    });
    provenance.operations.push(call.psi_operation);
    Ok(())
}

/// Land the result through the tail of its kind. Unit defines nothing. A
/// scalar result retains its home among the dominating scalar sources. A
/// structural result establishes its declared reference leaves against the
/// committed argument moves and its durable value or reference-payload home.
#[allow(clippy::too_many_arguments)]
fn land_result(
    contract: CallContract,
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    structural_home: Option<TargetStructuralHomeRequirement>,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), super::references::ReferenceCustody>,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
) -> Result<TargetCallResult, LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    Ok(match call.result {
        AbstractStructuralCallResult::Unit => TargetCallResult::Unit,
        AbstractStructuralCallResult::Scalar(result) => {
            // The scalar-only call refuses a result value any dominating
            // definition already names, before its home joins the graph.
            if contract == CallContract::PublishedScalar
                && super::scalar_sources::source(result.value, function, live).is_ok()
            {
                return Err(LoweringError::DuplicateValue(result.value));
            }
            TargetCallResult::Scalar(super::primitive_storage::retain_result(
                call.psi_operation,
                *result,
                live,
            )?)
        }
        AbstractStructuralCallResult::Structural {
            result,
            returned_claim_transfers,
            ..
        } => {
            let result_home = structural_home.ok_or_else(unsupported)?;
            let callee_result = callee_function
                .result
                .structural()
                .ok_or_else(unsupported)?;
            // Arguments committed: the declared result roster lands as live
            // custody.
            let reference_results = super::references::call_results(
                function,
                callee_function,
                types,
                result,
                call.psi_operation,
                call.structural_arguments,
                live,
                moved,
            )?;
            // Reference results own an address payload home, not referent
            // storage. The separate roster above retains loan authority;
            // ordinary result transport captures the actually selected address.
            if live
                .structural_homes
                .insert(result.place, result_home.clone())
                .is_some()
            {
                return Err(unsupported());
            }
            if (bounded_owned_byte_result(result.structural_type, types)
                || terminal_semantics::descriptor_results::is_shared_descriptor(
                    types.values().copied(),
                    result.structural_type,
                ))
                && live
                    .views
                    .insert(result.place, (call.psi_operation, result.structural_type))
                    .is_some()
            {
                return Err(unsupported());
            }
            TargetCallResult::Structural {
                result: result.clone(),
                callee_result: callee_result.clone(),
                result_home: Some(result_home),
                reference_results,
                returned_claim_transfers: returned_claim_transfers.clone(),
            }
        }
    })
}

/// The scalar-only call takes a callee without structural parameters or entry
/// claims, whose fixed-native scalar result matches the call's and sits in one
/// register, and whose published standalone scalar ABI equals this plan.
fn published_scalar_contract(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    target: NativeTarget,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
) -> Result<PreparedFunctionSignature, LoweringError> {
    let callee = call.callee;
    let unsupported = || LoweringError::UnitScalarCallTargetShapeUnsupported(callee);
    let AbstractStructuralCallResult::Scalar(result) = call.result else {
        return Err(unsupported());
    };
    if !callee_function.structural_parameters.is_empty() || !callee_function.entry_claims.is_empty()
    {
        return Err(unsupported());
    }
    let callee_result = callee_function.result.scalar().ok_or_else(unsupported)?;
    let result_shape = fixed_native_scalar_shape(result.scalar_type).ok_or(
        LoweringError::UnitScalarCallIntegerTypeUnsupported(result.value),
    )?;
    if result.scalar_type != callee_result.scalar_type {
        return Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee,
            result: result.value,
        });
    }
    let parameter_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| fixed_native_scalar_shape(parameter.scalar_type).ok_or_else(unsupported))
        .collect::<Result<Vec<_>, _>>()?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes,
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call.scalar_arguments.len() != callee_function.parameters.len()
        || call.scalar_arguments.len() != call_plan.parameters.len()
    {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee,
            expected: callee_function.parameters.len(),
            actual: call.scalar_arguments.len(),
        });
    }
    let placement_unsupported = || LoweringError::UnitScalarCallResultPlacementUnsupported {
        callee,
        result: result.value,
    };
    let result_placement = call_plan.result.clone().ok_or_else(placement_unsupported)?;
    if !matches!(
        result_placement.locations.as_slice(),
        [ValueLocation::Register {
            value_byte_offset: 0,
            byte_size,
            ..
        }] if *byte_size == result_shape.byte_size
    ) {
        return Err(placement_unsupported());
    }
    let expected_abi = ScalarFunctionAbi {
        parameters: callee_function
            .parameters
            .iter()
            .zip(&call_plan.parameters)
            .map(|(parameter, placement)| ScalarAbiValue {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                placement: placement.clone(),
            })
            .collect(),
        result: ScalarAbiValue {
            value: callee_result.value,
            scalar_type: callee_result.scalar_type,
            placement: result_placement,
        },
        call_plan,
    };
    require_exact_target_abi(callee, scalar_abis.get(&callee), &expected_abi)?;
    Ok(PreparedFunctionSignature {
        call_plan: expected_abi.call_plan,
        scalar_parameters: expected_abi.parameters,
        parameters: Vec::new(),
        dynamic_parameters: Vec::new(),
    })
}

fn require_exact_target_abi(
    callee: MachineId,
    actual: Option<&ScalarFunctionAbi>,
    expected: &ScalarFunctionAbi,
) -> Result<(), LoweringError> {
    if actual != Some(expected) {
        return Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee));
    }
    Ok(())
}

/// Ordinary calls share the declared signature, independently of result kind
/// and operand storage. Claims and descriptor forwarding retain their separate
/// unsupported transport boundaries; proof obligations and crash routes travel
/// unchanged on the emitted call.
fn ordinary_contract(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    function: &AbstractFunction,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<
    (
        PreparedFunctionSignature,
        Option<TargetStructuralHomeRequirement>,
    ),
    LoweringError,
> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    if !call.dynamic_arguments.is_empty()
        || !call.claim_transfers.is_empty()
        || !callee_function.entry_claims.is_empty()
    {
        return Err(invalid());
    }
    if call.scalar_arguments.len() != callee_function.parameters.len() {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee: call.callee,
            expected: callee_function.parameters.len(),
            actual: call.scalar_arguments.len(),
        });
    }
    if call.structural_arguments.len() != callee_function.structural_parameters.len() {
        return Err(LoweringError::StructuralCallArgumentCountMismatch {
            callee: call.callee,
            expected: callee_function.structural_parameters.len(),
            actual: call.structural_arguments.len(),
        });
    }
    let home = match call.result {
        AbstractStructuralCallResult::Unit => {
            if callee_function.result != AbstractFunctionResult::Unit {
                return Err(LoweringError::UnitCallTargetKindMismatch(call.callee));
            }
            None
        }
        AbstractStructuralCallResult::Scalar(result) => {
            if callee_function
                .result
                .scalar()
                .map(|declared| declared.scalar_type)
                != Some(result.scalar_type)
            {
                return Err(LoweringError::UnitScalarCallResultTypeMismatch {
                    callee: call.callee,
                    result: result.value,
                });
            }
            None
        }
        AbstractStructuralCallResult::Structural { .. } => Some(structural_result_home(
            call,
            callee_function,
            function,
            types,
        )?),
    };
    let signature = prepare_function_signature(callee_function, target, types)?;
    if !signature.dynamic_parameters.is_empty() {
        return Err(invalid());
    }
    Ok((signature, home))
}

/// The structural result retains its exact declared type and home layout.
/// Returned claims and outcome-specific evidence still need separate transport.
/// Byte carriers — bounded owners and shared views alike — may carry
/// qualifications; the caller may add only domains this call's own
/// establishment rows mint.
fn structural_result_home(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractStructuralCallResult::Structural {
        result,
        returned_claim_transfers,
        selected_evidence,
    } = call.result
    else {
        return Err(invalid());
    };
    let callee_result = callee_function.result.structural().ok_or_else(invalid)?;
    let result_home = super::aggregate_results::home(call.psi_operation, result, types)?;
    if callee_result.structural_type != result.structural_type
        || callee_result.multiplicity != result.multiplicity
        || !result_qualifications_match(callee_result, result)
        || callee_result.projected_qualifications != result.projected_qualifications
        || !callee_result.projected_qualifications.is_empty()
        || (!callee_result.qualifications.is_empty()
            && !types
                .get(&result.structural_type)
                .is_some_and(|declaration| declaration.shape.supports_result_qualifications()))
        || !returned_claim_transfers.is_empty()
        || !selected_evidence.is_empty()
    {
        return Err(invalid());
    }
    Ok(result_home)
}

/// The caller-visible roster is the callee's declared roster plus the domains
/// minted by this call's establishment rows (an `established by` route such
/// as a selected provider realizing its requirement). Terminal verification
/// replays each row against its route. A minted domain is discharged proof
/// evidence with no storage, so the result home and ABI stay those of the
/// callee's declared result. Any other caller domain is a qualification the
/// callee never returned and still refuses.
fn result_qualifications_match(
    callee_result: &terminal_psi::StructuralResultDeclaration,
    result: &terminal_psi::StructuralOperationResult,
) -> bool {
    let minted = result
        .qualification_establishments
        .iter()
        .map(|establishment| establishment.domain);
    let expected = callee_result
        .qualifications
        .iter()
        .copied()
        .chain(minted)
        .collect::<BTreeSet<_>>();
    let actual = result
        .qualifications
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    actual.len() == result.qualifications.len() && actual == expected
}

fn bounded_owned_byte_result(
    structural_type: StructuralTypeId,
    types: &StructuralTypeLookup<'_>,
) -> bool {
    types.get(&structural_type).is_some_and(|declaration| {
        matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        )
    })
}

/// One scalar argument resolves through the dominating scalar sources and must
/// carry its callee parameter's declared type.
fn scalar_argument(
    call: &DirectCall<'_>,
    position: usize,
    value: ValueId,
    parameter: &ScalarAbiValue,
    function: &AbstractFunction,
    live: &LiveDefinitions,
) -> Result<TargetUnitScalarCallArgument, LoweringError> {
    let source = super::scalar_sources::source(value, function, live)?;
    if source.scalar_type() != parameter.scalar_type {
        return Err(LoweringError::CallArgumentTypeMismatch {
            callee: call.callee,
            argument: value,
        });
    }
    Ok(TargetUnitScalarCallArgument {
        parameter_index: u32::try_from(position)
            .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
        source,
        placement: parameter.placement.clone(),
    })
}

/// Resolve each operand from its own permission carrier or storage owner.
/// The ordinary parameter resolver also owns inline field presentations;
/// live homes, descriptors and address joins retain their distinct identities.
#[allow(clippy::too_many_arguments)]
fn structural_argument(
    argument: &StructuralArgument,
    declaration: &StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    callee: MachineId,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    live: &LiveDefinitions,
    operations: &[TargetUnitOperation],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<TargetStructuralArgument, LoweringError> {
    if let Some(argument) = super::byte_arguments::shared_owner_view(
        argument,
        declaration,
        destination,
        function,
        prepared,
        live,
        types,
    )? {
        return Ok(argument);
    }
    if argument.path.contains(&StructuralPathSegment::Referent) {
        return super::references::reference_call_argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    if super::aggregate_borrows::is_reference(declaration, types) {
        return super::aggregate_borrows::argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    if super::owned_arguments::is_owned_parameter(declaration, types)
        && (argument.path.is_empty()
            || live.structural_homes.contains_key(&argument.place)
            || live.trivial_affine_locals.contains_key(&argument.place))
    {
        return super::owned_arguments::argument(
            argument,
            declaration,
            destination,
            prepared,
            live,
            types,
        );
    }
    if super::primitive_storage::is_primitive_reference(declaration, types)
        && (argument.path.is_empty() || live.structural_homes.contains_key(&argument.place))
    {
        return super::borrowed_calls::argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    let view =
        is_byte_parameter(declaration, types) || is_element_view_parameter(declaration, types);
    // Whole invocation operands already have their declared placement. Inline
    // byte fields also retain their carrier presentation: they do not acquire
    // a canonical structural leaf identity merely by being borrowed. The
    // roster resolver owns their capacity and descriptor/owner distinction.
    let invocation_projection = !argument.path.is_empty()
        && !types
            .get(&declaration.structural_type)
            .is_some_and(|ty| matches!(ty.shape, StructuralTypeShape::ByteSequence(_)))
        && prepared.parameters.iter().any(|parameter| {
            parameter.place == argument.place && parameter.access != StructuralAccess::Owned
        });
    if !view
        && (live.address_joins.contains(&argument.place) || invocation_projection)
        && argument.access != StructuralAccess::Owned
    {
        return super::borrowed_calls::storage_argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    if live.address_joins.contains(&argument.place) {
        return super::borrowed_calls::address_join_window_argument(
            argument,
            declaration,
            destination,
            callee,
            function,
            live,
            types,
        );
    }
    if live.block_views.contains(&argument.place)
        || (view
            && (live.views.contains_key(&argument.place)
                || live.structural_homes.contains_key(&argument.place)))
    {
        return super::borrowed_calls::byte_argument(
            argument,
            declaration,
            destination,
            callee,
            function,
            prepared,
            live,
            types,
        );
    }
    if live.structural_homes.contains_key(&argument.place) {
        return super::borrowed_calls::argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    lower_structural_argument(
        argument,
        declaration,
        destination.shape,
        &destination.placement,
        callee,
        function,
        types,
        parameters_by_place,
        &live.structural_homes,
        &live.views,
        operations,
        shape_cache,
        active,
    )
}

#[cfg(test)]
mod tests {
    use super::require_exact_target_abi;
    use crate::LoweringError;
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
    use target::NativeTarget;
    use target_operations::{ScalarAbiValue, ScalarFunctionAbi};

    fn abi() -> ScalarFunctionAbi {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let shape = ValueShape::integer(4, 4);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![shape],
                result: Some(shape),
            },
        )
        .unwrap();
        ScalarFunctionAbi {
            parameters: vec![ScalarAbiValue {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
                placement: call_plan.parameters[0].clone(),
            }],
            result: ScalarAbiValue {
                value: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
                placement: call_plan.result.clone().unwrap(),
            },
            call_plan,
        }
    }

    #[test]
    fn exact_target_abi_gate_rejects_absence_and_semantic_or_plan_drift() {
        let callee = MachineId::new(1).unwrap();
        let expected = abi();
        assert_eq!(
            require_exact_target_abi(callee, Some(&expected), &expected),
            Ok(())
        );
        assert_eq!(
            require_exact_target_abi(callee, None, &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );

        let mut changed_value = expected.clone();
        changed_value.parameters[0].value = ValueId::new(3).unwrap();
        assert_eq!(
            require_exact_target_abi(callee, Some(&changed_value), &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );

        let mut changed_plan = expected.clone();
        changed_plan.call_plan.stack_alignment = 32;
        assert_eq!(
            require_exact_target_abi(callee, Some(&changed_plan), &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );
    }
}

/// Cleanup selects a callee through the verified return roster, then uses the
/// same signature and operand transport as an ordinary call. The edge/action
/// identity never substitutes an authored operation identity.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_cleanup_calls(
    terminator: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
) -> Result<(), LoweringError> {
    let (edge, actions) = match terminator {
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        }
        | AbstractOperation::Return {
            psi_edge,
            cleanup_actions,
            ..
        } => (*psi_edge, cleanup_actions),
        _ => return Ok(()),
    };
    for (ordinal, action) in actions.iter().enumerate() {
        let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        let invalid = || LoweringError::unsupported_control_flow(function.machine);
        let callee = functions
            .get(&cleanup.cleanup_machine)
            .copied()
            .ok_or_else(invalid)?;
        if callee.attachment != Some(cleanup.structural_type)
            || callee.result != AbstractFunctionResult::Unit
            || !callee.parameters.is_empty()
            || callee.structural_parameters.len() > 1
            || !cleanup.requirement_obligations.is_empty()
        {
            return Err(invalid());
        }
        let signature = prepare_function_signature(callee, target, types)?;
        let mut arguments = Vec::new();
        for (parameter, destination) in callee
            .structural_parameters
            .iter()
            .zip(&signature.parameters)
        {
            if parameter.position != 0
                || parameter.structural_type != cleanup.structural_type
                || cleanup.cleanup_receiver != Some(parameter.place)
                || !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
            {
                return Err(invalid());
            }
            let argument = StructuralArgument {
                place: cleanup.place,
                path: Vec::new(),
                access: parameter.access,
            };
            // A whole owned record lends its original home; it does not copy
            // that home into the cleanup's borrowed parameter.
            arguments.push(super::borrowed_calls::argument(
                &argument,
                parameter,
                destination,
                function,
                prepared,
                live,
                types,
            )?);
        }
        if !signature.scalar_parameters.is_empty()
            || !signature.dynamic_parameters.is_empty()
            || signature.parameters.len() != callee.structural_parameters.len()
            || (callee.structural_parameters.is_empty() && cleanup.cleanup_receiver.is_some())
        {
            return Err(invalid());
        }
        operations.push(TargetUnitOperation::CleanupCall {
            owner: target_operations::CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: ordinal.try_into().map_err(|_| invalid())?,
            },
            callee: cleanup.cleanup_machine,
            call_plan: signature.call_plan,
            arguments,
        });
    }
    Ok(())
}
