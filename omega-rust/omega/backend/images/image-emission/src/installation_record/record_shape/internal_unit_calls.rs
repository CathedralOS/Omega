//! The internal Unit call rows: each call's retained custody replays
//! exactly against the caller's parameter homes, the callee's ABI and the
//! call plan, in physical call order. `validate_internal_unit_calls` builds
//! one `InternalUnitCall` per row from the callee and custody facts, then runs
//! the checks in `call_roster` and `argument_custody`.

use machine_code::StructuralReturnRecord;
use semantic_vocabulary::PlaceId;
mod argument_custody;
mod call_roster;

use crate::installation_record::{
    CallSignature, CallSiteOwner, CallingPolicy, InstallationError, InstallationRecord,
    InstalledFunction, InstalledInternalUnitCall, MachineId, ValueShape, borrowed_structural,
    evaluate_call_plan, incoming_structural,
};

pub(super) fn validate_internal_unit_calls(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_call = None;
    for installed in &record.internal_unit_calls {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::InvalidInternalUnitCall(installed.machine),
        )?;
        let custody = &installed.custody;
        if borrowed_structural::has_borrowed(function) || custody.arguments.iter().any(|argument| {
            matches!(
                argument.source,
                machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedByteView { .. }
                    | machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedElementView { .. }
                    | machine_code::InternalUnitStructuralArgumentSourceRecord::EstablishedPrimitiveLocal { .. }
                    | machine_code::InternalUnitStructuralArgumentSourceRecord::BlockParameter { .. }
            )
        }) {
            let key = (
                installed.machine,
                custody.code_offset,
                custody.operation_ordinal,
            );
            if previous_call.is_some_and(|previous| previous >= key)
                || !borrowed_structural::call_is_exact(record, function, installed)
            {
                return Err(InstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
            previous_call = Some(key);
            continue;
        }
        let incoming_call = validate_call_source(record, function, installed)?;
        let callee = callee_facts(record, function_by_machine, custody);
        let (expected_text_offset, end, plan) =
            call_span_and_plan(record, function, installed, custody, callee)?;
        // The image retains physical call order. Semantic operation ordinals
        // follow the source block roster and need not increase in that order;
        // owner_valid separately binds each ordinal to its exact source span.
        let key = (
            installed.machine,
            custody.code_offset,
            custody.operation_ordinal,
        );
        let function_unit_calls = record
            .internal_unit_calls
            .iter()
            .filter(|call| call.machine == installed.machine)
            .map(|call| call.custody.clone())
            .collect::<Vec<_>>();
        let facts = custody_facts(function, custody, &function_unit_calls)?;
        let call = InternalUnitCall {
            record,
            function,
            installed,
            custody,
            plan: &plan,
            end,
            incoming_call,
            callee_parameter_abi: callee.callee_parameter_abi,
            callee_unit_parameters: callee.callee_unit_parameters,
            callee_mixed_abi: callee.callee_mixed_abi,
            callee_mixed_structural_return: callee.callee_mixed_structural_return,
            parameter_homes: facts.parameter_homes,
            affine_cleanup: facts.affine_cleanup,
            projected_result: facts.projected_result,
            fully_consumed_affine_parameter: facts.fully_consumed_affine_parameter,
            continuation_discards: &facts.continuation_discards,
            projected_argument_indexes: &facts.projected_argument_indexes,
            transferred_argument_indexes: &facts.transferred_argument_indexes,
            function_unit_calls: &function_unit_calls,
        };
        call.validate_projected_continuation()?;
        call.validate_result_home()?;
        if previous_call.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || end > function.byte_count
            || !function_by_machine.contains_key(&custody.target)
            || custody.result.is_some() != callee.target_returns_scalar
            || custody
                .semantic_result
                .as_ref()
                .map(|result| result.scalar_type)
                != custody.result
            || function_by_machine
                .get(&custody.target)
                .is_some_and(|target| {
                    target
                        .structural_call_scalar_return
                        .is_some_and(|returned| custody.result != Some(returned.scalar_type))
                })
            || !callee.structural_result_valid
            || (custody.structural_result.is_some() && callee.target_returns_scalar)
            || !call.owner_is_valid()
            || !call.mixed_roster_is_exact()
            || plan.parameters.len() != custody.scalar_arguments.len() + custody.arguments.len()
            || custody.arguments.windows(2).any(|pair| {
                pair[0]
                    .code_offset
                    .checked_add(pair[0].byte_count)
                    .is_none_or(|end| end > pair[1].code_offset)
            })
            || call.arguments_lack_exact_custody()
            || call.projected_arguments_are_unsettled()
            || claim_transfers_are_malformed(custody)
        {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
        previous_call = Some(key);
    }
    Ok(())
}

/// One internal Unit call under validation: the record and function it sits
/// in, its custody, the call plan it must match, the callee facts, and the
/// custody facts every check reads. The checks live in `call_roster` and
/// `argument_custody`.
#[derive(Clone, Copy)]
struct InternalUnitCall<'a> {
    record: &'a InstallationRecord,
    function: &'a InstalledFunction,
    installed: &'a InstalledInternalUnitCall,
    custody: &'a machine_code::InternalUnitCallRecord,
    plan: &'a calling_conventions::CallPlan,
    end: usize,
    incoming_call: bool,
    callee_parameter_abi: Option<&'a machine_code::ParameterFunctionAbiRecord>,
    callee_unit_parameters: &'a [machine_code::UnitParameterRecord],
    callee_mixed_abi: Option<&'a target_operations::MixedStructuralScalarFunctionAbi>,
    callee_mixed_structural_return: Option<&'a StructuralReturnRecord>,
    parameter_homes: &'a [machine_code::UnitParameterHomeRecord],
    affine_cleanup: Option<&'a machine_code::UnitAffineCleanupRecord>,
    projected_result: Option<&'a machine_code::InternalStructuralCallResult>,
    fully_consumed_affine_parameter: bool,
    continuation_discards: &'a [PlaceId],
    projected_argument_indexes: &'a std::collections::BTreeSet<usize>,
    transferred_argument_indexes: &'a std::collections::BTreeSet<usize>,
    function_unit_calls: &'a [machine_code::InternalUnitCallRecord],
}

/// What the callee contributes: its ABI, parameters, mixed contract and return.
#[derive(Clone, Copy)]
struct CalleeFacts<'a> {
    callee_parameter_abi: Option<&'a machine_code::ParameterFunctionAbiRecord>,
    callee_unit_parameters: &'a [machine_code::UnitParameterRecord],
    callee_mixed_abi: Option<&'a target_operations::MixedStructuralScalarFunctionAbi>,
    target_returns_scalar: bool,
    target_structural_return: Option<&'a StructuralReturnRecord>,
    structural_result_valid: bool,
    callee_mixed_structural_return: Option<&'a StructuralReturnRecord>,
}

/// What the caller's custody contributes: projected and transferred argument
/// indexes, the cleanup and homes in force, the projected result and the
/// continuation discards.
struct CustodyFacts<'a> {
    projected_argument_indexes: std::collections::BTreeSet<usize>,
    transferred_argument_indexes: std::collections::BTreeSet<usize>,
    affine_cleanup: Option<&'a machine_code::UnitAffineCleanupRecord>,
    parameter_homes: &'a [machine_code::UnitParameterHomeRecord],
    fully_consumed_affine_parameter: bool,
    projected_result: Option<&'a machine_code::InternalStructuralCallResult>,
    continuation_discards: Vec<PlaceId>,
}

/// Every argument has a placement, and an incoming-pointer call replays its
/// incoming copies exactly while any other call is authored.
fn validate_call_source(
    record: &InstallationRecord,
    function: &InstalledFunction,
    installed: &InstalledInternalUnitCall,
) -> Result<bool, InstallationError> {
    let custody = &installed.custody;
    if custody
        .arguments
        .iter()
        .any(|argument| argument.source.placement().is_none())
    {
        return Err(InstallationError::InvalidInternalUnitCall(
            installed.machine,
        ));
    }
    // Incoming pointer homes describe the caller's parameters, not every
    // call it makes. Only arguments transported from those homes use the
    // legacy incoming-copy checks; a parameterless call still has its
    // ordinary call ABI and attribution even inside an owned-value caller.
    let incoming_call = custody.arguments.iter().any(|argument| {
        matches!(
            argument.source_location,
            machine_code::StructuralSourceLocation::IncomingIndirectPointer { .. }
                | machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
        )
    });
    if (incoming_call && !incoming_structural::call_is_exact(record, function, installed))
        || (!incoming_call
            && !matches!(
                custody.source,
                machine_code::InternalUnitCallSource::Authored
            ))
    {
        return Err(InstallationError::InvalidInternalUnitCall(
            installed.machine,
        ));
    }
    Ok(incoming_call)
}

/// The callee's ABI, parameters, mixed contract and structural return, and
/// whether this call's result shape agrees with them.
fn callee_facts<'a>(
    record: &'a InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &'a InstalledFunction>,
    custody: &machine_code::InternalUnitCallRecord,
) -> CalleeFacts<'a> {
    let callee_parameter_abi = function_by_machine
        .get(&custody.target)
        .and_then(|target| target.parameter_abi.as_ref());
    let callee_unit_parameters = function_by_machine
        .get(&custody.target)
        .map_or(&[][..], |target| target.unit_parameters.as_slice());
    let callee_mixed_abi = function_by_machine
        .get(&custody.target)
        .and_then(|target| target.mixed_structural_scalar_abi.as_ref());
    let target_returns_scalar = function_by_machine
        .get(&custody.target)
        .is_some_and(|target| {
            target.scalar_stack.is_some() || target.structural_call_scalar_return.is_some()
        });
    let target_structural_return = record
        .structural_returns
        .iter()
        .find(|target| target.machine == custody.target)
        .map(|target| &target.returned);
    let structural_result_valid = match (&custody.structural_result, target_structural_return) {
        (None, None) => true,
        (Some(result), Some(target)) => custody.result.is_none()
            && custody
                .arguments
                .iter()
                .all(|argument| argument.place != result.operation_result.place)
            && crate::object_artifact::replay::unit::call_custody::structural_result_matches_return(
                result, target,
            ),
        _ => false,
    };
    let callee_mixed_structural_return = target_structural_return.filter(|returned| {
        !returned.scalar_parameters.is_empty()
            || crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned)
    });
    CalleeFacts {
        callee_parameter_abi,
        callee_unit_parameters,
        callee_mixed_abi,
        target_returns_scalar,
        target_structural_return,
        structural_result_valid,
        callee_mixed_structural_return,
    }
}

/// The call's text span and the call plan its arguments must realize.
fn call_span_and_plan(
    record: &InstallationRecord,
    function: &InstalledFunction,
    installed: &InstalledInternalUnitCall,
    custody: &machine_code::InternalUnitCallRecord,
    callee: CalleeFacts<'_>,
) -> Result<(usize, usize, calling_conventions::CallPlan), InstallationError> {
    let CalleeFacts {
        callee_parameter_abi,
        callee_unit_parameters,
        callee_mixed_abi,
        target_structural_return,
        callee_mixed_structural_return,
        ..
    } = callee;
    let expected_text_offset = function
        .text_offset
        .checked_add(custody.code_offset)
        .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let end = custody
        .code_offset
        .checked_add(custody.byte_count)
        .ok_or(InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(record.target),
        &CallSignature {
            parameters: if let Some(abi) = callee_parameter_abi {
                abi.parameters
                    .iter()
                    .map(|parameter| {
                        crate::object_artifact::replay::unit::call_custody::unit_scalar_shape(
                            parameter.scalar_type,
                        )
                        .ok_or(InstallationError::InvalidInternalUnitCall(
                            installed.machine,
                        ))
                    })
                    .chain(
                        callee_unit_parameters
                            .iter()
                            .map(|parameter| Ok(parameter.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(abi) = callee_mixed_abi {
                abi.scalar_parameters
                    .iter()
                    .map(|parameter| {
                        let semantic_vocabulary::ScalarType::Integer(integer) =
                            parameter.scalar_type
                        else {
                            return Err(InstallationError::InvalidInternalUnitCall(
                                installed.machine,
                            ));
                        };
                        if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                            return Err(InstallationError::InvalidInternalUnitCall(
                                installed.machine,
                            ));
                        }
                        let bytes = integer.bits() / 8;
                        Ok(ValueShape::integer(bytes, bytes))
                    })
                    .chain(
                        abi.structural_parameters
                            .iter()
                            .map(|parameter| Ok(parameter.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else if let Some(returned) = callee_mixed_structural_return {
                returned
                    .scalar_parameters
                    .iter()
                    .map(|parameter| {
                        let semantic_vocabulary::ScalarType::Integer(integer) =
                            parameter.scalar_type
                        else {
                            return Err(InstallationError::InvalidInternalUnitCall(
                                installed.machine,
                            ));
                        };
                        if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
                            return Err(InstallationError::InvalidInternalUnitCall(
                                installed.machine,
                            ));
                        }
                        let bytes = integer.bits() / 8;
                        Ok(ValueShape::integer(bytes, bytes))
                    })
                    .chain(
                        returned
                            .parameter_placements
                            .iter()
                            .map(|placement| Ok(placement.shape)),
                    )
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                custody
                    .arguments
                    .iter()
                    .map(|argument| argument.shape)
                    .collect()
            },
            result: if let Some(result) = custody.result {
                let bytes = match result {
                    semantic_vocabulary::ScalarType::Boolean => 1,
                    semantic_vocabulary::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
                    semantic_vocabulary::ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary32,
                    ) => 4,
                    semantic_vocabulary::ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ) => 8,
                };
                Some(match result {
                    semantic_vocabulary::ScalarType::IeeeFloat(_) => ValueShape::float(bytes),
                    _ => ValueShape::integer(bytes, bytes.next_power_of_two().min(8)),
                })
            } else if custody.structural_result.is_some() {
                target_structural_return.map(|returned| returned.shape)
            } else {
                None
            },
        },
    )
    .map_err(|_| InstallationError::InvalidInternalUnitCall(installed.machine))?;
    Ok((expected_text_offset, end, plan))
}

/// The custody facts the checks share.
fn custody_facts<'a>(
    function: &'a InstalledFunction,
    custody: &'a machine_code::InternalUnitCallRecord,
    function_unit_calls: &'a [machine_code::InternalUnitCallRecord],
) -> Result<CustodyFacts<'a>, InstallationError> {
    let installed_machine = function.machine;
    let projected_argument_indexes = custody
        .arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
        .collect::<std::collections::BTreeSet<_>>();
    let transferred_argument_indexes = custody
        .claim_transfers
        .iter()
        .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
        .collect::<std::collections::BTreeSet<_>>();
    let control_cleanup = match custody.owner {
        CallSiteOwner::CleanupAction { edge, .. } => function
            .scalar_control_affine_cleanups
            .iter()
            .find(|cleanup| cleanup.psi_edge == edge),
        CallSiteOwner::Operation(_) => None,
    };
    let affine_cleanup = function
        .scalar_affine_cleanup
        .as_ref()
        .or_else(|| {
            crate::object_artifact::replay::unit::continuations::cleanup_for_call(
                &function.unit_continuations,
                custody.operation_ordinal,
            )
        })
        .or(function.unit_affine_cleanup.as_ref())
        .or(control_cleanup);
    let parameter_homes = if function.scalar_structural_parameter_homes.is_empty() {
        function.unit_parameter_homes.as_slice()
    } else {
        function.scalar_structural_parameter_homes.as_slice()
    };
    let fully_consumed_affine_parameter =
        crate::object_artifact::replay::structural::affine_projected_calls::exact_fully_consumed_affine_parameter(
            &function.unit_parameter_homes,
            function_unit_calls,
            function.unit_affine_cleanup.as_ref(),
        );
    let projected_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
        parameter_homes,
        function_unit_calls,
        affine_cleanup,
    )
    .or_else(|| {
        let disposed = crate::object_artifact::replay::unit::continuations::completed_roots(
            parameter_homes,
            function_unit_calls,
            &function.unit_continuations,
            function.unit_affine_cleanup.as_ref(),
        )?;
        crate::object_artifact::replay::unit::continuations::result_for_call(function_unit_calls, &disposed, custody)
    });
    let continuation_discards =
        crate::object_artifact::replay::unit::continuations::completed_roots(
            parameter_homes,
            function_unit_calls,
            &function.unit_continuations,
            function.unit_affine_cleanup.as_ref(),
        )
        .ok_or(InstallationError::InvalidInternalUnitCall(
            installed_machine,
        ))?;
    Ok(CustodyFacts {
        projected_argument_indexes,
        transferred_argument_indexes,
        affine_cleanup,
        parameter_homes,
        fully_consumed_affine_parameter,
        projected_result,
        continuation_discards,
    })
}

/// Claim transfers name existing arguments and each claim at most once.
pub(super) fn claim_transfers_are_malformed(
    custody: &machine_code::InternalUnitCallRecord,
) -> bool {
    custody.claim_transfers.iter().any(|transfer| {
        usize::try_from(transfer.argument_index)
            .map_or(true, |index| index >= custody.arguments.len())
    }) || custody
        .claim_transfers
        .iter()
        .map(|transfer| transfer.claim)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != custody.claim_transfers.len()
}
