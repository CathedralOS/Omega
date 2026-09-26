//! A call whose value lands in a structural result binding. A returned
//! reference or borrowed view carries a loan whose operands must be the
//! caller's structural parameters or earlier results; any other result must
//! be a claim-free affine value. Either way the call becomes one
//! `StructuralCall` that carries the binding.

use crate::execution::terminal_unit::cleanup::service_reach_is_empty;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{base_type_identity, machine_binders};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, TypeReferenceNode, TypedTrees,
    is_reference,
};

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    caller_structural_results: &[(
        CheckedUnitStructuralResultBindingPlan,
        crate::fact_plan::PlaceRoot,
    )],
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
    let phase = |name: &'static str| {
        trace.phase(name);
        trace.statement(Some(coordinate.statement_index));
    };
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
                            && (crate::validation::is_closed_primitive_array_type(
                                program,
                                parameter.type_reference,
                            ) || crate::validation::has_plain_owned_contents_with_numeric_constraints(
                                program,
                                parameter.type_reference,
                            ) || crate::validation::reference_result_custody::is_reference_record(
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
                                    && crate::validation::reference_result_custody::is_reference_record(
                                        program,
                                        parameter.type_reference,
                                    )
                                    && argument.path.iter().all(|segment| {
                                        matches!(
                                            segment,
                                            crate::checked_trees::CheckedUnitStructuralPathSegment::Field(
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
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
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
            ) && crate::validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                target_state.return_type,
            ) && matches!(
                program
                    .type_reference_table
                    .type_reference(target_state.return_type),
                TypeReferenceNode::Named { .. }
            ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && crate::validation::is_closed_primitive_array_type(
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
            custody: crate::checked_trees::CheckedStructuralCallCustodyPlan {
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
}
