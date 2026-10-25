//! Project checked declarations into the public identity vocabulary. Resolution
//! and type/expression normalization already own semantic naming; this owner
//! only supplies positional binders and the declared crash-clause algebra.
//! It does not interpret diagnostic text or assign a parallel byte grammar.
//! Preparation and checked-fact construction borrow the same queries; neither
//! pipeline mutation nor proof search is a dependency of this projection.

use checked_trees::{
    DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact, DeclaredContractKind,
    DeclaredContractParameter, DeclaredProgressPremise, DeclaredTermination,
};
use symbols::SymbolHandle;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::type_identity::{TypeIdentityRequest, normalized_expression_identity};

mod correspondence;
pub(crate) use correspondence::{CorrespondenceFact, correspondence_fact};

/// Bind generic declarations by their authored position and exact symbol.
pub fn generic_binders(
    parameters: &[typed_trees::data::TypeParameter],
) -> Vec<(SymbolHandle, String)> {
    parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| (parameter.symbol, format!("$G{position}")))
        .collect()
}

/// Extend generic binders with a separate positional value-parameter namespace.
pub fn callable_binders(
    generics: &[(SymbolHandle, String)],
    parameters: &[StateParameter],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = generics.to_vec();
    binders.extend(
        parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| (parameter.symbol, format!("$V{position}"))),
    );
    binders
}

/// Requirement signatures can live in a trait or a nested machine-parameter
/// telescope. Bind the retained enclosing scopes, not words found in a type.
/// Each scope's parameter arena order is its authored telescope order.
pub fn signature_generic_binders(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<(SymbolHandle, String)> {
    let mut scopes = Vec::new();
    let mut scope = signature.symbol;
    while scope.is_valid() && !scopes.contains(&scope) {
        scopes.push(scope);
        scope = program.symbols.get(scope).parent;
    }
    let mut binders = Vec::<(SymbolHandle, String)>::new();
    for scope in scopes.into_iter().rev() {
        for (_, parameter) in program.data_type_parameters.iter() {
            if parameter.symbol.is_valid()
                && program.symbols.get(parameter.symbol).parent == scope
                && !binders
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
            {
                let position = binders.len();
                binders.push((parameter.symbol, format!("$G{position}")));
            }
        }
    }
    for parameter in program.state_signature_type_parameters(signature) {
        if !binders
            .iter()
            .any(|(symbol, _)| *symbol == parameter.symbol)
        {
            let position = binders.len();
            binders.push((parameter.symbol, format!("$G{position}")));
        }
    }
    binders
}

/// Project a callable's declared types and clauses using positional binders.
pub fn declared_callable(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    result: typed_trees::types::TypeReferenceHandle,
    contracts: &[SignatureContract],
    generics: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    include_crashes: bool,
) -> Result<DeclaredCallableContract, diagnostics::Diagnostic> {
    let binders = callable_binders(generics, parameters);
    Ok(DeclaredCallableContract {
        parameters: parameters
            .iter()
            .map(|parameter| DeclaredContractParameter {
                mutable: parameter.is_mutable,
                constant: parameter.is_const,
                relevance: parameter.relevance,
                type_identity: program.type_identity(TypeIdentityRequest {
                    binders: &binders,
                    ..TypeIdentityRequest::ordinary(parameter.type_reference)
                }),
            })
            .collect(),
        result: program.type_identity(TypeIdentityRequest {
            binders: &binders,
            ..TypeIdentityRequest::ordinary(result)
        }),
        clauses: declared_contracts(
            program,
            contracts,
            &binders,
            content_conservation,
            include_crashes,
        )?,
    })
}

/// Project and sort declared clauses, merging same-cause crash alternatives.
pub fn declared_contracts(
    program: &dyn crate::CheckingInput,
    contracts: &[SignatureContract],
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    include_crashes: bool,
) -> Result<Vec<DeclaredContractClause>, diagnostics::Diagnostic> {
    let mut clauses = Vec::new();
    let mut crashes = Vec::<(checked_trees::CrashCause, bool, Vec<DeclaredContractFact>)>::new();
    for contract in contracts {
        let facts = program.proof_facts.span_or_empty(contract.facts);
        let project = || {
            facts
                .iter()
                .map(|fact| declared_fact(program, fact, binders, content_conservation))
                .collect::<Result<Vec<_>, _>>()
        };
        let kind = match contract.kind {
            SignatureContractKind::Requires => DeclaredContractKind::Requires,
            SignatureContractKind::Ensures => DeclaredContractKind::Ensures,
            SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } => DeclaredContractKind::EnsuresForResultCase {
                data: symbol_identity(program, result_data, binders),
                case: symbol_identity(program, result_case, binders),
            },
            SignatureContractKind::Crashes { cause } => {
                if !include_crashes {
                    continue;
                }
                let cause = checked_trees::CrashCause::from(cause);
                let position = crashes
                    .iter()
                    .position(|bucket| bucket.0 == cause)
                    .unwrap_or_else(|| {
                        crashes.push((cause, false, Vec::new()));
                        crashes.len() - 1
                    });
                let bucket = &mut crashes[position];
                let projected = project()?;
                if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                    bucket.1 = true;
                } else {
                    bucket.2.extend(projected);
                }
                continue;
            }
        };
        clauses.extend(project()?.into_iter().map(|fact| DeclaredContractClause {
            kind: kind.clone(),
            fact: Some(fact),
        }));
    }
    for (cause, unconditional, mut routes) in crashes {
        let kind = DeclaredContractKind::Crashes(cause);
        if unconditional {
            clauses.push(DeclaredContractClause { kind, fact: None });
        } else {
            routes.sort();
            routes.dedup();
            clauses.extend(routes.into_iter().map(|fact| DeclaredContractClause {
                kind: kind.clone(),
                fact: Some(fact),
            }));
        }
    }
    clauses.sort();
    Ok(clauses)
}

/// Project one fact without using its diagnostic spelling as binder identity.
pub fn declared_fact(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> Result<DeclaredContractFact, diagnostics::Diagnostic> {
    project_declared_fact(program, fact, binders, content_conservation, |expression| {
        normalized_expression_identity(program, expression, binders)
    })
    .ok_or_else(|| diagnostics::Diagnostic::error("declared contract fact has no complete normalized identity; an unbound subject or unresolved proposition cannot define a contract"))
}

fn try_declared_fact(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> Option<DeclaredContractFact> {
    project_declared_fact(program, fact, binders, content_conservation, |expression| {
        if !super::contract_results::expression_has_source_builtin_index_meaning(
            program, expression,
        ) {
            return None;
        }
        typed_trees::type_identity::try_normalized_index_identity(
            program.typed_program(),
            typed_trees::type_identity::IndexIdentityArgument::Expression(expression),
            binders,
            &[],
        )
    })
}

// Descriptive publication and proof correspondence share the fact structure.
// The latter supplies a fallible expression projection at every nested value;
// wrapping an incomplete call in membership/proposition syntax grants nothing.
fn project_declared_fact(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    expression_identity: impl Fn(typed_trees::expression::ExpressionHandle) -> Option<String>,
) -> Option<DeclaredContractFact> {
    use typed_trees::domain::ProofFact;
    Some(match fact {
        ProofFact::Expression(expression) => {
            if let Some(conservation) = content_conservation
                .iter()
                .find(|candidate| candidate.source_expression == *expression)
            {
                return Some(DeclaredContractFact::ContentConservation(
                    language_semantics::content::content_conservation_plan_bytes(
                        &conservation.plan,
                    ),
                ));
            }
            DeclaredContractFact::Expression(expression_identity(*expression)?)
        }
        ProofFact::Membership(membership) => DeclaredContractFact::Membership {
            value: expression_identity(membership.value)?,
            domain: symbol_identity(program, membership.domain_symbol, binders),
            arguments: program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments)
                .iter()
                .map(|argument| {
                    program.type_identity(TypeIdentityRequest {
                        binders,
                        ..TypeIdentityRequest::ordinary(*argument)
                    })
                })
                .collect(),
        },
        ProofFact::Proposition(application) => {
            let binder_labels = application
                .binder_arguments
                .iter()
                .map(|argument| {
                    if let Some(literal) = &argument.const_literal {
                        return literal.value_bignum().map(|value| value.to_string());
                    }
                    let identity = symbol_identity(program, argument.symbol, binders);
                    if let Some(projection) = &argument.evidence_projection {
                        Some(format!("{identity}.{}", projection.member))
                    } else {
                        Some(identity)
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            let argument_labels = program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| expression_identity(*argument))
                .collect::<Option<Vec<_>>>()?;
            let formula = program.normalize_proposition_application(
                application,
                Some(typed_trees::proposition::PropositionLabels {
                    binder_labels: &binder_labels,
                    argument_labels: &argument_labels,
                }),
            )?;
            DeclaredContractFact::Proposition(formula.identity_label())
        }
    })
}

fn symbol_identity(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    binders: &[(SymbolHandle, String)],
) -> String {
    if let Some((_, identity)) = binders.iter().find(|(binder, _)| *binder == symbol) {
        return identity.clone();
    }
    program.symbols.display_path(symbol, "::")
}

/// Project published progress premises with canonical sorting and deduplication.
pub fn declared_termination(
    program: &dyn crate::CheckingInput,
    termination: &language_semantics::TerminationInterface,
    binders: &[(SymbolHandle, String)],
) -> DeclaredTermination {
    use language_semantics::{TerminationGuarantee, TerminationInterface};
    match termination {
        TerminationInterface::InternalDerived => DeclaredTermination::InternalDerived,
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
            DeclaredTermination::NoGuarantee
        }
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            let mut premises = premises
                .iter()
                .map(|premise| DeclaredProgressPremise {
                    profile: program
                        .semantic_domains
                        .name(premise.profile)
                        .expect("checked progress premise must retain its semantic domain")
                        .to_owned(),
                    subject: symbol_identity(program, premise.subject.root, binders),
                    projections: premise
                        .subject
                        .projections
                        .iter()
                        .map(|projection| symbol_identity(program, *projection, &[]))
                        .collect(),
                })
                .collect::<Vec<_>>();
            premises.sort();
            premises.dedup();
            DeclaredTermination::Terminates(premises)
        }
    }
}

/// Recognize the literal truth route in the declared crash-clause algebra.
pub fn is_true_crash_route(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    matches!(
        fact,
        typed_trees::domain::ProofFact::Expression(expression)
            if matches!(
                program.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_contract_hash_uses_owned_type_content_across_mint_orders() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::types::TypeReferenceNode;
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Alpha")),
                (SymbolKind::Data, SymbolNameRef::Static("Beta")),
            ],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let alpha = children.next().unwrap();
        let beta = children.next().unwrap();
        let mut first = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let alpha_type = first.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: alpha,
            name: "Alpha".into(),
        });
        let beta_type = first.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: beta,
            name: "Beta".into(),
        });
        let second = first.clone();
        let different = first.clone();
        let first_alpha = first.type_identity(TypeIdentityRequest::ordinary(alpha_type));
        second.type_identity(TypeIdentityRequest::ordinary(beta_type));
        let second_alpha = second.type_identity(TypeIdentityRequest::ordinary(alpha_type));
        let different_beta = different.type_identity(TypeIdentityRequest::ordinary(beta_type));
        assert_ne!(first_alpha, second_alpha);
        assert_eq!(first_alpha, different_beta);
        let contract = |program: &typed_trees::TypedTrees, value| {
            let parameters = [StateParameter {
                type_reference: value,
                ..Default::default()
            }];
            let declaration = checked_trees::DeclaredContractStructure {
                callables: vec![
                    declared_callable(program, &parameters, value, &[], &[], &[], true)
                        .expect("formed callable contract"),
                ],
                clauses: Vec::new(),
            };
            program.type_identities.with(|table| {
                checked_trees::contract_identity(
                    language_semantics::MachineSupplyMode::CheckedBody,
                    None,
                    &[],
                    language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                    &[],
                    language_semantics::SuspensionInterface::PublishedMaySuspend(false),
                    language_semantics::BlockingInterface::PublishedMayBlock(false),
                    checked_trees::CrashInterface::PublishedCeiling,
                    &checked_trees::DeclaredTermination::NoGuarantee,
                    &declaration,
                    table,
                )
                .commitment
            })
        };
        assert_eq!(contract(&first, alpha_type), contract(&second, alpha_type));
        assert_ne!(
            contract(&first, alpha_type),
            contract(&different, beta_type)
        );
    }

    #[test]
    fn nominal_type_spelling_cannot_be_replaced_by_a_generic_binder() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::types::TypeReferenceNode;

        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("T")),
                (SymbolKind::Machine, SymbolNameRef::Static("convert")),
            ],
        );
        let mut declarations = SymbolTableBuilder::child_handles(declarations);
        let nominal = declarations.next().unwrap();
        let machine = declarations.next().unwrap();
        let parameters = symbols.insert_children(
            machine,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Parameter, SymbolNameRef::Static("value")),
            ],
        );
        let mut parameters = SymbolTableBuilder::child_handles(parameters);
        let generic = parameters.next().unwrap();
        let formal = parameters.next().unwrap();
        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let nominal_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: nominal,
                name: "T".into(),
            });
        let generic_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: generic,
                name: "T".into(),
            });
        let mut parameters = [StateParameter {
            symbol: formal,
            name: "value".into(),
            type_reference: nominal_type,
            ..Default::default()
        }];
        let binders = [(generic, "$G0".to_owned())];
        let original = declared_callable(
            &program,
            &parameters,
            generic_type,
            &[],
            &binders,
            &[],
            true,
        )
        .expect("formed original callable contract");
        // Lexical replacement used to rewrite the independently selected
        // nominal T too. Changing only the binder's name must do nothing.
        program.symbols.rename(generic, "Element");
        program.symbols.rename(formal, "argument");
        parameters[0].name = "argument".into();
        let renamed = declared_callable(
            &program,
            &parameters,
            generic_type,
            &[],
            &binders,
            &[],
            true,
        )
        .expect("formed renamed callable contract");
        assert_eq!(original, renamed);
        assert_ne!(original.parameters[0].type_identity, original.result);
        parameters[0].type_reference = generic_type;
        assert_ne!(
            original,
            declared_callable(
                &program,
                &parameters,
                generic_type,
                &[],
                &binders,
                &[],
                true
            )
            .expect("formed generic callable contract")
        );
    }
}
