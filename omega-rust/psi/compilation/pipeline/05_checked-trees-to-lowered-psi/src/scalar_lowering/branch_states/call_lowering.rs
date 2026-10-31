//! Lower a direct scalar call binding against its callee's Unit body.

use crate::checked_facts::value_facts;
use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::buffer::SourceInvocation;
use crate::emission::operation_emission::calls::{
    LoweredDirectCallBinding, PreparedStructuralArgument,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::scalar_lowering::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, QualifiedScalarType, scalar_carriers,
    unsupported,
};
use checked_trees::CheckedErasedProofParameterPlan;

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_scalar_call(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    caller_machine: symbols::SymbolHandle,
    coordinate: SourceInvocation,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    result_type: QualifiedScalarType,
    caller_value_types: &[QualifiedScalarType],
    scalar_bindings: &storage::ScalarBindings,
    arguments: Vec<LoweredDirectExpression>,
    structural_arguments: Vec<PreparedStructuralArgument>,
    claim_transfers: Vec<checked_trees::CheckedUnitClaimTransferPlan>,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
    loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let caller_state = coordinate.state();
    let statement_site = coordinate
        .statement()
        .map(|site| {
            Ok::<_, LoweringError>((
                u32::try_from(site.statement_index).map_err(|_| {
                    LoweringError::Unsupported("scalar call statement ordinal exceeds u32")
                })?,
                u32::try_from(site.call_ordinal)
                    .map_err(|_| LoweringError::Unsupported("scalar call ordinal exceeds u32"))?,
            ))
        })
        .transpose()?;
    let target = if structural_arguments.is_empty() {
        crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find(
            checked,
            target_machine,
        )?
    } else {
        crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked,
            target_machine,
        )?
    };
    // Field selectors must already have been captured in the authored operand
    // stream. Emission only resolves those carried values; it never reads fields.
    // Structural operands arrive from source custody and catalog projection
    // preparation. Reclassifying their path spelling here would discard valid
    // reference crossings after their geometry and access were already checked.
    // The checked transfers retain each active owned path and its caller
    // identity. A callee's entry roster also declares inactive Case alternatives,
    // so its total claim count is not a call's transfer requirement. Ordinary
    // calls and scalar-producing calls use the same guarded path preparation
    // below; emission resolves those identities in the caller's live namespace.
    if target.structural_parameters().len() != structural_arguments.len()
        || claim_transfers
            .iter()
            .any(|transfer| transfer.argument_index as usize >= structural_arguments.len())
    {
        return unsupported("computed scalar call requires exact structural custody");
    }
    let (target_parameter_types, target_result_type) =
        qualifications.scalar_state_types(checked, target_state)?;
    if target.entry_state()? != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if target_result_type != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if arguments.len() != target_parameter_types.len() {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    for (expression, target_type) in arguments.iter().zip(&target_parameter_types) {
        if expression.value_type(caller_value_types)? != *target_type {
            return unsupported(
                "checked scalar call argument type must match its callee parameter",
            );
        }
        validate_direct_parameter_types(expression, &scalar_carriers(caller_value_types))?;
    }
    // Erased actuals sit on the proof-only lane the checker bound under
    // `ErasedUnitCallArgument`, in the target's erased-formal order.
    let erased_arguments = (0..target.erased_parameters().len())
        .map(|erased_ordinal| {
            let (statement_ordinal, call_ordinal) = statement_site.ok_or(
                LoweringError::Unsupported("declaration call has no erased value argument custody"),
            )?;
            scalar_bindings.expression_at(
                identities,
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::ErasedUnitCallArgument {
                    call_ordinal,
                    erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "scalar call erased argument ordinal exceeds u32",
                        )
                    })?,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Proof-only actuals sit on the adjacent lane under the same
    // `ErasedUnitCallArgument` role, in the target's erased-proof roster
    // order; each converts through the caller machine's roster so forwarded
    // `Formal` positions stay dense.
    let erased_proof_arguments = (0..target.erased_proof_parameters().len())
        .map(|erased_ordinal| {
            let (statement_ordinal, call_ordinal) = statement_site.ok_or(
                LoweringError::Unsupported("declaration call has no erased proof argument custody"),
            )?;
            let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                    LoweringError::Unsupported("scalar call erased proof ordinal exceeds u32")
                })?,
            };
            let term = value_facts(checked)
                .proof_terms
                .term_at(caller_state, statement_ordinal, role)
                .ok_or(LoweringError::Unsupported(
                    "scalar call erased proof actual is absent",
                ))?;
            crate::scalar_lowering::contracts::checked_proof_term(
                identities,
                checked,
                term,
                caller_erased_proof_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let checked_identity = if let Some((statement_ordinal, call_ordinal)) = statement_site {
        let checked_call = checked
            .facts
            .contract_plans
            .for_machine(caller_machine)
            .and_then(|plan| {
                plan.crash
                    .checked_call_at(caller_state, statement_ordinal, call_ordinal)
            })
            .ok_or(LoweringError::Unsupported(
                "direct scalar call has no matching checked crash-refinement row",
            ))?;
        if checked_call.target_machine() != target_machine
            || checked_call.target_state() != target_state
        {
            return unsupported("checked scalar call target disagrees with crash refinement");
        }
        checked_trees::MachineContractIdentity {
            report_fingerprint: checked_call.target_contract_report_fingerprint(),
            commitment: checked_call.target_contract_commitment(),
        }
    } else {
        let SourceInvocation::Declaration {
            application, call, ..
        } = coordinate
        else {
            return unsupported("scalar invocation has no admitted source origin");
        };
        let (_, call) = checked_trees::CheckedCallOrigin::Declaration { application, call }
            .declaration(
                &checked.facts.termination.rank_arrivals,
                caller_machine,
                caller_state,
            )
            .map_err(LoweringError::Unsupported)?;
        if call.target_machine != target_machine || call.target_state != target_state {
            return unsupported("scalar declaration invocation changed its selected callee");
        }
        call.target_contract.ok_or(LoweringError::Unsupported(
            "scalar declaration invocation has no bound target contract",
        ))?
    };
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_identity.report_fingerprint != target_contract.report_fingerprint
        || checked_identity.commitment != target_contract.commitment
    {
        return unsupported("checked scalar call target contract identity disagrees");
    }
    // Computed arguments have already become values. Bind the pinned callee
    // routes to those values, as ordinary staged calls do, rather than trying
    // to turn their effectful source expressions into pure caller predicates.
    // The pinned ceiling is the callee's effective crash routes — authored
    // buckets for a published ceiling or the union of retained body evidence
    // for an inferred contract — in the callee's parameter namespace, exactly
    // the ceiling the emitted machine contract publishes and the verifier
    // substitutes back at this call.
    let target_routes = crate::unit::effective_crash_routes(checked, target_machine)?;
    if statement_site.is_none() && !target_routes.is_empty() {
        return unsupported("declaration invocation lost its checked crash-free value closure");
    }
    let crash_continuations = target_routes.clone();
    let claim_transfers = crate::emission::claim_bindings::prepare_checked_claim_transfers(
        identities,
        checked,
        &claim_transfers,
        target.structural_parameters(),
        structural_types,
    )?;
    // This summary only tells graph preparation whether the checked call can
    // crash; an identity-only predicate suffices for that question. Emission
    // lowers `target_routes` against evaluated arguments, not this refined
    // summary. Its structured-term checks and independent callee substitution
    // still reject an unsupported or mismatched executable continuation.
    Ok(LoweredDirectCallBinding {
        caller_types: scalar_bindings.published_structural_types()?,
        loan_completions,
        source_coordinate: coordinate,
        target_machine,
        result_type,
        arguments,
        erased_arguments,
        erased_proof_arguments,
        structural_arguments,
        claim_transfers,
        // The selected body owns storage even when its public signature has
        // only scalars. Graph and ordered-body callers use the same decision.
        crash_continuations,
        parameter_relative_crash_routes: target_routes,
    })
}
