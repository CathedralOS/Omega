//! A call whose value lands in a structural result binding. A returned
//! reference or borrowed view carries a loan whose operands must be the
//! caller's structural parameters or earlier results; any other result must
//! be a claim-free owned value. Either way the call becomes one ordinary
//! call with a structural result that carries the binding, and its custody
//! is decided once here and recorded in the plan (`with_structural_custody`).

use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};

use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{base_type_identity, machine_binders};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, SymbolHandle, TypeReferenceNode,
    TypedTrees, is_reference,
};

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    trace: &LocalConstructionTrace,
    planned: super::call_operations::PlannedCall<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let super::call_operations::PlannedCall {
        coordinate,
        source_site,
        call,
        target_machine,
        target_state,
        target_contract,
        scalar_parameters,
        scalar_arguments,
        erased_scalar_arguments,
        erased_proof_arguments,
        structural_arguments,
        transfers,
    } = planned;
    let call_target = CheckedUnitCallTarget {
        machine: target_machine.symbol,
        state: target_state.symbol,
        contract_report_fingerprint: target_contract.report_fingerprint,
        service_reach: call.service_reach,
    };
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
    phase("call operation: structural result loan");
    let forwarded_reference_sources =
        validation::reference_result_custody::terminal_call_reference_sources(
            program, state, call, result,
        )
        .unwrap_or_default();
    let reference_loan = if !forwarded_reference_sources.is_empty() {
        arena::Handle::invalid()
    } else if crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
        program,
        target_state.return_type,
    )
    .is_some()
        || crate::execution::terminal_unit::reference_results::shared_borrowed_parts(
            program,
            target_state.return_type,
        )
        .is_some()
    {
        // Both accesses retain the exact call's loan, activation and release.
        // Origin reconstruction belongs to the shared reference-result owner;
        // a private view walker must not miss ordinary forwarded completions.
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
    // A result whose contents hold no reference or slice cannot retain a loan
    // of its operands: whatever the callee read or wrote through a borrowed
    // argument stays in the lender's storage. A nominal cleanup obligation
    // travels with the owned result without retaining an operand loan.
    let represented_copy_result = result.multiplicity == Multiplicity::Unrestricted
        && validation::has_contents(
            program,
            target_state.return_type,
            validation::ContentsRequirement::RepresentedOwned,
        );
    let result_retains_no_loan = !reference_loan.is_valid()
        && (represented_copy_result
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
                program,
                target_state.return_type,
                validation::ContentsRequirement::CleanupOwned,
            ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(program, target_state.return_type)));
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
                            && !(false && is_reference(program, parameter.type_reference))
                    })
                    .nth(argument_index)
                    .is_some_and(|parameter| {
                        // Typed Self names the machine, not its data carrier.
                        // Use the same checked signature that declares the
                        // callee's receiver; this also covers non-generic
                        // attachments without a retained owner application.

                        let carrier = parameter.type_reference;
                        (validation::is_closed_primitive_array_type(
                                program,
                                carrier,
                            ) || validation::has_contents(
                                program,
                                carrier, validation::ContentsRequirement::CleanupOwned) || validation::reference_result_custody::is_reference_record(
                                program,
                                carrier,
                            )
                            // An owned actual moved into a by-value linear
                            // formal transfers its claims into the callee;
                            // the call's claim transfers carry them, so the
                            // carrier needs no claim-free contents proof.
                            || crate::checks::type_multiplicity(program, carrier)
                                == Multiplicity::Linear)
                            && base_type_identity(program, carrier, &[])
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
                                        carrier,
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
                // The argument planner already rejoined this loan
                // to its exact earlier result and captured call access.
                // Borrowing that live storage does not transfer its owner.
                || (matches!(
                    argument.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                )
                    && argument
                        .source_structural_result_binding_ordinal()
                        .is_some())
                // An exclusive loan of an earlier result or local record,
                // whole or by fixed field and index path, was rejoined the
                // same way to its captured `&mut` access. Only a result that
                // retains no loan admits it, so the loan ends with the call
                // and the lender keeps its owner.
                || (argument.access == CheckedStructuralAccess::MutableBorrow
                    && result_retains_no_loan
                    && argument
                        .source_structural_result_binding_ordinal()
                        .is_some()
                    && argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                        )
                    }))
                || argument.byte_sequence_literal().is_some()
                // A range descriptor over an established caller view was
                // admitted by the same storage judgment every other call
                // form uses; its root names adjudicated caller storage.
                || super::view_subslice::is_view_subslice(argument)
                || matches!(
                    argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                ))
                && matches!(
                    argument.access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                )
        });
    // An ordinary call keeps one route and result binding whatever claims it
    // consumes. Consumed linear claim-carrying inputs become the custody's
    // input transfer roster, and the custody judgment fences that result side
    // itself; a call that consumes none must also carry admitted result
    // contents.
    if args_ok
        && (!transfers.is_empty()
            || represented_copy_result
            || reference_loan.is_valid()
            || !forwarded_reference_sources.is_empty()
            || crate::execution::terminal_unit::reference_results::is_reference_record(
                program,
                target_state.return_type,
            )
            // Closed contents and the retained body determine the carrier,
            // including lifetime-only applications. Named syntax is not an
            // extra ownership proof; the classifier still rejects open types,
            // carried loans and unknown qualifications. Nominal cleanup remains
            // attached to the transported owner and is checked at its death.
            //
            // The judgment is the one the result binding answered to: the
            // contents a structural literal establishes. It carries the
            // scalar members whose declared plain domain is represented as
            // well as the nominal-cleanup owners — the callee's completion
            // proved each member's obligation and this route only transports
            // the storage, so a transported record is admitted exactly where
            // a constructed one is. A top-level qualification on the result
            // still refuses: the binding classifier rejects one before this
            // route can drop it.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
                program,
                target_state.return_type, validation::ContentsRequirement::StructuralConstruction))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(
                    program,
                    target_state.return_type,
                ))
            // A sum or record carrying shared-view members — the same
            // descriptor loans the bare-view arm below names, nested inside
            // owned contents. The classifier admits no loan that mutates, no
            // erased or generic member, and no cleanup owner, so the result
            // keeps the same claim-free custody the plain-contents arm gives.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
                program,
                target_state.return_type, validation::ContentsRequirement::OwnedOrSharedView))
            // Shared descriptors and materialized reference carriers borrow
            // storage but retain their distinct result-custody obligations.
            || (result.multiplicity
                == validation::reference_result_custody::result_multiplicity(
                    program,
                    target_state.return_type,
                )
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
        return with_structural_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            structural_custody_parts(
                coordinate,
                source_site,
                call_target,
                target_contract.commitment,
                erased_scalar_arguments,
                erased_proof_arguments,
                scalar_arguments,
                structural_arguments,
                result.clone(),
                result.multiplicity == Multiplicity::Affine && !reference_loan.is_valid(),
            ),
        );
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
    with_structural_custody(
        program,
        facts,
        machine.symbol,
        state.symbol,
        CheckedUnitCallParts {
            coordinate,
            source_site,
            target: call_target,
            route: CheckedUnitCallRoute::Module {
                contract_commitment: Some(target_contract.commitment),
                erased_scalar_arguments,
                erased_proof_arguments,
            },
            scalar_arguments,
            structural_arguments,
            result: CheckedUnitCallResult::Structural {
                binding: result.clone(),
                discard_result_on_return: true,
            },
            transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
        },
    )
}

/// Admit one ordinary structural call and record its custody: the checker's
/// single judgment over the exact call and ownership facts. Lowering reads
/// the recorded custody; a call the judgment refuses is not planned.

/// The Module-route structural-custody call parts both admissible sites build.
pub(super) fn structural_custody_parts(
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    source_site: Option<checked_trees::NominalMachineUseSite>,
    target: checked_trees::CheckedUnitCallTarget,
    commitment: checked_trees::MachineContractCommitment,
    erased_scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    erased_proof_arguments: Vec<checked_trees::CheckedProofTerm>,
    scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    structural_arguments: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
    binding: checked_trees::CheckedUnitStructuralResultBindingPlan,
    discard_result_on_return: bool,
) -> checked_trees::CheckedUnitCallParts {
    checked_trees::CheckedUnitCallParts {
        coordinate,
        source_site,
        target,
        route: CheckedUnitCallRoute::Module {
            contract_commitment: Some(commitment),
            erased_scalar_arguments,
            erased_proof_arguments,
        },
        scalar_arguments,
        structural_arguments,
        result: CheckedUnitCallResult::Structural {
            binding,
            discard_result_on_return,
        },
        transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
    }
}

pub(super) fn with_structural_custody(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    parts: CheckedUnitCallParts,
) -> Option<CheckedUnitEffectOperationPlan> {
    let mut operation = CheckedUnitEffectOperationPlan::Call(CheckedUnitCallPlan::new(parts)?);
    let custody = validation::structural_call_custody(
        program,
        facts,
        caller_machine,
        caller_state,
        &operation,
    )
    .ok()?;
    *operation.call_mut()?.structural_custody_mut()? = custody;
    Some(operation)
}

/// A parameter names caller storage only while it is mutably borrowed:
/// `bag: &mut Bag` spells a `Reference` type, while a binding declared on the
/// bare record type carries that access in the parameter's own `is_mutable`.
pub(crate) fn view_source_parameter_is_mutable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    parameter.is_mutable
        || crate::execution::terminal_unit::reference_results::mutable_borrowed_parts(
            program,
            parameter.type_reference,
        )
        .is_some()
}

/// The field symbol `root` names inside this state's data binding, when that
/// binding is mutably borrowed — the root a minted view loan publishes for
/// `main.<field>` storage. `None` for roots that are not members of the
/// binding's record and whenever the binding is not mutably borrowed.
///
/// The binding is the parameter declaring the machine's attached data, which
/// `data_binding_parameter` names. The retired `self` flag no longer marks
/// it, so asking for that flag refused every state unconditionally.
pub(crate) fn view_data_member_symbol(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine: SymbolHandle,
    root: SymbolHandle,
) -> Option<SymbolHandle> {
    let owner = program.machine_by_symbol(machine)?;
    let data_binding = program.data_binding_parameter(owner, program.state_parameters(state))?;
    if !view_source_parameter_is_mutable(program, data_binding) {
        return None;
    }
    let mut type_reference = data_binding.type_reference;
    let mut symbol = loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            _ => return None,
        }
    };
    // A `&mut Self` referee names the machine's `Self` alias rather than the
    // data declaration; resolve it through the machine's attached data owner.
    if symbol == machine {
        symbol = owner.attached_data_symbol;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.symbol == root => {
                Some(field.symbol)
            }
            _ => None,
        })
}
