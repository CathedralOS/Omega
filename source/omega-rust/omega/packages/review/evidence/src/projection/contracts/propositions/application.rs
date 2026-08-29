use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use crate::evidence::{
    PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionParameterApplication,
};
use crate::projection::contracts::expressions::names::portable_parameter_position;
use crate::projection::contracts::expressions::projection::project_contract_expression_with_substitutions;
use crate::projection::contracts::metadata::contracts::ContractProjectionContext;

use super::binders::{project_proposition_binder_argument, proposition_binder_value_expression};
use super::endpoint::project_proposition_endpoint;

fn require_exact_reference_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    argument: psi_typed_trees::expression::ExpressionHandle,
    expected_type: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<(), Vec<Diagnostic>> {
    if matches!(
        compilation.expression_table.expression(argument),
        psi_typed_trees::expression::ExpressionNode::Borrow(_)
    ) && !psi_validation::checked_argument_matches_type_reference(
        &compilation.typed,
        argument,
        expected_type,
    ) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` reference argument does not match its proposition parameter type",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
}

pub(crate) fn project_contract_proposition(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    application: &psi_typed_trees::proposition::PropositionApplication,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    binder_substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
    value_substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    visiting: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    if depth >= 64 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    if visiting.contains(&application.proposition) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion is cyclic",
            context.subject_kind, context.subject_name
        ))]);
    }
    if application.proposition.is_valid()
        && compilation.typed.symbols.get(application.proposition).kind
            == psi_symbols::SymbolKind::PropositionParameter
    {
        if !application.binder_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has unexpected static arguments",
                context.subject_kind, context.subject_name
            ))]);
        }
        let matching_parameters = compilation
            .typed
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .filter(|parameter| parameter.symbol == application.proposition)
            .collect::<Vec<_>>();
        let [parameter] = matching_parameters.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} declaration parameters; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_parameters.len()
            ))]);
        };
        let psi_typed_trees::data::TypeParameterKind::Proposition { contract } = &parameter.kind
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint does not rejoin a proposition-family signature",
                context.subject_kind, context.subject_name
            ))]);
        };
        let argument_handles = compilation
            .expression_table
            .expression_handles(application.arguments);
        let parameters = compilation
            .typed
            .state_parameters
            .span_or_empty(contract.parameters);
        if argument_handles.len() != parameters.len() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has inconsistent checked arity",
                context.subject_kind, context.subject_name
            ))]);
        }
        for (argument, parameter) in argument_handles.iter().zip(parameters) {
            require_exact_reference_argument(
                compilation,
                context,
                *argument,
                parameter.type_reference,
            )?;
        }
        let mut static_ordinal = 0usize;
        let mut matching_ordinals = Vec::new();
        for (symbol, _) in callable_binders {
            if matches!(
                compilation.typed.symbols.get(*symbol).kind,
                psi_symbols::SymbolKind::TypeParameter
                    | psi_symbols::SymbolKind::MachineParameter
                    | psi_symbols::SymbolKind::PropositionParameter
            ) {
                if *symbol == application.proposition {
                    matching_ordinals.push(static_ordinal);
                }
                static_ordinal += 1;
            }
        }
        let [binder_ordinal] = matching_ordinals.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} callable static binders; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_ordinals.len()
            ))]);
        };
        let arguments = argument_handles
            .iter()
            .map(|argument| {
                project_contract_expression_with_substitutions(
                    compilation,
                    context,
                    callable_binders,
                    *argument,
                    value_substitutions,
                    &[],
                    checked_fact,
                    0,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractFact::PropositionParameter(
            PackageReviewPropositionParameterApplication {
                binder_ordinal: portable_parameter_position(*binder_ordinal)?,
                arguments,
            },
        ));
    }
    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == application.proposition)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract refers to an unresolved or generic proposition endpoint",
                context.subject_kind, context.subject_name
            ))]
        })?;
    let declaration_binders = compilation.proposition_binders(declaration);
    let declaration_parameters = compilation.proposition_parameters(declaration);
    if declaration_binders.len() != application.binder_arguments.len()
        || declaration_parameters.len()
            != compilation
                .expression_table
                .expression_handles(application.arguments)
                .len()
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition `{}` has inconsistent checked arity",
            context.subject_kind, context.subject_name, declaration.name
        ))]);
    }
    let binder_arguments = declaration_binders
        .iter()
        .zip(&application.binder_arguments)
        .map(|(binder, argument)| {
            let expected = match binder.kind {
                psi_typed_trees::proposition::PropositionBinderKind::Type => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Type
                }
                psi_typed_trees::proposition::PropositionBinderKind::Const { .. } => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Const
                }
                psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine
                }
            };
            if argument.kind != expected {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition `{}` binder kind changed during typing",
                    context.subject_kind, context.subject_name, declaration.name
                ))]);
            }
            project_proposition_binder_argument(
                compilation,
                context,
                callable_binders,
                argument,
                binder_substitutions,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = compilation
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .zip(declaration_parameters)
        .map(|(argument, parameter)| {
            require_exact_reference_argument(
                compilation,
                context,
                *argument,
                parameter.type_reference,
            )?;
            project_contract_expression_with_substitutions(
                compilation,
                context,
                callable_binders,
                *argument,
                value_substitutions,
                &[],
                checked_fact,
                0,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    match &declaration.body {
        PropositionBody::Primitive | PropositionBody::Witness { .. } => Ok(
            PackageReviewContractFact::Proposition(project_proposition_endpoint(
                compilation,
                declaration,
                binder_arguments,
                arguments,
            )?),
        ),
        PropositionBody::Transparent { proposition } => {
            visiting.push(declaration.symbol);
            let mut nested_binders = binder_substitutions.to_vec();
            nested_binders.extend(
                declaration_binders
                    .iter()
                    .zip(&binder_arguments)
                    .map(|(binder, argument)| (binder.symbol, argument.clone())),
            );
            let mut nested_values = value_substitutions.to_vec();
            nested_values.extend(
                declaration_parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, argument)| (parameter.symbol, argument.clone())),
            );
            for (binder, argument) in declaration_binders.iter().zip(&binder_arguments) {
                if let Some(value) = proposition_binder_value_expression(argument) {
                    nested_values.push((binder.symbol, value));
                }
            }
            let projected = match proposition {
                PropositionFormula::Application(expansion) => project_contract_proposition(
                    compilation,
                    context,
                    callable_binders,
                    expansion,
                    checked_fact,
                    &nested_binders,
                    &nested_values,
                    visiting,
                    depth + 1,
                ),
                PropositionFormula::BooleanExpression(expression) => {
                    let projection_substitutions = declaration_parameters
                        .iter()
                        .zip(
                            compilation
                                .expression_table
                                .expression_handles(application.arguments),
                        )
                        .map(|(parameter, argument)| (parameter.symbol, *argument))
                        .collect::<Vec<_>>();
                    project_contract_expression_with_substitutions(
                        compilation,
                        context,
                        callable_binders,
                        *expression,
                        &nested_values,
                        &projection_substitutions,
                        checked_fact,
                        0,
                    )
                    .map(PackageReviewContractFact::Expression)
                }
            };
            visiting.pop();
            projected
        }
    }
}
