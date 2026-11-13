//! Calls use the evaluated machine value while contracts stay independent of code.
//!
//! Argument evaluation, claim transfer, requirements and crash continuation
//! preparation share the ordinary invocation owner. Only the executable target
//! differs: it is the saved callable operand, with no implicit receiver.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::expression_preparation::bindings::{PublishedStructuralTypes, ScalarBindings};
use crate::proofs::crash_routes::lowered_erased_scalar_term;
use crate::scalar_lowering::contracts::{LoweredProofTerm, checked_proof_term};
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::terminal_identities::{mint_with, value_id};
use checked_trees::{CheckedErasedProofParameterPlan, CheckedScalarExpressionRole, CheckedTrees};
use semantic_vocabulary::{QualifiedScalarType, ScalarType, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, TerminalCallableContract, ValueDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredRuntimeCallBinding {
    pub source_coordinate: SourceCallCoordinate,
    pub binding: typed_trees::callable_values::RuntimeCallBinding,
    pub caller_types: PublishedStructuralTypes,
    pub loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
    pub result_type: QualifiedScalarType,
    pub callee: LoweredDirectExpression,
    pub arguments: Vec<LoweredDirectExpression>,
    pub erased_arguments: Vec<LoweredDirectExpression>,
    pub erased_proof_arguments: Vec<LoweredProofTerm>,
    pub structural_arguments: Vec<PreparedStructuralArgument>,
    pub claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
    pub crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_scalar_call(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    catalog: &terminal_psi::TerminalCallableCatalog,
    coordinate: SourceCallCoordinate,
    callee: LoweredDirectExpression,
    caller_value_types: &[QualifiedScalarType],
    scalar_bindings: &ScalarBindings,
    arguments: Vec<LoweredDirectExpression>,
    structural_arguments: Vec<PreparedStructuralArgument>,
    claim_transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    result_type: QualifiedScalarType,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
    loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
) -> Result<LoweredRuntimeCallBinding, LoweringError> {
    let (binding, _) = crate::shared::call_sites::checked_runtime_invocation(checked, coordinate)?;
    let row = checked
        .facts
        .callable_contracts
        .contract_for_source(binding.contract)
        .map_err(LoweringError::Unsupported)?;
    let retained = row.signature().map_err(LoweringError::Unsupported)?;
    let contract = catalog.contract(row.id).ok_or(LoweringError::Unsupported(
        "runtime scalar call has no prepared callable contract",
    ))?;
    if callee.value_type(caller_value_types)?.scalar_type != ScalarType::Callable(contract.id)
        || contract.result.scalar() != Some(result_type.scalar_type)
        || !matches!(contract.result, terminal_psi::BoundaryMachineResult::Scalar(value) if value == result_type)
        || arguments.len() != contract.scalar_parameters.len()
        || structural_arguments.len() != contract.structural_parameters.len()
    {
        return unsupported("runtime scalar call changed its checked callable shape");
    }
    for (argument, parameter) in arguments.iter().zip(&contract.scalar_parameters) {
        if argument.value_type(caller_value_types)? != parameter.value_type() {
            return unsupported("runtime scalar call changed its formal argument qualifications");
        }
    }
    let statement_ordinal = u32::try_from(coordinate.statement_index)
        .map_err(|_| LoweringError::Unsupported("runtime call statement ordinal exceeds u32"))?;
    let call_ordinal = u32::try_from(coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("runtime call ordinal exceeds u32"))?;
    let erased_arguments = (0..retained.erased_scalar_parameters.len())
        .map(|ordinal| {
            scalar_bindings.expression_at(
                identities,
                checked,
                coordinate.state,
                statement_ordinal,
                CheckedScalarExpressionRole::ErasedUnitCallArgument {
                    call_ordinal,
                    erased_ordinal: u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("runtime erased argument ordinal exceeds u32")
                    })?,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let erased_proof_arguments = (0..retained.erased_proof_parameters.len())
        .map(|ordinal| {
            let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                call_ordinal,
                erased_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("runtime proof argument ordinal exceeds u32")
                })?,
            };
            let term = checked
                .facts
                .values
                .proof_terms
                .term_at(coordinate.state, statement_ordinal, role)
                .ok_or(LoweringError::Unsupported(
                    "runtime call lost an erased proof actual",
                ))?;
            checked_proof_term(identities, checked, term, caller_erased_proof_parameters)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (owner, _) = crate::shared::declarations::state_declaration(checked, coordinate.state)?;
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(owner.symbol)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(coordinate.state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "runtime call has no exact checked crash refinement",
        ))?;
    checked_call
        .runtime_callable_contract_identity(
            &checked.typed,
            &checked.facts.contract_plans,
            &checked.facts.proof,
            owner.symbol,
            binding,
        )
        .map_err(LoweringError::Unsupported)?;
    let capsule = checked
        .facts
        .contract_plans
        .runtime_callable_crash_capsule(binding.contract)
        .ok_or(LoweringError::Unsupported(
            "runtime call lost its published crash capsule",
        ))?;
    let claim_transfers = crate::emission::claim_bindings::prepare_checked_claim_transfers(
        identities,
        checked,
        claim_transfers,
        &retained.structural_parameters,
        structural_types,
    )?;
    Ok(LoweredRuntimeCallBinding {
        source_coordinate: coordinate,
        binding,
        caller_types: scalar_bindings.published_structural_types()?,
        loan_completions,
        result_type,
        callee,
        arguments,
        erased_arguments,
        erased_proof_arguments,
        structural_arguments,
        claim_transfers,
        crash_routes: capsule.published_buckets().to_vec(),
    })
}

pub(crate) fn target(
    contract: &TerminalCallableContract,
) -> crate::unit::attached_unit::ordinary_calls::Target<'_> {
    crate::unit::attached_unit::ordinary_calls::Target {
        parameters: &contract.structural_parameters,
        scalar_parameters: &contract.scalar_parameters,
        erased_scalar_parameters: &contract.proof.erased_scalar_formals,
        predicate_parameters: &contract.structural_parameters,
        requires: &contract.proof.requires,
        runtime_requirements: &contract.proof.requires,
        source_checked_requires: &contract.proof.source_checked_requires,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_scalar_call(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    call: &LoweredRuntimeCallBinding,
    parameters: &[ValueDeclaration],
    frontier: &lowered_psi::LoweredSourceFrontierBindings,
    caller_erased_formals: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<ValueId, LoweringError> {
    let row = checked
        .facts
        .callable_contracts
        .contract_for_source(call.binding.contract)
        .map_err(LoweringError::Unsupported)?;
    let catalog = calls.callables;
    let contract = catalog.contract(row.id).ok_or(LoweringError::Unsupported(
        "runtime call has no emitted contract declaration",
    ))?;
    let parameter_types = parameters
        .iter()
        .map(|parameter| parameter.value_type())
        .collect::<Vec<_>>();
    let callee = emit_direct_expression(&call.callee, parameters, next_value, operations);
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let value_type = argument.value_type(&parameter_types)?;
            Ok(ValueDeclaration {
                id: emit_direct_expression(argument, parameters, next_value, operations),
                scalar_type: value_type.scalar_type,
                qualifications: value_type.qualifications,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let erased_arguments = call
        .erased_arguments
        .iter()
        .map(|argument| lowered_erased_scalar_term(argument, parameters, caller_erased_formals))
        .collect::<Result<Vec<_>, _>>()?;
    let target = target(contract);
    let requirements = crate::unit::attached_unit::ordinary_calls::prepare_requirement_obligations(
        &target, calls, operations,
    )?;
    let inputs = crate::unit::attached_unit::ordinary_calls::prepare_computed_invocation(
        identities,
        checked,
        call.source_coordinate.into(),
        &call.caller_types,
        Some(target),
        requirements,
        &arguments,
        erased_arguments,
        &call.erased_proof_arguments,
        &call.structural_arguments,
        &call.claim_transfers,
        &call.crash_routes,
        parameters,
        caller_erased_formals,
        calls,
        next_value,
        operations,
        current_claims,
    )?;
    let result = mint_with(
        next_value,
        value_id,
        "runtime call result identity overflow",
    )?;
    let operation = operations.allocate();
    let (binding, site) =
        crate::shared::call_sites::checked_runtime_invocation(checked, call.source_coordinate)?;
    if binding != call.binding {
        return unsupported("runtime call changed its source binding during emission");
    }
    operations.record_source_call(call.source_coordinate, site, operation, binding, frontier)?;
    operations.push(Operation::unsuspended(
        operation,
        OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: call.result_type.qualifications,
        }),
        OperationKind::CallMachine {
            callee,
            contract: contract.id,
            arguments: inputs.arguments,
            erased_arguments: inputs.erased_arguments,
            erased_proof_arguments: inputs.erased_proof_arguments,
            structural_arguments: inputs.structural_arguments,
            claim_transfers: inputs.claim_transfers,
            requirement_obligations: inputs.requirement_obligations,
            crash_continuations: inputs.crash_continuations,
        },
    ));
    Ok(result)
}
