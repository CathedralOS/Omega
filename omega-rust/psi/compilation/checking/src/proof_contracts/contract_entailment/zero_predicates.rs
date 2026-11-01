//! Default-domain admission observes zero operands, never assumed membership.
//!
//! Use the same exact declaration substitution and predicate judgment as an
//! explicit constructor. The first case is fixed by representation; no later
//! satisfiable case may replace it. Contained field qualifications and custody
//! are still checked independently by the establishment owner.

use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceNode;

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
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
    if let Some(origin) = definition.generic_instance
        && let TypeReferenceNode::Generic { base_symbol, .. } =
            program.type_reference_table.type_reference(origin)
        && let Some(template) = program.unique_data_definition_by_symbol(*base_symbol)
    {
        for parameter in program.data_type_parameters(template) {
            let Some((_, value)) = crate::proof_contracts::default_domains::applied_const_integer(
                program,
                definition,
                parameter.symbol,
            ) else {
                continue;
            };
            let (typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                continue;
            };
            // A retained static actual is not zeroed storage. Bind its exact
            // declaration symbol into this proof frame before interpreting the
            // predicate. Private names cannot collide with authored fields.
            let name = format!(
                "\0data-constant:{}:{}",
                parameter.symbol.arena_index(),
                parameter.symbol.generation(),
            );
            parameters.push(StateParameter {
                symbol: parameter.symbol,
                name: name.as_str().into(),
                type_reference,
                ..Default::default()
            });
            bindings.push((
                name,
                StructuralTerm::Integer(numerics::bignum::BigInt::from_i128(value)),
            ));
        }
    }
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
