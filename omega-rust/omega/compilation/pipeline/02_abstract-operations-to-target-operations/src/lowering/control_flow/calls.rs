//! Calls resolve their declared contract, then each operand's actual storage.
//! A sibling operand and the call's result kind never change argument transport.
//! Published scalar calls retain their independently checked standalone ABI;
//! ordinary calls use the declared signature and live reference custody.
//! Structural results establish their own homes and returned reference leaves
//! only after argument preparation succeeds.
//! Dynamic targets borrow the public requirement slot, never one table row.
//! Their opaque selection operand stays separate from the ordinary argument ABI.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::{PreparedFunctionSignature, prepare_function_signature};
use crate::lowering::scalar::byte_views::is_byte_parameter;
use crate::lowering::scalar::element_views::is_element_view_parameter;
use crate::lowering::scalar_abi::fixed_native_scalar_shape;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use crate::lowering::unit::roster_arguments::lower_structural_argument;
use abstract_operations::{
    AbstractCallSignature, AbstractCallTarget, AbstractFunction, AbstractFunctionResult,
    AbstractOperation, AbstractResult, AbstractStructuralCallResult, StructuralCallFormal,
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

/// One invocation's operands, whichever abstract operation spelled it.
struct Invocation<'a> {
    psi_operation: OperationId,
    target: AbstractCallTarget,
    // Diagnostics name the direct callee or the dynamic occurrence's caller;
    // this coordinate never supplies a requirement signature or implementation.
    callee: MachineId,
    scalar_arguments: &'a [ValueId],
    structural_arguments: &'a [StructuralArgument],
    claim_transfers: &'a [ClaimTransfer],
    requirement_obligations: &'a [ObligationId],
    crash_continuations: &'a [CrashRouteBucket],
    result: &'a AbstractStructuralCallResult,
}

/// Lower one invocation to its `TargetUnitOperation::Call` row.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    types: &StructuralTypeLookup<'_>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    prepared: &PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    // The scalar-only `Call` is a scalar-result call without structural
    // operands; it borrows the same result vocabulary.
    let published_result;
    let (call, contract) = match operation {
        AbstractOperation::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            result,
            requirement_obligations,
            crash_continuations,
        } => {
            let call = Invocation {
                psi_operation: *psi_operation,
                target: *callee,
                callee: callee.machine().unwrap_or(function.machine),
                scalar_arguments: arguments,
                structural_arguments,
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
            let call = Invocation {
                psi_operation: *psi_operation,
                target: AbstractCallTarget::Machine(*callee),
                callee: *callee,
                scalar_arguments: arguments,
                structural_arguments: &[],
                claim_transfers: &[],
                requirement_obligations,
                crash_continuations,
                result: &published_result,
            };
            (call, CallContract::PublishedScalar)
        }
        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
    };
    let (declaration, callee_function) = match call.target {
        AbstractCallTarget::Machine(machine) => {
            let callee = functions
                .get(&machine)
                .copied()
                .ok_or(LoweringError::UnknownCallTarget(machine))?;
            (AbstractCallSignature::Machine(callee), Some(callee))
        }
        AbstractCallTarget::Dynamic {
            interface,
            slot,
            selector,
        } => {
            let requirement = dynamic_dispatch
                .interface(interface)
                .and_then(|interface| interface.slots.get(slot as usize))
                .filter(|requirement| requirement.slot == slot)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let operand = super::scalar_sources::source(selector, function, live)?;
            if !matches!(operand.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)
            {
                return Err(LoweringError::ValueTypeMismatch(selector));
            }
            (AbstractCallSignature::Requirement(requirement), None)
        }
    };
    if callee_function.is_none()
        && (0..declaration.structural_parameter_count()).any(|position| {
            declaration
                .structural_parameter(position)
                .is_some_and(|formal| {
                    types
                        .catalog()
                        .placed_views()
                        .iter()
                        .any(|plan| plan.carrier == formal.structural_type)
                })
        })
    {
        return Err(LoweringError::PlacedDynamicCallRequiresBinding {
            caller: function.machine,
            operation: call.psi_operation,
        });
    }
    let placed_bindings = if let Some(callee) = callee_function {
        super::references::placed_call_custody(
            function,
            callee,
            call.psi_operation,
            call.structural_arguments,
            types,
            live,
        )?
    } else {
        Vec::new()
    };
    let (signature, structural_home) = match contract {
        CallContract::PublishedScalar => (
            published_scalar_contract(
                &call,
                callee_function.ok_or(LoweringError::unsupported_control_flow(function.machine))?,
                target,
                scalar_abis,
            )?
            .into(),
            None,
        ),
        CallContract::Ordinary => ordinary_contract(&call, declaration, function, target, types)?,
    };
    // Retain descriptor origins before preparing operands. Psi has checked
    // their ownership; commit the native metadata move only after every
    // operand has retained its exact storage and ABI placement.
    let moved = super::references::moved_argument_leaves(live, call.structural_arguments);
    let stored = super::references::stage_stored_effects(
        function,
        callee_function,
        types,
        live,
        call.structural_arguments,
    )?;
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
        .zip(
            (0..declaration.structural_parameter_count())
                .map(|position| declaration.structural_parameter(position)),
        )
        .zip(&signature.parameters)
        .map(|((argument, declaration), destination)| {
            let declaration =
                declaration.ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            structural_argument(
                argument,
                &declaration,
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
        declaration,
        structural_home,
        &moved,
        function,
        types,
        live,
    )?;
    super::references::commit_stored_effects(live, stored);
    operations.push(TargetUnitOperation::Call {
        origin: target_operations::NativeCallOrigin::Authored,
        psi_operation: call.psi_operation,
        target: match call.target {
            AbstractCallTarget::Machine(machine) => {
                target_operations::NativeCallTarget::Machine(machine)
            }
            AbstractCallTarget::Dynamic {
                selector,
                interface,
                slot,
            } => target_operations::NativeCallTarget::Dynamic {
                selector,
                interface,
                slot,
            },
        },
        call_plan: signature.call_plan,
        result,
        scalar_arguments,
        arguments,
        placed_bindings,
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
    call: &Invocation<'_>,
    declaration: AbstractCallSignature<'_>,
    structural_home: Option<TargetStructuralHomeRequirement>,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), super::references::ReferenceStorage>,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
) -> Result<TargetCallResult, LoweringError> {
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
        AbstractStructuralCallResult::Structural { result, .. } => {
            let result_home =
                structural_home.ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let reference_results = super::references::call_results(
                function,
                declaration,
                types,
                result,
                call.structural_arguments,
                live,
                moved,
            )?;
            let callee_result = match declaration {
                AbstractCallSignature::Machine(callee) => {
                    let result_declaration = callee
                        .result
                        .structural()
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                    target_operations::TargetStructuralResultContract::Machine(
                        result_declaration.clone(),
                    )
                }
                AbstractCallSignature::Requirement(requirement) => {
                    let terminal_psi::BoundaryMachineResult::Structural(result_declaration) =
                        &requirement.result
                    else {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    };
                    target_operations::TargetStructuralResultContract::Requirement(
                        result_declaration.clone(),
                    )
                }
            };
            // Reference results own an address payload home, not referent
            // storage. The separate roster above retains possible origins;
            // ordinary result transport captures the actually selected address.
            if live
                .structural_homes
                .insert(result.place, result_home.clone())
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
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
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            TargetCallResult::Structural {
                result: result.clone(),
                callee_result,
                result_home: Some(result_home),
                reference_results,
            }
        }
    })
}

/// The scalar-only call takes a callee without structural parameters or entry
/// claims, whose fixed-native scalar result matches the call's and sits in one
/// register, and whose published standalone scalar ABI equals this plan.
fn published_scalar_contract(
    call: &Invocation<'_>,
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
        result: Some(ScalarAbiValue {
            value: callee_result.value,
            scalar_type: callee_result.scalar_type,
            placement: result_placement,
        }),
        call_plan,
    };
    require_exact_target_abi(callee, scalar_abis.get(&callee), &expected_abi)?;
    Ok(PreparedFunctionSignature {
        call_plan: expected_abi.call_plan,
        scalar_parameters: expected_abi.parameters,
        parameters: Vec::new(),
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
/// and operand storage. Exact claim transfers, proof obligations and crash
/// routes travel unchanged on the emitted call. Selector forwarding uses the
/// ordinary scalar carrier without lending any data authority.
fn ordinary_contract(
    call: &Invocation<'_>,
    declaration: AbstractCallSignature<'_>,
    function: &AbstractFunction,
    target: NativeTarget,
    types: &StructuralTypeLookup<'_>,
) -> Result<
    (
        super::call_signature::PreparedCallSignature,
        Option<TargetStructuralHomeRequirement>,
    ),
    LoweringError,
> {
    if call.scalar_arguments.len() != declaration.scalar_parameter_count() {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee: call.callee,
            expected: declaration.scalar_parameter_count(),
            actual: call.scalar_arguments.len(),
        });
    }
    if call.structural_arguments.len() != declaration.structural_parameter_count() {
        return Err(LoweringError::StructuralCallArgumentCountMismatch {
            callee: call.callee,
            expected: declaration.structural_parameter_count(),
            actual: call.structural_arguments.len(),
        });
    }
    let home = match call.result {
        AbstractStructuralCallResult::Unit => {
            if !declaration.is_unit_result() {
                return Err(LoweringError::UnitCallTargetKindMismatch(call.callee));
            }
            None
        }
        AbstractStructuralCallResult::Scalar(result) => {
            if declaration.scalar_result() != Some(result.scalar_type) {
                return Err(LoweringError::UnitScalarCallResultTypeMismatch {
                    callee: call.callee,
                    result: result.value,
                });
            }
            None
        }
        AbstractStructuralCallResult::Structural { .. } => {
            Some(structural_result_home(call, declaration, function, types)?)
        }
    };
    let signature = super::call_signature::prepare(function.machine, declaration, target, types)?;
    Ok((signature, home))
}

/// The structural result retains its exact declared type and home layout.
/// Its path-bound claims are custody, not storage; outcome-specific evidence
/// still needs separate transport.
/// Qualifications do not restrict physical carrier families; the caller may
/// add only domains this call's own establishment rows mint.
fn structural_result_home(
    call: &Invocation<'_>,
    declaration: AbstractCallSignature<'_>,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    let AbstractStructuralCallResult::Structural {
        result,
        selected_evidence,
    } = call.result
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let callee_result = declaration
        .structural_result()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let result_home = super::aggregate_results::home(call.psi_operation, result, types)?;
    if callee_result.structural_type != result.structural_type
        || callee_result.multiplicity != result.multiplicity
        || !result_qualifications_match(callee_result.qualifications, result)
        || callee_result.projected_qualifications != result.projected_qualifications
        || !selected_evidence.is_empty()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    result: &terminal_psi::StructuralOperationResult,
) -> bool {
    let minted = result
        .qualification_establishments
        .iter()
        .map(|establishment| establishment.domain);
    let expected = qualifications
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
    types
        .get(&structural_type)
        .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
}

/// One scalar argument resolves through the dominating scalar sources and must
/// carry its callee parameter's declared type.
fn scalar_argument(
    call: &Invocation<'_>,
    position: usize,
    value: ValueId,
    parameter: &super::call_signature::PreparedScalarCallFormal,
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
    formal: &StructuralCallFormal<'_>,
    placement: &super::call_signature::PreparedStructuralCallFormal,
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
    // Existing geometry classifiers consume declaration-shaped inputs but read
    // only formal type/access and placement. Bind that scratch view to the
    // actual caller operand, not an invented callee place. Its identity grants
    // no storage or permission: every transport below reconstructs source
    // custody from `function`, `prepared` and `live` independently.
    let declaration = &StructuralParameterDeclaration {
        place: argument.place,
        position: formal.position,
        structural_type: formal.structural_type,
        multiplicity: formal.multiplicity,
        access: formal.access,
        qualifications: formal.qualifications.to_vec(),
        projected_qualifications: formal.projected_qualifications.to_vec(),
    };
    let destination = &TargetStructuralParameter {
        place: argument.place,
        structural_type: formal.structural_type,
        multiplicity: formal.multiplicity,
        access: formal.access,
        projected_qualifications: formal.projected_qualifications.to_vec(),
        shape: placement.shape,
        placement: placement.placement.clone(),
    };
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
    // Owned aggregate transport follows the selected type and exact root
    // storage, whether that root arrived as a parameter or was produced here.
    // Routing incoming projections through view presentation instead would
    // attach array descriptor metadata to an owned child ABI.
    if super::owned_arguments::is_owned_parameter(declaration, types) {
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
    // A view arriving at a state block lends its elements the same way.
    let invocation_projection = !argument.path.is_empty()
        && !types
            .get(&declaration.structural_type)
            .is_some_and(|ty| matches!(ty.shape, StructuralTypeShape::ByteSequence(_)))
        && (prepared.parameters.iter().any(|parameter| {
            parameter.place == argument.place && parameter.access != StructuralAccess::Owned
        }) || live.block_views.contains(&argument.place));
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

/// Cleanup selects a callee through the verified edge roster, then uses the
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
    for (edge, actions) in terminator.cleanup_rosters() {
        for (ordinal, action) in actions.iter().enumerate() {
            let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                continue;
            };
            let callee = functions
                .get(&cleanup.cleanup_machine)
                .copied()
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            if callee.attachment != Some(cleanup.structural_type)
                || callee.nominal_cleanup_subject != Some(cleanup.structural_type)
                || callee.result != AbstractFunctionResult::Unit
                || !callee.parameters.is_empty()
                || callee.structural_parameters.len() > 1
                || !cleanup.requirement_obligations.is_empty()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let signature = prepare_function_signature(callee, target, types)?;
            let mut arguments = Vec::new();
            for (parameter, destination) in callee
                .structural_parameters
                .iter()
                .zip(&signature.parameters)
            {
                if !parameter.is_cleanup_receiver(cleanup) {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let argument = StructuralArgument {
                    place: cleanup.place,
                    path: cleanup.path.clone(),
                    access: parameter.access,
                };
                // Cleanup lends the selected owner's original storage; it does
                // not copy that storage into the hook's borrowed parameter.
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
                || signature.parameters.len() != callee.structural_parameters.len()
                || (callee.structural_parameters.is_empty() && cleanup.cleanup_receiver.is_some())
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            operations.push(TargetUnitOperation::CleanupCall {
                owner: target_operations::CallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal: ordinal
                        .try_into()
                        .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
                },
                callee: cleanup.cleanup_machine,
                call_plan: signature.call_plan,
                arguments,
            });
        }
    }
    Ok(())
}
