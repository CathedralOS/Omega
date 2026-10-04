//! A call whose value lands in a structural result binding. A returned
//! reference or borrowed view carries a loan whose operands must be the
//! caller's structural parameters or earlier results; any other result must
//! be a claim-free owned value. Either way the call becomes one ordinary
//! call with a structural result that carries the binding.

use checked_trees::{
    CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
    CheckedUnitCallTarget, CheckedUnitCallTransfers,
};

use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity, machine_binders};
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
        && validation::has_plain_owned_contents_with_represented_constraints(
            program,
            target_state.return_type,
        );
    let result_retains_no_loan = !reference_loan.is_valid()
        && (represented_copy_result
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_cleanup_owned_contents(program, target_state.return_type))
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
                            && !(parameter.is_self
                                && is_reference(program, parameter.type_reference))
                    })
                    .nth(argument_index)
                    .is_some_and(|parameter| {
                        // Typed Self names the machine, not its data carrier.
                        // Use the same checked signature that declares the
                        // callee's receiver; this also covers non-generic
                        // attachments without a retained owner application.
                        if parameter.is_self {
                            let Some((_, parameters, _)) = super::structural_scalar_signature(
                                program,
                                &mut ShapeCollector::new(program),
                                target_machine,
                                target_state,
                                &[],
                                false,
                            ) else {
                                return false;
                            };
                            return parameters.iter().find(|formal| formal.is_self).is_some_and(
                                |formal| {
                                    formal.access == CheckedStructuralAccess::Owned
                                        && matches!(
                                            formal.multiplicity,
                                            Multiplicity::Affine | Multiplicity::Unrestricted
                                        )
                                        && formal.type_identity == argument.type_identity
                                        && formal.qualifications.is_empty()
                                        && formal.projected_qualifications.is_empty()
                                        && formal.fused_service_erasure.is_none()
                                        && argument.path.is_empty()
                                },
                            );
                        }
                        let carrier = parameter.type_reference;
                        (validation::is_closed_primitive_array_type(
                                program,
                                carrier,
                            ) || validation::has_cleanup_owned_contents(
                                program,
                                carrier,
                            ) || validation::reference_result_custody::is_reference_record(
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
    // An ordinary call that consumes linear claim-carrying inputs keeps the
    // same route and result binding: the consumed set is re-derived as the
    // custody's input transfer roster, and a claim-free result carries no
    // returned frontier. The reconstruction fences the result side itself,
    // so only the empty-transfer rosters keep the inline emit below.
    if args_ok
        && !transfers.is_empty()
        && program
            .machine_states(target_machine)
            .first()
            .is_some_and(|entry| entry.symbol == target_state.symbol)
        && machine_binders(program, target_machine).is_empty()
    {
        let mut operation =
            CheckedUnitEffectOperationPlan::Call(CheckedUnitCallPlan::new(CheckedUnitCallParts {
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
                    discard_result_on_return: result.multiplicity == Multiplicity::Affine
                        && !reference_loan.is_valid(),
                },
                transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
            })?);
        let custody = validation::reconstruct_structural_call_custody(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &operation,
        )
        .ok()?;
        *operation.call_mut()?.structural_custody_mut()? = custody;
        return Some(operation);
    }
    if args_ok
        && transfers.is_empty()
        && (represented_copy_result || reference_loan.is_valid() || !forwarded_reference_sources.is_empty()
            || crate::execution::terminal_unit::reference_results::is_reference_record(
                program,
                target_state.return_type,
            )
            // Closed contents and the retained body determine the carrier,
            // including lifetime-only applications. Named syntax is not an
            // extra ownership proof; the classifier still rejects open types,
            // carried loans and unknown qualifications. Nominal cleanup remains
            // attached to the transported owner and is checked at its death.
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_cleanup_owned_contents(
                program,
                target_state.return_type,
            ))
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
            ) && validation::has_owned_or_shared_view_contents(
                program,
                target_state.return_type,
            ))
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
        return CheckedUnitCallPlan::new(CheckedUnitCallParts {
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
                discard_result_on_return: result.multiplicity == Multiplicity::Affine
                    && !reference_loan.is_valid(),
            },
            transfers: CheckedUnitCallTransfers::StructuralCustody(
                checked_trees::CheckedStructuralCallCustodyPlan {
                    reference_loan,
                    forwarded_reference_sources,
                    result_qualifications: if validation::shared_slice_view_result(
                        program,
                        target_state.return_type,
                    ) {
                        // A shared view's declared byte-predicate domains
                        // ride the same carrier identity its result spells.
                        validation::view_result_carrier_qualifications(
                            program,
                            target_state.return_type,
                        )
                        .ok()?
                    } else if represented_copy_result {
                        validation::structural_result_qualifications(
                            program,
                            target_state.return_type,
                        )
                        .ok()?
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                },
            ),
        })
        .map(CheckedUnitEffectOperationPlan::Call);
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
    CheckedUnitCallPlan::new(CheckedUnitCallParts {
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
    })
    .map(CheckedUnitEffectOperationPlan::Call)
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
