// Contract call-expression projection preserving exact target, static-argument,
// evidence-lane, and value-argument custody. Call-site suspend/block
// acknowledgement is diagnostic audit metadata outside contract identity because
// fact-position calls have already been checked as total and pure.

use super::super::calls::{
    contract_call_value_receiver, exact_checked_contract_call_target,
    exact_checked_runtime_contract_call, require_checked_contract_call_arguments,
    resolved_contract_call_symbol,
};
use super::super::evidence::project_contract_call_evidence_arguments;
use super::super::static_arguments::{
    contract_call_static_parameter_kinds, project_contract_static_argument,
    require_exact_conformance_static_argument_selections,
    require_exact_named_const_static_argument_selections,
};
use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{PackageReviewContractCallTarget, PackageReviewContractExpression};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};

pub(super) fn project_call_expression(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: ExpressionHandle,
    call: &TableCallExpression,
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    child: &impl Fn(ExpressionHandle) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if call.layout_query.is_some() {
        return super::super::layout_queries::project_layout_query(
            compilation,
            context,
            binders,
            expression,
            call,
        );
    }
    let runtime = exact_checked_runtime_contract_call(compilation, context, expression, call)?;
    let target = if let Some((binding, _)) = runtime {
        let fact = checked_fact.ok_or_else(|| {
            context.rejected("runtime proof call has no exact checked fact owner")
        })?;
        if !compilation
            .facts
            .proof
            .contract_facts
            .iter()
            .any(|(_, checked)| checked.owner == context.owner && checked.fact == fact)
            || !validation::expression_owners::proof_fact_contains_expression(
                compilation.checking_input,
                fact,
                expression,
            )
        {
            return Err(context.rejected("runtime proof call is outside its retained checked fact"));
        }
        let canonical = compilation
            .runtime_callable_contract_identity(binding.contract, binders)
            .map_err(|reason| {
                context.rejected(format!(
                    "runtime proof call has no complete contract identity: {reason}"
                ))
            })?;
        PackageReviewContractCallTarget::Runtime {
            contract: crate::record::PackageReviewTypeIdentity { canonical },
            callee: Box::new(child(binding.callee)?),
        }
    } else {
        exact_checked_contract_call_target(compilation, context, expression, call)?
    };
    let resolved_symbol = if runtime.is_some() {
        None
    } else {
        resolved_contract_call_symbol(compilation, call)
            .or_else(|| compilation.derive_checked_nominal_call_target(expression))
    };
    let static_parameter_kinds = match &target {
        PackageReviewContractCallTarget::Runtime { .. } => Vec::new(),
        PackageReviewContractCallTarget::Nominal(_) => {
            let target_symbol = resolved_symbol.ok_or_else(|| {
                context.rejected("contract call has no exact resolved target symbol")
            })?;
            require_checked_contract_call_arguments(
                compilation,
                context,
                target_symbol,
                expression,
                call,
            )?;
            contract_call_static_parameter_kinds(
                compilation,
                context,
                target_symbol,
                &call.machine_arguments,
            )?
        }
        PackageReviewContractCallTarget::BuiltinFunction(_) => {
            if !call.machine_arguments.is_empty() {
                return Err(context
                    .rejected("supplies static arguments to a compiler-owned builtin function"));
            }
            Vec::new()
        }
        PackageReviewContractCallTarget::ByteSequencePredicate(_) => {
            if !call.machine_arguments.is_empty() {
                return Err(context.rejected(
                    "supplies static arguments to a compiler-owned byte-sequence predicate",
                ));
            }
            Vec::new()
        }
        PackageReviewContractCallTarget::ProofView { .. } => {
            if !call.machine_arguments.is_empty() {
                return Err(
                    context.rejected("supplies static arguments to a compiler-owned proof view")
                );
            }
            Vec::new()
        }
    };
    if call.quotient_operation.is_some() {
        return Err(
            context.rejected("uses a quotient contract call not yet represented by package review")
        );
    }
    let evidence_arguments = if call.evidence_arguments.is_empty() {
        Vec::new()
    } else {
        project_contract_call_evidence_arguments(
            compilation,
            context,
            binders,
            checked_fact,
            expression,
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
