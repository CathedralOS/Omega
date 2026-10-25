//! Proof correspondence is not a rendered formula. Opaque propositions retain
//! their resolved declaration; transparent aliases expand under simultaneous
//! caller-owned operands using the existing structural index normalizer. An
//! unsupported operand contributes no proof, even inside an alias.

use checked_trees::DeclaredContractFact;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::proposition::{PropositionApplication, PropositionBody, PropositionFormula};
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, try_normalized_index_identity,
};

#[derive(PartialEq, Eq)]
pub(crate) enum CorrespondenceFact {
    Structural(DeclaredContractFact),
    Proposition {
        declaration: PropositionIdentity,
        binders: Vec<String>,
        arguments: Vec<String>,
    },
}

#[derive(PartialEq, Eq)]
pub(crate) enum PropositionIdentity {
    Declaration(SymbolHandle),
    Binder(String),
}

pub(crate) fn correspondence_fact(
    program: &dyn crate::CheckingInput,
    fact: &ProofFact,
    binders: &[(SymbolHandle, String)],
) -> Option<CorrespondenceFact> {
    if !crate::proof_contracts::contract_results::fact_has_source_builtin_index_meaning(
        program, fact,
    ) {
        return None;
    }
    match fact {
        ProofFact::Proposition(application) => proposition_fact(program, application, binders, &[]),
        _ => super::try_declared_fact(program, fact, binders, &[])
            .map(CorrespondenceFact::Structural),
    }
}

fn proposition_fact(
    program: &dyn crate::CheckingInput,
    application: &PropositionApplication,
    binders: &[(SymbolHandle, String)],
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<CorrespondenceFact> {
    if !program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .all(|argument| {
            crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(
                program, *argument,
            )
        })
    {
        return None;
    }
    if !application.proposition.is_valid()
        || scopes
            .iter()
            .any(|scope| scope.declaration == application.proposition)
    {
        return None;
    }
    let operands = application
        .binder_arguments
        .iter()
        .map(|argument| StaticMachineArgument {
            symbol: argument.symbol,
            path: argument.path.clone(),
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.clone(),
            ..StaticMachineArgument::default()
        })
        .collect::<Vec<_>>();
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    let declaration = program.proposition_definition_by_symbol(application.proposition);
    if let Some(declaration) = declaration {
        let parameters = program.proposition_parameters(declaration);
        let static_parameters = program.proposition_binders(declaration);
        if parameters.len() != arguments.len() || static_parameters.len() != operands.len() {
            return None;
        }
        if let PropositionBody::Transparent { proposition } = &declaration.body {
            let replacements = static_parameters
                .iter()
                .zip(&operands)
                .map(|(parameter, operand)| {
                    (
                        parameter.symbol,
                        IndexIdentityArgument::StaticArgument(operand),
                    )
                })
                .chain(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.symbol,
                                IndexIdentityArgument::Expression(*argument),
                            )
                        }),
                )
                .collect::<Vec<_>>();
            let mut applied = scopes.to_vec();
            applied.push(AppliedTypeIdentityScope {
                declaration: declaration.symbol,
                arguments: &replacements,
                lifetimes: &[],
            });
            return match proposition {
                PropositionFormula::Application(expansion) => {
                    proposition_fact(program, expansion, binders, &applied)
                }
                PropositionFormula::BooleanExpression(expression) => {
                    if !crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(program, *expression) {
                        return None;
                    }
                    let identity = try_normalized_index_identity(
                        program.typed_program(),
                        IndexIdentityArgument::Expression(*expression),
                        binders,
                        &applied,
                    )?;
                    Some(CorrespondenceFact::Structural(
                        DeclaredContractFact::Expression(identity),
                    ))
                }
            };
        }
    } else if program.symbols.get(application.proposition).kind
        != symbols::SymbolKind::PropositionParameter
    {
        return None;
    }
    let declaration = binders
        .iter()
        .find(|(symbol, _)| *symbol == application.proposition)
        .map_or(
            PropositionIdentity::Declaration(application.proposition),
            |(_, position)| PropositionIdentity::Binder(position.clone()),
        );
    Some(CorrespondenceFact::Proposition {
        declaration,
        binders: operands
            .iter()
            .map(|operand| {
                try_normalized_index_identity(
                    program.typed_program(),
                    IndexIdentityArgument::StaticArgument(operand),
                    binders,
                    scopes,
                )
            })
            .collect::<Option<Vec<_>>>()?,
        arguments: arguments
            .iter()
            .map(|argument| {
                try_normalized_index_identity(
                    program.typed_program(),
                    IndexIdentityArgument::Expression(*argument),
                    binders,
                    scopes,
                )
            })
            .collect::<Option<Vec<_>>>()?,
    })
}
