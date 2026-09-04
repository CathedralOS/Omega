//! Exact target, static-argument, evidence-lane, and value-argument call custody.

use super::super::calls::{
    contract_call_value_receiver, exact_checked_contract_call_target,
    require_exact_contract_call_reference_arguments, resolved_contract_call_symbol,
};
use super::super::evidence::project_contract_call_evidence_arguments;
use super::super::static_arguments::{
    contract_call_static_parameter_kinds, project_contract_static_argument,
    require_exact_conformance_static_argument_selections,
    require_exact_named_const_static_argument_selections,
};
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{PackageReviewContractCallTarget, PackageReviewContractExpression};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{ExpressionHandle, TableCallExpression};

pub(super) fn project_call_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: ExpressionHandle,
    call: &TableCallExpression,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let target = exact_checked_contract_call_target(compilation, context, expression, call)?;
    let resolved_symbol = resolved_contract_call_symbol(compilation, call).or_else(|| {
        psi_typed_trees_to_checked_trees::derive_checked_nominal_call_target(
            &compilation.typed,
            &compilation.facts,
            expression,
        )
    });
    let static_parameter_kinds = match &target {
        PackageReviewContractCallTarget::Nominal(_) => {
            let target_symbol = resolved_symbol.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` contract call has no exact resolved target symbol",
                    context.subject_kind, context.subject_name
                ))]
            })?;
            require_exact_contract_call_reference_arguments(
                compilation,
                context,
                target_symbol,
                call,
            )?;
            contract_call_static_parameter_kinds(
                compilation,
                context,
                target_symbol,
                call.machine_arguments.len(),
            )?
        }
        PackageReviewContractCallTarget::BuiltinFunction(_) => {
            if !call.machine_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` supplies static arguments to a compiler-owned builtin function",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Vec::new()
        }
        PackageReviewContractCallTarget::ByteSequencePredicate(_) => {
            if !call.machine_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` supplies static arguments to a compiler-owned byte-sequence predicate",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Vec::new()
        }
        PackageReviewContractCallTarget::CollectionView(_) => {
            if !call.machine_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` supplies static arguments to a compiler-owned collection view",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Vec::new()
        }
    };
    if call.quotient_operation.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a quotient contract call not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]);
    }
    let evidence_arguments = if call.evidence_arguments.is_empty() {
        Vec::new()
    } else {
        project_contract_call_evidence_arguments(
            compilation,
            context,
            checked_fact,
            expression,
            resolved_symbol.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` evidence-bearing call has no exact resolved target",
                    context.subject_kind, context.subject_name,
                ))]
            })?,
            call.evidence_arguments.len(),
        )?
    };
    require_exact_named_const_static_argument_selections(
        compilation,
        context,
        expression,
        &call.machine_arguments,
    )?;
    require_exact_conformance_static_argument_selections(
        compilation,
        context,
        expression,
        &call.machine_arguments,
    )?;
    // Call-site suspend/block acknowledgement is diagnostic audit metadata,
    // explicitly outside contract identity. Fact-position calls have already
    // been checked as total and pure.
    Ok(PackageReviewContractExpression::Call {
        receiver: contract_call_value_receiver(compilation, call, resolved_symbol)
            .map(child)
            .transpose()?
            .map(Box::new),
        target,
        static_arguments: call
            .machine_arguments
            .iter()
            .enumerate()
            .zip(static_parameter_kinds)
            .map(|((static_argument_position, argument), parameter_kind)| {
                project_contract_static_argument(
                    compilation,
                    context,
                    binders,
                    checked_fact,
                    expression,
                    static_argument_position,
                    argument,
                    parameter_kind,
                    0,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        evidence_arguments,
        arguments: compilation
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .map(|argument| child(*argument))
            .collect::<Result<Vec<_>, _>>()?,
    })
}
