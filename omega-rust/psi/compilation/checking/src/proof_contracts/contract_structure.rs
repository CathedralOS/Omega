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
) -> DeclaredCallableContract {
    let binders = callable_binders(generics, parameters);
    DeclaredCallableContract {
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
        ),
    }
}

/// Machine declaration contracts share one normalized clause projection at
/// checking publication, template commitments and external review boundaries.
pub fn declared_machine_contracts(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> Vec<DeclaredContractClause> {
    let mut clauses = declared_contracts(
        program,
        program.machine_contracts(machine),
        binders,
        content_conservation,
        true,
    );
    clauses.extend(declared_where_facts(
        program,
        machine.where_facts,
        binders,
        content_conservation,
    ));
    clauses.sort();
    clauses.dedup();
    clauses
}

/// Predicate eligibility is committed separately from runtime preconditions.
/// The same fact normalizer owns both clause vocabularies.
pub fn declared_where_facts(
    program: &dyn crate::CheckingInput,
    facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> Vec<DeclaredContractClause> {
    let mut clauses = program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .map(|fact| DeclaredContractClause {
            kind: DeclaredContractKind::Where,
            fact: Some(declared_fact(program, fact, binders, content_conservation)),
        })
        .collect::<Vec<_>>();
    clauses.sort();
    clauses.dedup();
    clauses
}

/// Project and sort declared clauses, merging same-cause crash alternatives.
pub fn declared_contracts(
    program: &dyn crate::CheckingInput,
    contracts: &[SignatureContract],
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
    include_crashes: bool,
) -> Vec<DeclaredContractClause> {
    let mut clauses = Vec::new();
    let mut crashes = Vec::<(checked_trees::CrashCause, bool, Vec<DeclaredContractFact>)>::new();
    for contract in contracts {
        let facts = program.proof_facts.span_or_empty(contract.facts);
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
                if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                    bucket.1 = true;
                } else {
                    bucket.2.extend(
                        facts.iter().map(|fact| {
                            declared_fact(program, fact, binders, content_conservation)
                        }),
                    );
                }
                continue;
            }
        };
        clauses.extend(facts.iter().map(|fact| DeclaredContractClause {
            kind: kind.clone(),
            fact: Some(declared_fact(program, fact, binders, content_conservation)),
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
    clauses
}

/// Project one fact without using its diagnostic spelling as binder identity.
pub fn declared_fact(
    program: &dyn crate::CheckingInput,
    fact: &typed_trees::domain::ProofFact,
    binders: &[(SymbolHandle, String)],
    content_conservation: &[crate::ContentConservationSourcePlan],
) -> DeclaredContractFact {
    use typed_trees::domain::ProofFact;
    match fact {
        ProofFact::Expression(expression) => {
            if let Some(conservation) = content_conservation
                .iter()
                .find(|candidate| candidate.source_expression == *expression)
            {
                return DeclaredContractFact::ContentConservation(
                    language_semantics::content::content_conservation_plan_bytes(
                        &conservation.plan,
                    ),
                );
            }
            DeclaredContractFact::Expression(normalized_expression_identity(
                program,
                *expression,
                binders,
            ))
        }
        ProofFact::Membership(membership) => DeclaredContractFact::Membership {
            value: normalized_expression_identity(program, membership.value, binders),
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
                        return literal
                            .value_bignum()
                            .expect("checked proposition const must be an integer")
                            .to_string();
                    }
                    let identity = symbol_identity(program, argument.symbol, binders);
                    if let Some(projection) = &argument.evidence_projection {
                        format!("{identity}.{}", projection.member)
                    } else {
                        identity
                    }
                })
                .collect::<Vec<_>>();
            let argument_labels = program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| normalized_expression_identity(program, *argument, binders))
                .collect::<Vec<_>>();
            let formula = program
                .normalize_proposition_application(
                    application,
                    Some(typed_trees::proposition::PropositionLabels {
                        binder_labels: &binder_labels,
                        argument_labels: &argument_labels,
                    }),
                )
                .expect("checked proposition application must have a normalized formula");
            DeclaredContractFact::Proposition(formula.identity_label())
        }
    }
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
        );
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
        );
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
        );
    }
}
