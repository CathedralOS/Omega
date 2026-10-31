use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::capture::contracts::expressions::names::portable_ordinal;
use crate::capture::contracts::expressions::projection::project_contract_expression_with_substitutions;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::record::{
    PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionParameterApplication,
};

use super::binders::{project_proposition_binder_argument, proposition_binder_value_expression};
use super::endpoint::project_proposition_endpoint;

fn require_exact_reference_argument(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    argument: typed_trees::expression::ExpressionHandle,
    expected_type: typed_trees::types::TypeReferenceHandle,
) -> Result<(), Vec<Diagnostic>> {
    if matches!(
        compilation.expression_table.expression(argument),
        typed_trees::expression::ExpressionNode::Borrow(_)
    ) && !validation::checked_argument_matches_type_reference(
        compilation.checking_input,
        argument,
        expected_type,
    ) {
        return Err(
            context.rejected("reference argument does not match its proposition parameter type")
        );
    }
    Ok(())
}

pub(crate) fn project_contract_proposition(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    application: &typed_trees::proposition::PropositionApplication,
    checked_fact: Option<arena::Handle<typed_trees::domain::ProofFact>>,
    binder_substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
    value_substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    visiting: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use typed_trees::proposition::{PropositionBody, PropositionFormula};

    if depth >= 64 {
        return Err(
            context.rejected("proposition expansion exceeds the package-review depth limit")
        );
    }
    if visiting.contains(&application.proposition) {
        return Err(context.rejected("proposition expansion is cyclic"));
    }
    if application.proposition.is_valid()
        && compilation.typed.symbols.get(application.proposition).kind
            == symbols::SymbolKind::PropositionParameter
    {
        if !application.binder_arguments.is_empty() {
            return Err(
                context.rejected("generic proposition endpoint has unexpected static arguments")
            );
        }
        let matching_parameters = compilation
            .typed
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .filter(|parameter| parameter.symbol == application.proposition)
            .collect::<Vec<_>>();
        let [parameter] = matching_parameters.as_slice() else {
            return Err(context.rejected(format!(
                "generic proposition endpoint rejoins {} declaration parameters; expected exactly one",
                matching_parameters.len(),
            )));
        };
        let typed_trees::data::TypeParameterKind::Proposition { contract } = &parameter.kind else {
            return Err(context.rejected(
                "generic proposition endpoint does not rejoin a proposition-family signature",
            ));
        };
        let argument_handles = compilation
            .expression_table
            .expression_handles(application.arguments);
        let parameters = compilation
            .typed
            .state_parameters
            .span_or_empty(contract.parameters);
        if argument_handles.len() != parameters.len() {
            return Err(
                context.rejected("generic proposition endpoint has inconsistent checked arity")
            );
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
                symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::PropositionParameter
            ) {
                if *symbol == application.proposition {
                    matching_ordinals.push(static_ordinal);
                }
                static_ordinal += 1;
            }
        }
        let [binder_ordinal] = matching_ordinals.as_slice() else {
            return Err(context.rejected(format!(
                "generic proposition endpoint rejoins {} callable static binders; expected exactly one",
                matching_ordinals.len(),
            )));
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
                binder_ordinal: portable_ordinal(*binder_ordinal)?,
                arguments,
            },
        ));
    }
    let declaration = compilation
        .proposition_definition_by_symbol(application.proposition)
        .ok_or_else(|| {
            context.rejected("contract refers to an unresolved or generic proposition endpoint")
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
        return Err(context.rejected(format!(
            "proposition `{}` has inconsistent checked arity",
            declaration.name
        )));
    }
    let binder_arguments = declaration_binders
        .iter()
        .zip(&application.binder_arguments)
        .map(|(binder, argument)| {
            let expected = match binder.kind {
                typed_trees::proposition::PropositionBinderKind::Type => {
                    typed_trees::proposition::PropositionBinderArgumentKind::Type
                }
                typed_trees::proposition::PropositionBinderKind::Const { .. } => {
                    typed_trees::proposition::PropositionBinderArgumentKind::Const
                }
                typed_trees::proposition::PropositionBinderKind::Machine => {
                    typed_trees::proposition::PropositionBinderArgumentKind::Machine
                }
            };
            if argument.kind != expected {
                return Err(context.rejected(format!(
                    "proposition `{}` binder kind changed during typing",
                    declaration.name
                )));
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
