//! Building one checked call operation and the value result it is expected
//! to produce.

use crate::execution::terminal_unit::ScalarCalleeClosure;

use crate::execution::terminal_unit::calls::argument_paths::{
    checked_call_erased_proof_arguments, checked_call_erased_scalar_arguments,
    checked_call_scalar_arguments, projected_call_contracts_are_represented,
};
use crate::execution::terminal_unit::calls::boundary_admission::boundary_value_result_matches;
use crate::execution::terminal_unit::calls::service_forward;
use crate::execution::terminal_unit::calls::structural_arguments::{
    call_claim_transfers, exact_integer_at, structural_call_arguments,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{is_unit, machine_binders};
use crate::execution::terminal_unit::{
    BuiltinFunction, CheckFacts, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    PrimitiveType, SymbolHandle, strips_erased_parameter,
};
use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};
use validation::exact_external_leaf_boundary_requirement;

pub(in crate::execution) enum ExpectedCallValueResult<'result> {
    Scalar(PrimitiveType),
    Structural(&'result CheckedUnitStructuralResultBindingPlan),
}

/// A resolved call through an ordinary or bodyless-boundary target: the
/// target, its contract and every argument lane, ready for the plan the
/// call's expected result selects.
pub(super) struct PlannedCall<'p> {
    pub(super) coordinate: CheckedUnitCallCoordinate,
    pub(super) source_site: Option<checked_trees::NominalMachineUseSite>,
    pub(super) call: &'p checked_trees::FlowCallFact,
    pub(super) target_machine: &'p typed_trees::machine::Machine,
    pub(super) target_state: &'p typed_trees::state::State,
    pub(super) target_contract: &'p checked_trees::MachineContractPlan,
    pub(super) scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    pub(super) erased_scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    pub(super) erased_proof_arguments: Vec<checked_trees::CheckedProofTerm>,
    pub(super) structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
    pub(super) transfers: Vec<checked_trees::CheckedUnitClaimTransferPlan>,
}

/// Build one call with the scalar-callee evidence available to this pass.
/// `None` supplies no registered scalar targets; it never consults published catalogs.
pub(in crate::execution) fn build_call_operation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: Option<ScalarCalleeClosure>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &checked_trees::FlowCallFact,
    expected_call_result: Option<ExpectedCallValueResult<'_>>,
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    caller_operations: &[CheckedUnitEffectOperationPlan],
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
    let call_site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let source_site = match &call_site {
        validation::CallSite::Statement(_) => {
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
        validation::CallSite::Expression { expression, .. } => Some(
            checked_trees::NominalMachineUseSite::Expression(*expression),
        ),
        validation::CallSite::TransitionNamed { .. } => None,
    };

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = validation::call_site_argument_expressions(program, &call_site);
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
        exact_external_leaf_boundary_requirement(program, call.target_symbol);
    let requirement = match boundary_requirement(program, call.target_symbol) {
        BoundaryTarget::Requirement(requirement) => Some(requirement),
        BoundaryTarget::Body => None,
        BoundaryTarget::Ambiguous => return None,
    };
    if let Some(requirement) = requirement {
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
            caller_operations,
            trace,
            coordinate,
            &call_site,
            source_site,
            requirement,
            selected_realization,
        );
    }

    if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
        return super::dynamic_call::build(
            program,
            facts,
            scalar_callees,
            machine,
            state,
            caller_parameters,
            caller_trivial_affine_locals,
            entry_claims,
            call,
            expected_call_result.as_ref(),
            caller_structural_results,
            caller_operations,
            trace,
            coordinate,
            &call_site,
            slot,
        );
    }
    phase("call operation: target state");
    // Resolve the executable pair without rewriting the flow occurrence,
    // which still authenticates the authored call and operands.
    let (target_machine, target_state) = executable_target(program, call.target_symbol)?;
    phase("call operation: target contract");
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    let call_target = CheckedUnitCallTarget {
        machine: target_machine.symbol,
        state: target_state.symbol,
        contract_report_fingerprint: target_contract.report_fingerprint,
        service_reach: call.service_reach,
    };
    // A bodied `boundary machine` is a checked adapter: callers reach its
    // authored body as an ordinary callee; only the bodyless declaration is
    // a boundary edge.
    let boundary =
        target_machine.supply_mode.is_boundary_declaration() && !target_machine.body_is_present;
    // Boundary and ordinary results retain the same structural carrier.
    // Installation supplies the checked provider body and its exact return
    // custody; a boundary declaration does not authorize fabricated payloads.
    phase("call operation: result shape");
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
                if crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
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
                if let Some(referee) =
                    crate::execution::terminal_unit::types::borrowed_view_referent(
                        program,
                        target_state.return_type,
                        language_semantics::ReferenceAccess::Mutable,
                    ) =>
            {
                // A `&mut R`/`&mut [T]` result is the exclusive loan the
                // shared-view arm above names for shared borrows: affine
                // custody against the callee's storage. Like the named
                // views, the bound side keeps its `ref-mut` shell — the
                // `Reference` shape is the custody it loans; the peeled
                // referent spelling stays accepted for anonymous results.
                let peeled = program
                    .type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
                        referee,
                    ))
                    .into_string();
                let shelled = program.normalized_type_identity(target_state.return_type);
                expected.multiplicity != Multiplicity::Affine
                    || (peeled.as_str() != expected.type_identity
                        && shelled.as_str() != expected.type_identity)
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
                // A shared slice result transports a copyable descriptor;
                // a named reference transports affine permission custody.
                // Neither carrier owns or extends the lifetime of its backing.
                let multiplicity = if crate::execution::terminal_unit::types::borrowed_slice_view(
                    program,
                    target_state.return_type,
                ) {
                    Multiplicity::Unrestricted
                } else {
                    Multiplicity::Affine
                };
                let peeled = crate::execution::terminal_unit::types::borrowed_view_result_identity(
                    program,
                    target_state.return_type,
                );
                let shelled = program.normalized_type_identity(target_state.return_type);
                expected.multiplicity != multiplicity
                    || (peeled.as_deref() != Some(expected.type_identity.as_str())
                        && shelled.as_str() != expected.type_identity)
            }
            Some(expected @ ExpectedCallValueResult::Structural(_)) => {
                !boundary_value_result_matches(program, target_state.return_type, expected, &[])
            }
        }
    } {
        return None;
    }
    phase("call operation: target supply mode");
    if !boundary && !target_machine.supplies_checked_body() {
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
        super::CallTarget::declared(program, target_machine, target_state),
        &call_site,
        call.statement_index,
        caller_structural_results,
        caller_operations,
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
        if (parameter.is_const)
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
                    typed_trees::service::classify_exact_service_parameter_carrier(
                        program,
                        parameter.type_reference,
                    )
                    .ok()
                    .flatten()
                    .is_some()
                });
        phase(if carries_routed_service {
            "call operation: routed service forward"
        } else {
            "call operation: projected operand support"
        });
        let supported = (!carries_routed_service
            || service_forward::fused_service_arguments_match(
                program,
                state,
                caller_parameters,
                target_state,
                call.statement_index,
                &structural_arguments,
            ))
            && projected_call_contracts_are_represented(
                program,
                facts,
                machine,
                state,
                target_machine,
                target_state,
                &structural_arguments,
            );
        if !supported {
            return None;
        }
    }
    // A result carrying linear claims, whole or beneath an affine owner,
    // binds them through its checked custody rather than an ownership-free
    // storage judgment: the callee's returning edge decides which claim
    // continues at each path.
    if !boundary
        && let Some(ExpectedCallValueResult::Structural(result)) = &expected_call_result
        && !validation::linear_claim_frontier(program, target_state.return_type).is_empty()
    {
        phase("call operation: claim-bearing structural result");
        if !machine_binders(program, target_machine).is_empty()
            || !program
                .machine_states(target_machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_state.symbol)
        {
            return None;
        }
        return super::structural_result_call::with_structural_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            super::structural_result_call::structural_custody_parts(
                coordinate,
                source_site,
                call_target,
                target_contract.commitment,
                erased_scalar_arguments,
                erased_proof_arguments,
                scalar_arguments,
                structural_arguments,
                (*result).clone(),
                // An affine owner's remaining storage is discarded at return
                // once its linear members have moved out; a linear root has
                // no such remainder.
                result.multiplicity == Multiplicity::Affine,
            ),
        );
    }
    phase("call operation: claim transfers");
    let completion = if boundary {
        validation::boundary_requirement_completion(program, call_target.machine)
    } else {
        semantic_vocabulary::BoundaryCompletion::NormalReturn
    };
    let transfers = if matches!(
        completion,
        semantic_vocabulary::BoundaryCompletion::External(_)
    ) {
        Vec::new()
    } else {
        call_claim_transfers(
            program,
            facts,
            machine.symbol,
            state.symbol,
            call,
            caller_parameters,
            entry_claims,
            caller_structural_results,
            &structural_arguments,
        )?
    };

    if boundary {
        CheckedUnitCallPlan::new(CheckedUnitCallParts {
            coordinate,
            source_site,
            target: call_target,
            route: CheckedUnitCallRoute::Boundary,
            completion,
            scalar_arguments,
            structural_arguments,
            result: CheckedUnitCallResult::Unit,
            transfers: match completion {
                semantic_vocabulary::BoundaryCompletion::NormalReturn => {
                    CheckedUnitCallTransfers::CompletionReceipts(transfers)
                }
                semantic_vocabulary::BoundaryCompletion::External(_) => {
                    CheckedUnitCallTransfers::Abandonment
                }
            },
        })
        .map(CheckedUnitEffectOperationPlan::Call)
    } else if let Some(ExpectedCallValueResult::Structural(result)) = expected_call_result {
        super::structural_result_call::build(
            program,
            facts,
            machine,
            state,
            trace,
            PlannedCall {
                coordinate,
                source_site,
                call,
                target_machine,
                target_state,
                target_contract,
                scalar_arguments,
                erased_scalar_arguments,
                erased_proof_arguments,
                structural_arguments,
                transfers,
            },
            result,
        )
    } else if expected_call_result.is_some()
        && (!structural_arguments.is_empty() || !transfers.is_empty())
        // A scalar result over structural operands is an ordinary scalar call whose
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
        // An ordinary Unit call retains no authored site.
        CheckedUnitCallPlan::new(CheckedUnitCallParts {
            coordinate,
            source_site: None,
            target: call_target,
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            route: CheckedUnitCallRoute::Module {
                contract_commitment: None,
                erased_scalar_arguments,
                erased_proof_arguments,
            },
            scalar_arguments,
            structural_arguments,
            result: CheckedUnitCallResult::Unit,
            transfers: CheckedUnitCallTransfers::Claims(transfers),
        })
        .map(CheckedUnitEffectOperationPlan::Call)
    }
}

/// What a call's authored target completes through.
pub(super) enum BoundaryTarget<'program> {
    /// A boundary requirement's call route.
    Requirement(super::trait_boundary_call::BoundaryRequirement<'program>),
    /// More than one boundary requirement declares the target.
    Ambiguous,
    /// No boundary requirement: a machine body or a dynamic slot.
    Body,
}

/// The boundary requirement a call to `target` invokes. A direct call to a
/// bodyless external leaf is a boundary call on the leaf's exact satisfied
/// requirement (see `validation`'s `intrinsic_boundaries` module); the leaf
/// owns no body to call. A nominal binder describes an obligation, not a
/// selected boundary body: specialization must replace it with the exact
/// executable target before a call is planned, and projecting its
/// requirement here would make an open generic entry executable and bypass
/// its selected implementation. Both declaration spellings retain a
/// requirement edge; the planner sees the authored signature, never its
/// selected provider.
pub(super) fn boundary_requirement(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> BoundaryTarget<'_> {
    let direct = exact_external_leaf_boundary_requirement(program, target)
        .map(|(requirement, _)| requirement)
        .unwrap_or(target);
    let static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == direct)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    match static_boundaries.as_slice() {
        [(definition, signature)] => {
            BoundaryTarget::Requirement(super::trait_boundary_call::BoundaryRequirement::Trait {
                definition,
                signature,
            })
        }
        [] => program
            .operators()
            .iter()
            .find(|operator| operator.is_boundary && operator.symbol == direct)
            .map_or(BoundaryTarget::Body, |operator| {
                BoundaryTarget::Requirement(
                    super::trait_boundary_call::BoundaryRequirement::Operator(operator),
                )
            }),
        _ => BoundaryTarget::Ambiguous,
    }
}

/// The machine and state a call to `target` executes. Source custody can name
/// a machine head or an exact stored state: a machine invokes its entry, and
/// an explicitly selected state keeps its own signature.
fn executable_target(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    validation::find_machine_head(program, target)
        .or_else(|| validation::find_state_with_machine(program, target))
}

/// Whether a call to `target` produces its result through a machine body:
/// the executable target `build_call_operation` resolves supplies a checked
/// body. A boundary requirement, a bodyless `boundary machine` declaration
/// and a dynamic slot each complete through their own call route, which
/// binds the result to the authored statement instead.
pub(crate) fn completes_in_machine_body(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> bool {
    matches!(boundary_requirement(program, target), BoundaryTarget::Body)
        && program.dynamic_slot_by_symbol(target).is_none()
        && executable_target(program, target)
            .is_some_and(|(machine, _)| machine.supplies_checked_body())
}
