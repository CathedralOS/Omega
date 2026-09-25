//! Building one checked call operation and the value result it is expected
//! to produce.

use crate::execution::terminal_unit::ScalarCalleePlans;

use crate::execution::terminal_unit::calls::argument_paths::{
    checked_call_erased_proof_arguments, checked_call_erased_scalar_arguments,
    checked_call_scalar_arguments, ordinary_projected_call_is_supported,
    target_contract_mentions_projected_parameter,
};
use crate::execution::terminal_unit::calls::boundary_admission::boundary_value_result_matches;
use crate::execution::terminal_unit::calls::service_forward;
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, exact_integer_at, structural_call_arguments,
};
use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{base_type_identity, is_unit, machine_binders};
use crate::execution::terminal_unit::{
    BuiltinFunction, CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, MachineSupplyMode, Multiplicity, PermissionEventKind,
    PrimitiveType, SymbolHandle, TypeReferenceNode, TypedTrees, is_reference,
    strips_erased_parameter,
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
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(call.statement_index).ok()?,
        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
    };
    // Each guard family below marks the trace before it can decline, so an
    // omission at this call names the requirement that refused it rather
    // than the whole builder.
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: call site");
    let call_site = crate::semantic::calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let source_site = match &call_site {
        crate::semantic::calls::CallSite::Statement(_) => {
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
        crate::semantic::calls::CallSite::Expression { expression, .. } => Some(
            checked_trees::NominalMachineUseSite::Expression(*expression),
        ),
        crate::semantic::calls::CallSite::TransitionNamed { .. } => None,
    };

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &call_site);
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
    // A call through a boundary trait requirement plans against the foreign
    // ABI contract; every other target takes the ordinary route below.
    if let [(definition, signature)] = static_boundaries.as_slice() {
        return super::trait_boundary_call::build(
            program,
            facts,
            machine,
            state,
            caller_parameters,
            entry_claims,
            call,
            expected_call_result.as_ref(),
            caller_structural_results,
            trace,
            coordinate,
            &call_site,
            source_site,
            definition,
            signature,
            selected_realization,
        );
    }
    if !static_boundaries.is_empty() {
        return None;
    }

    phase("call operation: target state");
    let target_state = crate::semantic::calls::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    phase("call operation: target contract");
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    // A bodied `boundary machine` is a checked adapter: callers reach its
    // authored body as an ordinary callee; only the bodyless declaration is
    // a boundary edge.
    let boundary =
        target_machine.supply_mode.is_boundary_declaration() && !target_machine.body_is_present;
    // Boundary results currently carry identity/claims, not an array payload.
    // Ordinary calls get their payload from the independently checked body.
    phase("call operation: result shape");
    if boundary && validation::is_closed_primitive_array_type(program, target_state.return_type) {
        return None;
    }
    if if boundary {
        match expected_call_result {
            None => !is_unit(program, target_state.return_type),
            Some(ref expected) => {
                !boundary_value_result_matches(program, target_state.return_type, expected, &[])
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
            Some(ExpectedCallValueResult::Structural(expected))
                if crate::execution::terminal_unit::types::borrowed_slice_view(
                    program,
                    target_state.return_type,
                ) || crate::execution::terminal_unit::types::borrowed_named_view(
                    program,
                    target_state.return_type,
                ) =>
            {
                // A `&[u8]`/`&'a V` result is a shared loan against the
                // callee's storage: the anonymous result carries its
                // declared identity with affine custody, exactly as the
                // reference-record arm above compares it — anonymous and
                // `let`-bound borrowed-view results share the one affine
                // mint, so only the identity spelling differs.
                {
                    let peeled =
                        crate::execution::terminal_unit::types::borrowed_view_result_identity(
                            program,
                            target_state.return_type,
                        );
                    // Anonymous and `let`-bound slice results mint the peeled
                    // referent identity; a `let`-bound named view keeps its
                    // `ref(...)` shell — the custody `add_named_view_type`
                    // registers — so both spellings name the same family.
                    let shelled = program.normalized_type_identity(target_state.return_type);
                    expected.multiplicity != Multiplicity::Affine
                        || (peeled.as_deref() != Some(expected.type_identity.as_str())
                            && shelled.as_str() != expected.type_identity)
                }
            }
            Some(expected @ ExpectedCallValueResult::Structural(_)) => {
                !boundary_value_result_matches(program, target_state.return_type, expected, &[])
            }
        }
    } {
        return None;
    }
    phase("call operation: target supply mode");
    if !boundary
        && !(target_machine.supply_mode == MachineSupplyMode::CheckedBody
            || (target_machine.supply_mode == MachineSupplyMode::Boundary
                && target_machine.body_is_present))
    {
        return None;
    }
    phase("call operation: structural arguments");
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
        trace,
    )?;
    // The callee's retained scalar positions: the same authored indices its
    // own signature plan keeps, so the erased position is absent on both
    // sides and `checked_call_scalar_arguments` pairs the caller's dense
    // argument ordinals with the retained parameters only.
    phase("call operation: scalar parameters");
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
    phase("call operation: scalar arguments");
    let scalar_arguments = if boundary {
        checked_call_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters, true)?
    } else {
        checked_call_scalar_arguments(facts, state.symbol, coordinate, &scalar_parameters, false)?
    };
    phase("call operation: erased scalar arguments");
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
    phase("call operation: erased proof arguments");
    let erased_proof_parameters =
        crate::execution::terminal_unit::types::erased_proof_parameter_plans(
            program,
            target_state,
        )?;
    let erased_proof_arguments = if boundary {
        if !erased_proof_parameters.is_empty() {
            return None;
        }
        Vec::new()
    } else {
        checked_call_erased_proof_arguments(
            program,
            facts,
            state.symbol,
            coordinate,
            &erased_proof_parameters,
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
        phase(if carries_routed_service {
            "call operation: routed service forward"
        } else {
            "call operation: projected operand support"
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
        phase("call operation: linear structural result");
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
            erased_proof_arguments,
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
    phase("call operation: claim transfers");
    let transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        caller_structural_results,
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
        phase("call operation: structural result loan");
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
        phase("call operation: structural result operands");
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
                    ))
                // A `&[u8]`/`&'a V` borrowed-view result loans the callee's
                // storage through the caller frame: its plan is affine like
                // the reference-record family above, and the result-shape arm
                // already proved the identity.
                || (result.multiplicity == Multiplicity::Affine
                    && (crate::execution::terminal_unit::types::borrowed_slice_view(
                        program,
                        target_state.return_type,
                    ) || crate::execution::terminal_unit::types::borrowed_named_view(
                        program,
                        target_state.return_type,
                    ))))
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
                erased_proof_arguments,
                structural_arguments,
                discard_result_on_return: result.multiplicity == Multiplicity::Affine
                    && !reference_loan.is_valid(),
            });
        }
        phase("call operation: claim-free affine result");
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
            erased_proof_arguments,
            structural_arguments,
            discard_result_on_return: true,
        })
    } else if expected_call_result.is_some()
        && (!structural_arguments.is_empty() || !transfers.is_empty())
        // A scalar result over structural operands is a `ScalarCall` whose
        // callee the candidate closure resolves after every body is built:
        // a registered producer, or the callee's own ordinary body when it
        // completes with a scalar (`scalar_targets::available_target`), and
        // the caller is dropped as `UnavailableScalarTarget` otherwise. A
        // pass with no scalar-callee catalog has no closure to defer to and
        // still admits only a settled provider adapter, whose settlement
        // already re-verified the exact signature and reach.
        && scalar_callees.is_none()
        && !facts
            .boundary_adapter_dispatch
            .iter()
            .any(|row| row.realization_state == target_state.symbol)
    {
        phase("call operation: scalar result producer");
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
            erased_proof_arguments,
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
