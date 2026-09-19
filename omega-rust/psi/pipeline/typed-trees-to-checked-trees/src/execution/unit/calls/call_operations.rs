//! Building one checked call operation and the value result it is expected
//! to produce.

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::byte_subslice;
use crate::execution::terminal_unit::calls::argument_paths::{
    byte_sequence_literal_argument, checked_call_erased_scalar_arguments,
    checked_call_scalar_arguments, ordinary_projected_call_is_supported, projected_argument_path,
    target_contract_mentions_projected_parameter,
};
use crate::execution::terminal_unit::calls::boundary_admission::{
    boundary_argument_presentation_is_admitted, boundary_value_result_matches,
    fixed_byte_array_mutable_view_is_admitted, is_registered_boundary_scalar_target,
    provider_attachment_receiver_matches,
};
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, exact_integer_at, exact_structural_argument_access,
    structural_call_arguments,
};
use crate::execution::terminal_unit::calls::{result_arguments, service_forward};
use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::{
    BuiltinFunction, CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, MachineSupplyMode, Multiplicity, PermissionEventKind,
    PrimitiveType, SymbolHandle, TypeReferenceNode, TypedTrees, base_type_identity,
    byte_sequence_type_identity, is_reference, is_unit, machine_binders, parameter_root_symbol,
    scalar_targets, signature_contracts_are_exact_parameter_qualifications,
    strips_erased_parameter, structural_access_for_type_reference,
};
use validation::exact_compiler_intrinsic_boundary_requirement;

pub(in crate::execution) enum ExpectedCallValueResult<'result> {
    Scalar(PrimitiveType),
    Structural(&'result CheckedUnitStructuralResultBindingPlan),
}

/// Build one call with the scalar-callee evidence available to this pass.
/// `None` supplies no registered scalar targets; it never consults published catalogs.
pub(in crate::execution) fn build_call_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: Option<ScalarCalleePlans<'_>>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &checked_trees::FlowCallFact,
    allow_field_path_projection: bool,
    expected_call_result: Option<ExpectedCallValueResult<'_>>,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
) -> Option<CheckedUnitEffectOperationPlan> {
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(call.statement_index).ok()?,
        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
    };
    let call_site = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let source_site = match &call_site {
        crate::semantic_calls::CallSite::Statement(_) => {
            let offset = u32::try_from(call.statement_index).ok()?;
            Some(checked_trees::NominalMachineUseSite::Statement(
                arena::Handle::from_parts(
                    state
                        .statement_nodes
                        .start()
                        .arena_index()
                        .checked_add(offset)?,
                    state.statement_nodes.start().generation(),
                ),
            ))
        }
        crate::semantic_calls::CallSite::Expression { expression, .. } => Some(
            checked_trees::NominalMachineUseSite::Expression(*expression),
        ),
        crate::semantic_calls::CallSite::TransitionNamed { .. } => None,
    };

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = crate::semantic_calls::call_site_argument_expressions(program, &call_site);
        let [port, value] = arguments else {
            return None;
        };
        return Some(CheckedUnitEffectOperationPlan::PortWrite {
            coordinate,
            port: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *port,
                PrimitiveType::U16,
            )?
            .try_into()
            .ok()?,
            value: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *value,
                PrimitiveType::U8,
            )?
            .try_into()
            .ok()?,
            service_reach: call.service_reach,
        });
    }

    let selected_realization =
        exact_compiler_intrinsic_boundary_requirement(program, call.target_symbol);
    let direct_boundary_target = selected_realization
        .map(|(requirement, _)| requirement)
        .unwrap_or(call.target_symbol);
    // A nominal binder describes an obligation, not a selected boundary body.
    // Specialization must replace it with the exact executable target before
    // this pass can produce a call. Projecting its requirement here would make
    // an open generic entry executable and bypass its selected implementation.
    let static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == direct_boundary_target)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    if let [(definition, signature)] = static_boundaries.as_slice() {
        let arguments = crate::semantic_calls::call_site_argument_expressions(program, &call_site);
        let source_parameters = program.state_signature_parameters(signature);
        // A boundary signature is a foreign ABI contract; an erased position
        // has no agreed foreign transfer yet, so the call fails closed here.
        if source_parameters
            .iter()
            .any(|parameter| parameter.relevance.is_erased())
        {
            return None;
        }
        let abi_parameters = source_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| !parameter.is_self)
            .collect::<Vec<_>>();
        let caller_source_parameters = program.state_parameters(state);
        let signature_type_parameters = program.state_signature_type_parameters(signature);
        // Each `machine` signature type parameter is a nominal binder discharged
        // by a `nominal_machine_use` at this exact call site: same registration
        // operation, binder ordinal, and satisfaction row. The callback's
        // destination comes from one of two places — an ABI `native callback`
        // parameter declared at the same telescope ordinal, or a target-owned
        // private slot whose `PrivateCallbackSlot` conformance supplied the
        // evaluated boundary calling plan retained on the use.
        let nominal_use_at_binder =
            |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
                let typed_trees::data::TypeParameterKind::Machine {
                    contract:
                        typed_trees::data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                        },
                } = parameter.kind
                else {
                    return None;
                };
                facts.nominal_machine_uses.uses.iter().find(|nominal_use| {
                    Some(nominal_use.site) == source_site
                        && nominal_use.registration_operation == signature.symbol
                        && usize::try_from(nominal_use.static_machine_ordinal).ok() == Some(ordinal)
                        && nominal_use.satisfaction_trait == trait_definition
                        && nominal_use.satisfaction_requirement == requirement
                })
            };
        let admitted_callback_telescope = source_site.is_some()
            && signature.native_callback_parameters.len() <= signature_type_parameters.len()
            && signature_type_parameters
                .iter()
                .enumerate()
                .all(|(ordinal, parameter)| {
                    let Some(nominal_use) = nominal_use_at_binder(ordinal, parameter) else {
                        return false;
                    };
                    match signature.native_callback_parameters.get(ordinal) {
                        Some(callback) => parameter.name == callback.binder,
                        None => nominal_use.callback_placement.is_some(),
                    }
                });
        let routed_service_parameter_receiver = caller_parameters.iter().any(|parameter| {
            parameter
                .fused_service_erasure
                .as_ref()
                .is_some_and(|receipt| {
                    receipt.source_parameter == call.receiver_symbol
                        && receipt.requirement == definition.symbol
                })
        });
        let mut scalar_parameters = Vec::new();
        let mut structural_arguments = Vec::new();
        for (abi_position, ((_, parameter), argument)) in
            abi_parameters.iter().zip(arguments.iter()).enumerate()
        {
            let source_position = u32::try_from(abi_position).ok()?;
            if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
            {
                scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                    source_position,
                    primitive_type,
                });
                continue;
            }
            let byte_sequence = byte_sequence_carrier(program, parameter.type_reference, &[]);
            if validation::is_closed_primitive_array_type(program, parameter.type_reference) {
                return None;
            }
            let target_identity = if byte_sequence.is_some() {
                byte_sequence_type_identity(program, parameter.type_reference, &[], &[])?
            } else {
                base_type_identity(program, parameter.type_reference, &[])?
            };
            if let Some(literal) =
                byte_sequence_literal_argument(program, parameter.type_reference, *argument)
            {
                structural_arguments.push(literal);
                continue;
            }
            if let Some(subslice) = byte_subslice::argument(
                program,
                facts,
                machine,
                state,
                caller_parameters,
                parameter.type_reference,
                *argument,
                call.statement_index,
                call.call_ordinal,
                structural_arguments.len(),
            ) {
                structural_arguments.push(subslice);
                continue;
            }

            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                call.statement_index,
                *argument,
            )?;
            if let Some((result, _)) = caller_structural_results
                .iter()
                .find(|(_, root)| *root == place.root)
            {
                if result.multiplicity == Multiplicity::Unrestricted {
                    return None;
                }
                structural_arguments.push(result_arguments::argument(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    call,
                    *argument,
                    &place,
                    result,
                    parameter,
                    &target_identity,
                    false,
                )?);
                continue;
            }
            let facts::PlaceRoot::Symbol(source_symbol) = place.root else {
                return None;
            };
            // An authored `self.field` argument roots at the `self` parameter's
            // own symbol, while an implicit receiver place roots at the machine
            // the parameter is attached to. Both spellings name one parameter.
            let source_parameter = caller_source_parameters.iter().find(|candidate| {
                parameter_root_symbol(machine.symbol, candidate) == source_symbol
                    || candidate.symbol == source_symbol
            })?;
            if validation::is_closed_primitive_array_type(program, source_parameter.type_reference)
            {
                return None;
            }
            let source_position = caller_source_parameters
                .iter()
                .position(|candidate| candidate.symbol == source_parameter.symbol)?;
            let source_parameter_index = caller_parameters.iter().position(|candidate| {
                candidate.position == u32::try_from(source_position).unwrap_or(u32::MAX)
            })?;
            let caller_parameter = caller_parameters.get(source_parameter_index)?;
            let path = if place.segments.is_empty() {
                // A whole array and its field projection lend the same fixed
                // range. Absence of a projection does not require a fake view
                // type identity; access and source custody still replay below.
                if caller_parameter.type_identity != target_identity
                    && !(caller_parameter.qualifications.is_empty()
                        && fixed_byte_array_mutable_view_is_admitted(
                            program,
                            source_parameter.type_reference,
                            parameter.type_reference,
                        ))
                {
                    return None;
                }
                Vec::new()
            } else {
                if !caller_parameter.qualifications.is_empty() {
                    return None;
                }
                let (projected_type, path) =
                    projected_argument_path(program, state.symbol, call.statement_index, &place)?;
                if !boundary_argument_presentation_is_admitted(
                    program,
                    projected_type,
                    parameter.type_reference,
                    &target_identity,
                ) && !fixed_byte_array_mutable_view_is_admitted(
                    program,
                    projected_type,
                    parameter.type_reference,
                ) {
                    return None;
                }
                path
            };
            structural_arguments.push(CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(source_parameter_index).ok()?,
                },
                path,
                type_identity: target_identity,
                access: exact_structural_argument_access(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    call,
                    &place,
                    structural_access_for_type_reference(program, parameter.type_reference)?,
                )?,
            });
        }
        if !program.trait_type_parameters(definition).is_empty()
            || (!signature_type_parameters.is_empty() && !admitted_callback_telescope)
            || program
                .state_signature_parameters(signature)
                .iter()
                // `is_mutable` is set by a `mut` binding and by an exclusive
                // borrow alike, and the requirement's exclusive borrow is
                // already carried exactly by the argument's presented access.
                // An owned mutable scalar is an argument destination, not
                // mutable storage in this Unit caller. Other owned mutable
                // carriers remain outside this call surface.
                .any(|parameter| {
                    !parameter.is_self
                        && (parameter.is_const
                            || (parameter.is_mutable
                                && !is_reference(program, parameter.type_reference)
                                && crate::values::mutable_scalar_parameter_type(
                                    program, parameter,
                                )
                                .is_none()))
                })
            || arguments.len() != abi_parameters.len()
            || if let Some((_, qualifier)) = selected_realization {
                call.has_receiver && call.receiver_symbol != qualifier
            } else {
                !call.has_receiver
                    || (call.receiver_symbol != definition.symbol
                        && !provider_attachment_receiver_matches(
                            program,
                            machine,
                            &call_site,
                            definition.symbol,
                        )
                        && !routed_service_parameter_receiver)
            }
            || validation::is_closed_primitive_array_type(program, signature.return_type)
            || match expected_call_result {
                None => !is_unit(program, signature.return_type),
                Some(ref expected) => {
                    !boundary_value_result_matches(program, signature.return_type, expected)
                }
            }
            || !signature_contracts_are_exact_parameter_qualifications(program, signature)
            // Suspension parks the activation, which this synchronous call
            // shape cannot express. Blocking only occupies the worker while
            // the boundary waits (effects.md, `blocks;`): the call returns
            // through the same edge, the site already acknowledged `block`
            // during checking, and the envelope travels on the target
            // contract fingerprint below, so a blocking boundary is planned
            // exactly like a nonblocking one.
            || signature.suspends
        {
            return None;
        }
        let capsule = facts
            .contract_plans
            .crash_capsule(definition.symbol, signature.symbol)?;
        // Trait calls transfer each owned argument into the boundary. Their
        // permission events are transfers, not the terminal consumption used
        // by an owned receiver. Reuse the exact call-site custody replay so
        // every live claim has a normal-completion receipt at its argument.
        let completion_receipts = call_claim_transfers(
            facts,
            machine.symbol,
            state.symbol,
            call,
            caller_parameters,
            entry_claims,
            &structural_arguments,
            PermissionEventKind::Transfer,
        )?;
        return Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine: signature.symbol,
            target_state: signature.symbol,
            target_contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
            service_reach: call.service_reach,
            scalar_arguments: checked_call_scalar_arguments(
                facts,
                state.symbol,
                coordinate,
                &scalar_parameters,
                true,
            )?,
            structural_arguments,
            completion_receipts,
        });
    }
    if !static_boundaries.is_empty() {
        return None;
    }

    let target_state = crate::semantic_calls::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    let boundary = target_machine.supply_mode.is_boundary_declaration();
    // Boundary results currently carry identity/claims, not an array payload.
    // Ordinary calls get their payload from the independently checked body.
    if boundary && validation::is_closed_primitive_array_type(program, target_state.return_type) {
        return None;
    }
    if if boundary {
        match expected_call_result {
            None => !is_unit(program, target_state.return_type),
            Some(ref expected) => {
                !boundary_value_result_matches(program, target_state.return_type, expected)
            }
        }
    } else {
        match &expected_call_result {
            None => !is_unit(program, target_state.return_type),
            Some(ExpectedCallValueResult::Scalar(expected)) => {
                program.primitive_type_reference(target_state.return_type) != Some(*expected)
            }
            Some(ExpectedCallValueResult::Structural(expected))
                if crate::execution::terminal_unit::reference_results::parts(
                    program,
                    target_state.return_type,
                )
                .is_some() =>
            {
                expected.multiplicity != Multiplicity::Affine
                    || program
                        .normalized_type_identity(target_state.return_type)
                        .as_str()
                        != expected.type_identity
            }
            Some(expected @ ExpectedCallValueResult::Structural(_)) => {
                !boundary_value_result_matches(program, target_state.return_type, expected)
            }
        }
    } {
        return None;
    }
    if !boundary && target_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let structural_arguments = structural_call_arguments(
        program,
        facts,
        scalar_callees,
        call,
        machine,
        state,
        caller_parameters,
        caller_trivial_affine_locals,
        target_machine,
        target_state,
        &call_site,
        call.receiver_symbol,
        call.statement_index,
        true,
        allow_field_path_projection,
        caller_structural_results,
    )?;
    // The callee's retained scalar positions: the same authored indices its
    // own signature plan keeps, so the erased position is absent on both
    // sides and `checked_call_scalar_arguments` pairs the caller's dense
    // argument ordinals with the retained parameters only.
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in program.state_parameters(target_state).iter().enumerate() {
        if strips_erased_parameter(parameter)? {
            continue;
        }
        let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
        else {
            continue;
        };
        if parameter.is_self
            || parameter.is_const
            || (parameter.is_mutable
                && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
        {
            return None;
        }
        scalar_parameters.push(CheckedStructuralScalarParameterPlan {
            source_position: u32::try_from(position).ok()?,
            primitive_type,
        });
    }
    let scalar_arguments = if boundary {
        checked_call_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters, true)?
    } else {
        checked_call_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters, false)?
    };
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(
            program,
            target_state,
        )?;
    // Boundary calls and bodyless targets own no erased proof lane; retained
    // erased formals belong to in-module Unit callees only.
    let erased_scalar_arguments = if boundary {
        if !erased_scalar_parameters.is_empty() {
            return None;
        }
        Vec::new()
    } else {
        checked_call_erased_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &erased_scalar_parameters,
        )?
    };
    if !boundary {
        let carries_routed_service = caller_parameters
            .iter()
            .any(|parameter| parameter.fused_service_erasure.is_some())
            || program
                .state_parameters(target_state)
                .iter()
                .any(|parameter| {
                    typed_trees::service::exact_bound_service_requirement(
                        program,
                        parameter.type_reference,
                    )
                    .is_some()
                });
        let supported = if carries_routed_service {
            service_forward::exact_single_fused_service_forward_is_supported(
                program,
                facts,
                machine,
                state,
                caller_parameters,
                target_machine,
                target_state,
                call,
                &structural_arguments,
            )
        } else {
            ordinary_projected_call_is_supported(
                program,
                facts,
                machine,
                state,
                caller_parameters,
                target_machine,
                target_state,
                &structural_arguments,
                allow_field_path_projection,
            ) || projected_reference_record_operands_supported(
                program,
                facts,
                machine,
                state,
                target_machine,
                target_state,
                &structural_arguments,
            )
        };
        if !supported {
            return None;
        }
    }
    if !boundary
        && let Some(ExpectedCallValueResult::Structural(result)) = &expected_call_result
        && result.multiplicity == Multiplicity::Linear
    {
        if !machine_binders(program, target_machine).is_empty()
            || !program
                .machine_states(target_machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_state.symbol)
        {
            return None;
        }
        let mut operation = CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            source_site,
            result: (*result).clone(),
            custody: Default::default(),
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_report_fingerprint: target_contract.report_fingerprint,
            target_contract_commitment: target_contract.commitment,
            service_reach: call.service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            discard_result_on_return: false,
        };
        let custody = validation::reconstruct_structural_call_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &operation,
        )
        .ok()?;
        let CheckedUnitEffectOperationPlan::StructuralCall {
            custody: retained, ..
        } = &mut operation
        else {
            return None;
        };
        *retained = custody;
        return Some(operation);
    }
    let transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        &structural_arguments,
        if boundary {
            PermissionEventKind::Consume
        } else {
            PermissionEventKind::Transfer
        },
    )?;

    if boundary {
        Some(CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_report_fingerprint: target_contract.report_fingerprint,
            service_reach: call.service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts: transfers,
        })
    } else if let Some(ExpectedCallValueResult::Structural(result)) = expected_call_result {
        let reference_loan = if crate::execution::terminal_unit::reference_results::parts(
            program,
            target_state.return_type,
        )
        .is_some()
        {
            crate::execution::terminal_unit::reference_results::result_loan(
                program,
                facts,
                machine.symbol,
                state,
                call,
                result,
            )?
        } else {
            arena::Handle::invalid()
        };
        // A result signature is available before its ordinary or graph body plan.
        // The closure pass below retains this call only when that complete body
        // was produced, avoiding an authored machine-order dependency.
        let args_ok = structural_arguments
            .iter()
            .enumerate()
            .all(|(argument_index, argument)| {
                if argument.access == CheckedStructuralAccess::Owned
                    && (argument.source_parameter_index().is_some()
                        || argument
                            .source_structural_result_binding_ordinal()
                            .is_some())
                    && program
                        .state_parameters(target_state)
                        .iter()
                        .filter(|parameter| {
                            program
                                .primitive_type_reference(parameter.type_reference)
                                .is_none()
                                && !(parameter.is_self
                                    && is_reference(program, parameter.type_reference))
                        })
                        .nth(argument_index)
                        .is_some_and(|parameter| {
                            !parameter.is_self
                                && (validation::is_closed_primitive_array_type(
                                    program,
                                    parameter.type_reference,
                                ) || validation::has_plain_owned_contents_with_numeric_constraints(
                                    program,
                                    parameter.type_reference,
                                ) || validation::reference_result_custody::is_reference_record(
                                    program,
                                    parameter.type_reference,
                                ))
                                && base_type_identity(program, parameter.type_reference, &[])
                                    .is_some_and(|identity| identity == argument.type_identity)
                                // A projected owned operand names the exact
                                // declared-field subtree whose captured leaf the
                                // bare reference result loan already replayed.
                                // Without that proven leaf custody the whole
                                // carrier spelling stays mandatory.
                                && (argument.path.is_empty()
                                    || (reference_loan.is_valid()
                                        && validation::reference_result_custody::is_reference_record(
                                            program,
                                            parameter.type_reference,
                                        )
                                        && argument.path.iter().all(|segment| {
                                            matches!(
                                                segment,
                                                checked_trees::CheckedUnitStructuralPathSegment::Field(
                                                    _
                                                )
                                            )
                                        })))
                        })
                {
                    return true;
                }
                (argument.source_parameter_index().is_some()
                    || argument.byte_sequence_literal().is_some()
                    || matches!(
                        argument.source,
                        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                    ))
                    && matches!(
                        argument.access,
                        CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::MutableBorrow
                    )
            });
        if args_ok
            && transfers.is_empty()
            && (reference_loan.is_valid()
                || crate::execution::terminal_unit::reference_results::is_reference_record(
                    program,
                    target_state.return_type,
                )
                || (matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                ) && validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    target_state.return_type,
                ) && matches!(
                    program
                        .type_reference_table
                        .type_reference(target_state.return_type),
                    TypeReferenceNode::Named { .. }
                ))
                || (result.multiplicity == Multiplicity::Unrestricted
                    && validation::is_closed_primitive_array_type(
                        program,
                        target_state.return_type,
                    )))
            && program
                .machine_states(target_machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_state.symbol)
            && machine_binders(program, target_machine).is_empty()
        {
            return Some(CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                source_site,
                result: result.clone(),
                custody: checked_trees::CheckedStructuralCallCustodyPlan {
                    reference_loan,
                    ..Default::default()
                },
                target_machine: target_machine.symbol,
                target_state: target_state.symbol,
                target_contract_report_fingerprint: target_contract.report_fingerprint,
                target_contract_commitment: target_contract.commitment,
                service_reach: call.service_reach,
                scalar_arguments,
                erased_scalar_arguments,
                structural_arguments,
                discard_result_on_return: result.multiplicity == Multiplicity::Affine
                    && !reference_loan.is_valid(),
            });
        }
        let target = facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_for_machine(target_machine.symbol)?;
        let [argument] = structural_arguments.as_slice() else {
            return None;
        };
        let source_matches = if let Some(index) = argument.source_parameter_index() {
            let source = caller_parameters.get(usize::try_from(index).ok()?)?;
            source.type_identity == argument.type_identity
                && source.multiplicity == Multiplicity::Affine
                && source.access == CheckedStructuralAccess::Owned
                && source.qualifications.is_empty()
        } else if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
            caller_structural_results.iter().any(|(source, _)| {
                source.binding_ordinal == ordinal
                    && source.statement_index <= coordinate.statement_index
                    && source.type_identity == argument.type_identity
                    && source.multiplicity == Multiplicity::Affine
            })
        } else {
            false
        };
        if target.state != target_state.symbol
            || target.result.type_identity != result.type_identity
            || result.multiplicity != Multiplicity::Affine
            || target.scalar_parameters != scalar_parameters
            || target.structural_parameter.type_identity != argument.type_identity
            || !source_matches
            || argument.access != CheckedStructuralAccess::Owned
            || !argument.path.is_empty()
            || !transfers.is_empty()
            || !service_reach_is_empty(facts, call.service_reach)
        {
            return None;
        }
        Some(CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            source_site,
            result: result.clone(),
            custody: Default::default(),
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_report_fingerprint: target_contract.report_fingerprint,
            target_contract_commitment: target_contract.commitment,
            service_reach: call.service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            discard_result_on_return: true,
        })
    } else if expected_call_result.is_some()
        && (!structural_arguments.is_empty() || !transfers.is_empty())
        && !matches!(expected_call_result, Some(ExpectedCallValueResult::Scalar(result))
            if is_registered_boundary_scalar_target(
                scalar_callees, target_machine.symbol, target_state.symbol, result)
                || scalar_targets::registered_primitive_store_target(
                    program, facts, scalar_callees, target_machine.symbol, target_state.symbol, result).is_some()
                || scalar_targets::registered_structural_graph_target(
                    program, facts, scalar_callees, target_machine.symbol, target_state.symbol, result).is_some())
    {
        None
    } else {
        Some(CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_report_fingerprint: target_contract.report_fingerprint,
            service_reach: call.service_reach,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            claim_transfers: transfers,
        })
    }
}

/// A projected owned operand may select one declared-field subtree out of an
/// earlier structural result when the target parameter is a reference-bearing
/// record. Argument construction already replayed every captured leaf loan
/// under that edge, and the bare reference result's loan re-derives the same
/// ingress at the consuming call, so the projected path admits here.
fn projected_reference_record_operands_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    arguments: &[CheckedUnitStructuralArgumentPlan],
) -> bool {
    if target_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return false;
    }
    let target_parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && !(parameter.is_self && is_reference(program, parameter.type_reference))
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if target_parameters.len() != arguments.len()
        || arguments.iter().all(|argument| argument.path.is_empty())
    {
        return false;
    }
    let has_content_evidence = |machine, state| {
        facts
            .qualifications
            .content
            .identity_reshuffles
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
            || facts
                .qualifications
                .content
                .partition_compositions
                .iter()
                .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
    };
    if has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol)
        || arguments
            .iter()
            .zip(&target_parameters)
            .any(|(argument, parameter)| {
                !argument.path.is_empty()
                    && target_contract_mentions_projected_parameter(
                        program,
                        facts,
                        target_machine,
                        target_state,
                        parameter,
                    )
            })
    {
        return false;
    }
    arguments
        .iter()
        .zip(&target_parameters)
        .all(|(argument, target)| {
            argument.path.is_empty()
                || (argument
                    .source_structural_result_binding_ordinal()
                    .is_some()
                    && argument.access == CheckedStructuralAccess::Owned
                    && argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        )
                    })
                    && !target.is_self
                    && validation::reference_result_custody::is_reference_record(
                        program,
                        target.type_reference,
                    )
                    && base_type_identity(program, target.type_reference, &[])
                        .is_some_and(|identity| identity == argument.type_identity))
        })
}
