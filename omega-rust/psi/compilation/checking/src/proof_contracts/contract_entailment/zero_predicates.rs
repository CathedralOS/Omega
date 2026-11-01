//! Default-domain admission observes zero operands, never assumed membership.
//!
//! Use the same exact declaration substitution and predicate judgment as an
//! explicit constructor. The first case is fixed by representation; no later
//! satisfiable case may replace it. Contained field qualifications and custody
//! are still checked independently by the establishment owner.

use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::signature::StateParameter;

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::{StructuralJudge, StructuralJudgment};
use super::structural_terms::{zero_constructor_operands, zero_value_structural_term};

pub fn data_predicates_admit_zero(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> bool {
    if !definition.type_parameters.is_empty() || definition.where_facts.is_empty() {
        return false;
    }
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();
    for member in program.data_members(definition) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        if let Some(value) = zero_value_structural_term(program, field.type_reference) {
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: field.type_reference,
                ..Default::default()
            });
            bindings.push((field.name.to_string(), value));
        }
    }
    if definition.predicate_subject_symbol.is_valid()
        && definition.predicate_subject_type.is_valid()
        && let Some(value) = zero_constructor_operands(program, definition)
    {
        parameters.push(StateParameter {
            symbol: definition.predicate_subject_symbol,
            name: "self".into(),
            type_reference: definition.predicate_subject_type,
            ..Default::default()
        });
        bindings.push(("self".to_owned(), value));
    }
    // No default predicate enters the premise set. In particular, forming a
    // subject term must not give the judgment the invariant being established.
    let judge = StructuralJudge::from_declaration_requires(
        program,
        definition.symbol,
        &parameters,
        &[],
        false,
    );
    program
        .conjunctive_proof_facts(definition.where_facts)
        .all(|fact| {
            let Some(expression) = fact.as_expression() else {
                return false;
            };
            let judgment = instantiated_selected_fact_judgment(
                    program,
                    &judge,
                    definition.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                    &[],
                    &[],
                    None,
                );
            if super::struct_trace() && !matches!(judgment, StructuralJudgment::Proven) {
                let term = super::structural_terms::structural_term(
                    program, None, Some((definition.symbol, &parameters)), *expression, None,
                );
                let instantiated = term.as_ref().map(|term| StructuralJudge::substitute_call_term(term, &bindings, &[]));
                let resolved = instantiated.map(|term| judge.resolve(term));
                eprintln!("STRUCT zero-predicate data={} expression={} judgment={judgment:?} term={term:?} resolved={resolved:?}", definition.name, program.expression_table.display_name(*expression));
            }
            matches!(judgment, StructuralJudgment::Proven)
                || crate::proof_contracts::collection_predicates::data_fact_holds_at_zero(
                program,
                definition,
                *expression,
            )
        })
}

/// Zero admission of an exact, predicate-only applied theory. The carrier's
/// own establishment is checked separately by the enclosing type walk. No
/// membership, route, semantic contribution or receiving annotation is a premise.
pub(crate) fn domain_predicates_admit_zero(
    program: &dyn crate::CheckingInput,
    carrier: typed_trees::types::TypeReferenceHandle,
    constraint: &typed_trees::types::DomainConstraint,
) -> bool {
    let Some(domain) =
        program.domain_definition_for_instance(constraint.symbol, constraint.semantic_id)
    else {
        return false;
    };
    if domain.alias.is_some()
        || !domain.establishment_routes.is_empty()
        || !domain.semantic_roles.is_empty()
        || !constraint.establishment_routes.is_empty()
        || !constraint.semantic_roles.is_empty()
        || !typed_trees::domain::has_bound_symbol_only_proof(program, domain)
    {
        return false;
    }
    let Some(value) = zero_value_structural_term(program, carrier) else {
        return false;
    };
    let parameters = [StateParameter {
        symbol: domain.subject_symbol,
        name: domain.subject_name.clone(),
        type_reference: carrier,
        ..Default::default()
    }];
    let bindings = [(domain.subject_name.to_string(), value)];
    let judge =
        StructuralJudge::from_declaration_requires(program, domain.symbol, &parameters, &[], false);
    program.proof_facts(domain).iter().all(|fact| {
        fact.as_expression().is_some_and(|expression| {
            matches!(
                instantiated_selected_fact_judgment(
                    program,
                    &judge,
                    domain.symbol,
                    &parameters,
                    *expression,
                    &bindings,
                    &[],
                    &[],
                    None,
                ),
                StructuralJudgment::Proven
            )
        })
    })
}
