//! Prove a concrete integer's complete declared qualification without a tree rewrite.
//!
//! A domain subject is a lexical binder, not a same-spelled field or a type
//! name. Each predicate uses its declaration's exact subject and closed index
//! roster. Nested membership carries its original value term into the nested
//! declaration's roster. Unknown propositions and recursion supply no evidence;
//! routes and semantic roles cannot be minted by a numerical predicate proof.

use super::closed_integer_index_bindings;
use crate::proof_contracts::contract_entailment::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment, scoped_arithmetic_implication,
};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::types::TypeReferenceHandle;

/// The caller supplies the established integer denotation, either an exact
/// value or independently checked bounds. It is never inferred from the
/// qualification being requested.
pub(crate) fn integer_domain_membership(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    semantic_domain: SemanticDomainId,
    value: &ScopedArithmeticValue,
) -> StrictArithmeticImplicationJudgment {
    membership(
        program,
        domain_symbol,
        arguments,
        semantic_domain,
        value,
        &mut Vec::new(),
    )
}

fn membership(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    semantic_domain: SemanticDomainId,
    value: &ScopedArithmeticValue,
    active_domains: &mut Vec<SymbolHandle>,
) -> StrictArithmeticImplicationJudgment {
    use StrictArithmeticImplicationJudgment::{Proven, Refuted, Unknown};

    if !domain_symbol.is_valid() || active_domains.contains(&domain_symbol) {
        return Unknown;
    }
    let Some(definition) = program.domain_definition(domain_symbol) else {
        return Unknown;
    };
    if !definition.establishment_routes.is_empty()
        || definition.semantic_roles != Default::default()
        || !program
            .primitive_type_reference(definition.target_type)
            .is_some_and(|primitive| primitive.is_fixed_integer())
    {
        return Unknown;
    }
    let Some(indices) =
        closed_integer_index_bindings(program, definition, arguments, semantic_domain)
    else {
        return Unknown;
    };
    active_domains.push(domain_symbol);
    let judgment = (|| {
        if let Some(alias) = &definition.alias {
            if alias.constituents.is_empty() {
                return Unknown;
            }
            for constituent in &alias.constituents {
                let Some(nested) = program.domain_definition(constituent.domain_symbol) else {
                    return Unknown;
                };
                let judgment = membership(
                    program,
                    nested.symbol,
                    &[],
                    nested.semantic_id,
                    value,
                    active_domains,
                );
                if judgment != Proven {
                    return judgment;
                }
            }
            return Proven;
        }
        if !definition.predicate_body.is_present() {
            return if crate::field_domains::domain_is_vacuous(
                program,
                definition.symbol,
                &mut Vec::new(),
            ) {
                Proven
            } else {
                Unknown
            };
        }
        let mut bindings = indices
            .into_iter()
            .map(|(symbol, value)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(symbol),
                value: ScopedArithmeticValue::Integer(value),
            })
            .collect::<Vec<_>>();
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(definition.subject_symbol),
            value: value.clone(),
        });
        for fact in program.proof_facts.span_or_empty(definition.facts) {
            let judgment = match fact {
                ProofFact::Expression(expression) => scoped_arithmetic_implication(
                    program,
                    definition.symbol,
                    &[],
                    &ScopedArithmeticExpression {
                        owner: definition.symbol,
                        expression: *expression,
                        bindings: bindings.clone(),
                    },
                ),
                ProofFact::Membership(fact) => membership(
                    program,
                    fact.domain_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(fact.domain_arguments),
                    fact.semantic_domain,
                    &ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: definition.symbol,
                        expression: fact.value,
                        bindings: bindings.clone(),
                    }),
                    active_domains,
                ),
                ProofFact::Proposition(_) => Unknown,
            };
            if judgment == Refuted || judgment == Unknown {
                return judgment;
            }
        }
        Proven
    })();
    active_domains.pop();
    judgment
}
