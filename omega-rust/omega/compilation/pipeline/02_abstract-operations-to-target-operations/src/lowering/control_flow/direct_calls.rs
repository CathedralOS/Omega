//! Direct calls to a known machine lower here to one `TargetUnitOperation::Call`
//! row: every `CallStructural` that forwards no descriptors (Unit, scalar or
//! structural result) and the scalar-only `Call`. Calls that forward
//! descriptors lower through `unit::dynamic::lower_dynamic_call` instead.
//!
//! `lower_direct_call` is the one entry and reads as the call's sequence:
//! select the route, resolve the callee, admit the callee contract (its
//! evaluated plan and placements), stage reference custody, resolve each
//! scalar argument through the dominating `scalar_sources`, dispatch each
//! structural argument by its callee declaration, then land the result through
//! the tail of its kind and emit the row.
//!
//! The route is the admission profile a call lowers under, kept as data rather
//! than as separate entrypoints. The abstract result kind selects it, except
//! that a Unit result splits on its arguments: a Unit call with a graph-local
//! structural argument uses the live-graph route, and one whose structural
//! arguments all resolve against the caller's parameter roster uses the roster
//! route. The routes still admit different callees, scalar sources and argument
//! declarations. Each difference is an explicit check below, never a fallback;
//! widening one changes which programs compile and needs its own evidence.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::{PreparedFunctionSignature, prepare_function_signature};
use crate::lowering::scalar::byte_views::is_byte_parameter;
use crate::lowering::scalar::element_views::is_element_view_parameter;
use crate::lowering::scalar_abi::fixed_native_scalar_shape;
use crate::lowering::structural_signature::StructuralCallSignature;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use crate::lowering::unit::roster_arguments::lower_structural_argument;
use abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractResult, AbstractStructuralCallResult,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    ScalarAbiValue, ScalarFunctionAbi, TargetCallResult, TargetStructuralArgument,
    TargetStructuralHomeRequirement, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use terminal_psi::{
    ClaimTransfer, CrashRouteBucket, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
};

/// The admission profile one direct call lowers under.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectCallRoute {
    /// The scalar-only `Call`: a callee without structural parameters or
    /// entry claims whose published standalone scalar ABI equals this plan.
    PublishedScalar,
    /// A scalar result, or a Unit result with a graph-local structural
    /// argument: operands resolve against the live graph under reference
    /// custody.
    LiveGraph,
    /// A structural result: live-graph operands under reference custody, and
    /// the result lands as a durable home with its reference leaves.
    StructuralResult,
    /// A Unit result whose structural arguments all resolve against the
    /// caller's parameter roster, earlier call results and established views.
    ParameterRoster,
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
    let (call, route) = match operation {
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
            let route = DirectCallRoute::of_structural_call(&call, function, types, live)
                .ok_or_else(unsupported)?;
            (call, route)
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
            (call, DirectCallRoute::PublishedScalar)
        }
        _ => return Err(unsupported()),
    };
    let callee_function = functions
        .get(&call.callee)
        .copied()
        .ok_or_else(|| route.missing_callee(function.machine, call.callee))?;
    let (signature, structural_home) = match route {
        DirectCallRoute::PublishedScalar => (
            published_scalar_contract(&call, callee_function, target, scalar_abis)?,
            None,
        ),
        DirectCallRoute::LiveGraph => (
            live_graph_contract(&call, callee_function, function, target, types)?,
            None,
        ),
        DirectCallRoute::StructuralResult => {
            let (signature, home) =
                structural_result_contract(&call, callee_function, function, target, types)?;
            (signature, Some(home))
        }
        DirectCallRoute::ParameterRoster => (
            parameter_roster_contract(&call, callee_function, target, types)?,
            None,
        ),
    };
    // The live-graph routes check the whole argument roster under reference
    // custody before any argument row is lowered; moved carrier leaves commit
    // only after every row succeeds. A roster-route argument touches no
    // custody (its selection guarantees that), and the published scalar call
    // has no structural arguments.
    let moved = match route {
        DirectCallRoute::LiveGraph | DirectCallRoute::StructuralResult => {
            super::references::call_arguments(function, types, live, call.structural_arguments)?
        }
        DirectCallRoute::PublishedScalar | DirectCallRoute::ParameterRoster => BTreeMap::new(),
    };
    let scalar_arguments = call
        .scalar_arguments
        .iter()
        .zip(&signature.scalar_parameters)
        .enumerate()
        .map(|(position, (value, parameter))| {
            scalar_argument(route, &call, position, *value, parameter, function, live)
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
                route,
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
        route,
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
/// committed argument moves and, unless it is a bare reference, its durable
/// home.
#[allow(clippy::too_many_arguments)]
fn land_result(
    route: DirectCallRoute,
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
            if route == DirectCallRoute::PublishedScalar
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
            if bounded_owned_byte_result(result.structural_type, types)
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

impl DirectCallRoute {
    /// A structural result always takes its own route, which refuses forwarded
    /// descriptors itself. A Unit or scalar call that forwards descriptors
    /// belongs to the dynamic lowering, and a Unit call on the roster route
    /// must transfer no claims; neither has a route here.
    fn of_structural_call(
        call: &DirectCall<'_>,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
    ) -> Option<Self> {
        match call.result {
            AbstractStructuralCallResult::Structural { .. } => Some(Self::StructuralResult),
            _ if !call.dynamic_arguments.is_empty() => None,
            AbstractStructuralCallResult::Scalar(_) => Some(Self::LiveGraph),
            AbstractStructuralCallResult::Unit
                if call
                    .structural_arguments
                    .iter()
                    .any(|argument| is_graph_local(argument, function, types, live)) =>
            {
                Some(Self::LiveGraph)
            }
            AbstractStructuralCallResult::Unit if call.claim_transfers.is_empty() => {
                Some(Self::ParameterRoster)
            }
            AbstractStructuralCallResult::Unit => None,
        }
    }

    fn missing_callee(self, machine: MachineId, callee: MachineId) -> LoweringError {
        match self {
            Self::StructuralResult => LoweringError::unsupported_control_flow(machine),
            Self::PublishedScalar | Self::LiveGraph | Self::ParameterRoster => {
                LoweringError::UnknownCallTarget(callee)
            }
        }
    }

    /// The refusal of a callee parameter this route cannot place.
    fn unsupported_parameter(self, machine: MachineId, callee: MachineId) -> LoweringError {
        match self {
            Self::PublishedScalar => LoweringError::UnitScalarCallTargetShapeUnsupported(callee),
            Self::ParameterRoster => LoweringError::UnitCallTargetKindMismatch(callee),
            Self::LiveGraph | Self::StructuralResult => {
                LoweringError::unsupported_control_flow(machine)
            }
        }
    }

    /// The refusal of a scalar argument whose source is not the declared type.
    fn scalar_argument_mismatch(
        self,
        machine: MachineId,
        callee: MachineId,
        argument: ValueId,
    ) -> LoweringError {
        match self {
            Self::LiveGraph => LoweringError::ValueTypeMismatch(argument),
            Self::StructuralResult => LoweringError::unsupported_control_flow(machine),
            Self::PublishedScalar | Self::ParameterRoster => {
                LoweringError::CallArgumentTypeMismatch { callee, argument }
            }
        }
    }
}

/// Whether an argument names a place only this graph's live state records: a
/// local or result home, an empty-record local, reference custody, a block
/// view, an address join, or an owned caller parameter passed owned. None of
/// these is in the parameter roster the roster route resolves against.
fn is_graph_local(
    argument: &StructuralArgument,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
) -> bool {
    live.structural_homes.contains_key(&argument.place)
        || live.trivial_affine_locals.contains_key(&argument.place)
        // An established element view is a graph-local producer: its
        // argument source replays the descriptor's own establishment, which
        // only the live-graph routes mint. Byte views stay roster-eligible —
        // `lower_structural_argument` carries their own source arm.
        || live
            .views
            .get(&argument.place)
            .is_some_and(|(_, structural_type)| {
                matches!(
                    types
                        .get(structural_type)
                        .map(|declaration| &declaration.shape),
                    Some(terminal_psi::StructuralTypeShape::ElementView { .. })
                )
            })
        || super::references::touches(function, types, live, argument)
        || live.block_views.contains(&argument.place)
        || live.address_joins.contains(&argument.place)
        || (argument.access == StructuralAccess::Owned
            && function.structural_parameters.iter().any(|parameter| {
                parameter.place == argument.place
                    && super::owned_arguments::is_owned_parameter(parameter, types)
            }))
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

/// Requirement obligations are discharged proof metadata and crash
/// continuations the call's verified surviving routes; neither changes custody,
/// so the row carries both unchanged. Ownership transfers still need their own
/// realization, so any claim or callee entry claim refuses. Service reach is
/// already verified against the caller's ceiling. The call keeps the exact
/// callee identity, and a scalar result must be the callee's declared type.
fn live_graph_contract(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    function: &AbstractFunction,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<PreparedFunctionSignature, LoweringError> {
    let result = call.result.scalar();
    if !call.claim_transfers.is_empty()
        || !callee_function.entry_claims.is_empty()
        || call.scalar_arguments.len() != callee_function.parameters.len()
        || call.structural_arguments.len() != callee_function.structural_parameters.len()
        || result.map(|result| result.scalar_type)
            != callee_function
                .result
                .scalar()
                .map(|result| result.scalar_type)
        || (result.is_none() && callee_function.result != AbstractFunctionResult::Unit)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    prepare_function_signature(callee_function, target, types)
}

/// A structural result admits its durable home layout before anything else,
/// then requires the callee's exact declared result, no claims on either
/// side, no outcome-specific evidence and no forwarded descriptors. Only a
/// bounded owned byte result may carry declared qualifications. The caller's
/// roster may add only domains this call's own establishment rows mint.
fn structural_result_contract(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    function: &AbstractFunction,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<(PreparedFunctionSignature, TargetStructuralHomeRequirement), LoweringError> {
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
            && !bounded_owned_byte_result(result.structural_type, types))
        || !callee_function.entry_claims.is_empty()
        || !call.claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !selected_evidence.is_empty()
        || !call.dynamic_arguments.is_empty()
        || call.scalar_arguments.len() != callee_function.parameters.len()
        || call.structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(invalid());
    }
    Ok((
        prepare_function_signature(callee_function, target, types)?,
        result_home,
    ))
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

/// A roster-route Unit call takes a Unit callee with fixed-native scalar
/// parameters. Its plan comes from the callee's declared parameters alone:
/// no result shape and no descriptor words. Callee entry claims are not
/// consulted on this route.
fn parameter_roster_contract(
    call: &DirectCall<'_>,
    callee_function: &AbstractFunction,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<PreparedFunctionSignature, LoweringError> {
    let callee = call.callee;
    if callee_function.result != AbstractFunctionResult::Unit {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    if call.scalar_arguments.len() != callee_function.parameters.len() {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee,
            expected: callee_function.parameters.len(),
            actual: call.scalar_arguments.len(),
        });
    }
    if call.structural_arguments.len() != callee_function.structural_parameters.len() {
        return Err(LoweringError::StructuralCallArgumentCountMismatch {
            callee,
            expected: callee_function.structural_parameters.len(),
            actual: call.structural_arguments.len(),
        });
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| {
            fixed_native_scalar_shape(parameter.scalar_type)
                .ok_or(LoweringError::UnitCallTargetKindMismatch(callee))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = StructuralCallSignature::derive(
        &scalar_shapes,
        &callee_function.structural_parameters,
        None,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let call_plan = signature.plan(target)?;
    Ok(callee_placements(
        callee_function,
        call_plan,
        signature.structural_shapes(),
    ))
}

/// Bind an evaluated plan's placements back to the callee's declared scalar
/// and structural parameters.
fn callee_placements(
    callee_function: &AbstractFunction,
    call_plan: CallPlan,
    structural_shapes: &[ValueShape],
) -> PreparedFunctionSignature {
    let scalar_parameters = callee_function
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .map(|(parameter, placement)| ScalarAbiValue {
            value: parameter.value,
            scalar_type: parameter.scalar_type,
            placement: placement.clone(),
        })
        .collect();
    let parameters = callee_function
        .structural_parameters
        .iter()
        .zip(structural_shapes)
        .zip(
            call_plan
                .parameters
                .iter()
                .skip(callee_function.parameters.len()),
        )
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: parameter.projected_qualifications.clone(),
                shape: *shape,
                placement: placement.clone(),
            },
        )
        .collect();
    PreparedFunctionSignature {
        call_plan,
        scalar_parameters,
        parameters,
        dynamic_parameters: Vec::new(),
    }
}

/// One scalar argument resolves through the dominating scalar sources and must
/// carry its callee parameter's declared type.
fn scalar_argument(
    route: DirectCallRoute,
    call: &DirectCall<'_>,
    position: usize,
    value: ValueId,
    parameter: &ScalarAbiValue,
    function: &AbstractFunction,
    live: &LiveDefinitions,
) -> Result<TargetUnitScalarCallArgument, LoweringError> {
    let resolved = super::scalar_sources::source(value, function, live);
    let source = if route == DirectCallRoute::ParameterRoster {
        roster_scalar_source(value, parameter.scalar_type, resolved, call.callee)?
    } else {
        resolved?
    };
    if source.scalar_type() != parameter.scalar_type {
        return Err(route.scalar_argument_mismatch(function.machine, call.callee, value));
    }
    Ok(TargetUnitScalarCallArgument {
        parameter_index: u32::try_from(position)
            .map_err(|_| route.unsupported_parameter(function.machine, call.callee))?,
        source,
        placement: parameter.placement.clone(),
    })
}

/// The roster route passes a scalar only from a definition its callee reads
/// without a computed Boolean or IEEE-float result home: a caller parameter,
/// an immediate, a Boolean block parameter, or any known integer. Boolean and
/// IEEE-float homes and IEEE-float block parameters are refused with the
/// errors this route has always reported: an unknown value where a Boolean or
/// integer is declared, an argument type mismatch where a float is declared.
fn roster_scalar_source(
    value: ValueId,
    declared: ScalarType,
    resolved: Result<TargetUnitScalarArgumentSource, LoweringError>,
    callee: MachineId,
) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
    match declared {
        ScalarType::Boolean => match resolved? {
            source @ (TargetUnitScalarArgumentSource::Parameter {
                scalar_type: ScalarType::Boolean,
                ..
            }
            | TargetUnitScalarArgumentSource::BooleanImmediate { .. }) => Ok(source),
            TargetUnitScalarArgumentSource::BlockParameter(parameter)
                if !matches!(parameter.scalar_type, ScalarType::Integer(_)) =>
            {
                Ok(TargetUnitScalarArgumentSource::BlockParameter(parameter))
            }
            _ => Err(LoweringError::UnknownValue(value)),
        },
        ScalarType::Integer(_) => match resolved? {
            source if matches!(source.scalar_type(), ScalarType::Integer(_)) => Ok(source),
            _ => Err(LoweringError::UnknownValue(value)),
        },
        ScalarType::IeeeFloat(_) => match resolved {
            Ok(
                source @ (TargetUnitScalarArgumentSource::Parameter { .. }
                | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. }),
            ) => Ok(source),
            _ => Err(LoweringError::CallArgumentTypeMismatch {
                callee,
                argument: value,
            }),
        },
    }
}

/// One structural argument lowers by its callee declaration. Reference,
/// owned and primitive-reference declarations share one source rule on both
/// live-graph routes. Byte views, element views and every other declaration
/// differ by route: a scalar- or Unit-result graph call lends views through
/// the live view sources and anything else through the borrowed-argument
/// sources, while a structural-result call lends a control-flow block view
/// whole and anything else through the parameter-roster sources. The roster
/// route resolves every declaration through the parameter-roster sources.
#[allow(clippy::too_many_arguments)]
fn structural_argument(
    route: DirectCallRoute,
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
    let mut from_parameter_roster = || {
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
    };
    if route == DirectCallRoute::ParameterRoster {
        return from_parameter_roster();
    }
    if matches!(
        argument.path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        let returned_record_argument = super::references::returned_record_call_argument(
            argument,
            declaration,
            destination,
            function,
            live,
            types,
        )?;
        if let Some(returned_record_argument) = returned_record_argument {
            return Ok(returned_record_argument);
        }
        if argument.path.len() > 1 {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
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
    if super::owned_arguments::is_owned_parameter(declaration, types) {
        return super::owned_arguments::argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        );
    }
    if super::primitive_storage::is_primitive_reference(declaration, types) {
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
    match route {
        // An address join lends a fixed-array window of its referent whatever
        // the result kind, exactly as an incoming parameter does.
        DirectCallRoute::LiveGraph | DirectCallRoute::StructuralResult
            if view && live.address_joins.contains(&argument.place) =>
        {
            super::borrowed_calls::address_join_window_argument(
                argument,
                declaration,
                destination,
                callee,
                function,
                live,
                types,
            )
        }
        DirectCallRoute::LiveGraph if view => super::borrowed_calls::byte_argument(
            argument,
            declaration,
            destination,
            callee,
            function,
            prepared,
            live,
            types,
        ),
        DirectCallRoute::LiveGraph => super::borrowed_calls::argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        ),
        DirectCallRoute::StructuralResult if live.block_views.contains(&argument.place) => {
            super::borrowed_calls::block_view_argument(
                argument,
                declaration,
                destination,
                function,
                types,
            )
        }
        DirectCallRoute::StructuralResult => from_parameter_roster(),
        // The published scalar callee declares no structural parameters.
        DirectCallRoute::PublishedScalar | DirectCallRoute::ParameterRoster => {
            Err(route.unsupported_parameter(function.machine, callee))
        }
    }
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
