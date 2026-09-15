//! Collecting the result dispatch terms of a domain.

use crate::TypedTrees;
use crate::typed_trees::type_system::type_identity::NormalizedDomainTerm;
use crate::typed_trees::type_system::type_identity::constraint_identity::normalized_compiler_domain_identity;
use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext;
use crate::types::{
    DomainConstraint, DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use symbols::SymbolHandle;

pub(crate) fn collect_result_dispatch_terms(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    terms: &mut Vec<NormalizedDomainTerm>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_result_dispatch_terms(program, *referee, terms, alias_stack);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_result_dispatch_terms(program, *base_type, terms, alias_stack);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        terms.push(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
                    }
                    TypeConstraintNode::Domain(domain) => {
                        if domain.subject == DomainConstraintSubject::Declared {
                            collect_declared_result_dispatch_terms(
                                program,
                                domain,
                                terms,
                                alias_stack,
                            );
                        } else {
                            terms.push(NormalizedDomainTerm::Compiler(
                                normalized_compiler_domain_identity(
                                    program,
                                    domain,
                                    &TypeIdentityContext::default(),
                                ),
                            ));
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::Range { .. } => {}
                }
            }
        }
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn collect_declared_result_dispatch_terms(
    program: &TypedTrees,
    constraint: &DomainConstraint,
    terms: &mut Vec<NormalizedDomainTerm>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    let definition = constraint.symbol.is_valid().then(|| {
        program
            .domain_definitions()
            .iter()
            .find(|definition| definition.symbol == constraint.symbol)
    });
    if let Some(Some(definition)) = definition {
        if let Some(alias) = definition.alias.as_ref() {
            if alias_stack.contains(&definition.symbol) {
                return;
            }
            alias_stack.push(definition.symbol);
            for constituent in &alias.constituents {
                let Some(constituent_definition) = program
                    .domain_definitions()
                    .iter()
                    .find(|candidate| candidate.symbol == constituent.domain_symbol)
                else {
                    continue;
                };
                let constituent_constraint = DomainConstraint {
                    name: constituent_definition.name.clone(),
                    arguments: Vec::new(),
                    subject: crate::types::DomainConstraintSubject::Declared,
                    symbol: constituent_definition.symbol,
                    semantic_id: constituent_definition.semantic_id,
                    classification: constituent_definition.classification,
                    predicate_body: constituent_definition.predicate_body,
                    semantic_roles: constituent_definition.semantic_roles,
                    establishment_routes: constituent_definition.establishment_routes.clone(),
                    authored_selection: None,
                };
                collect_declared_result_dispatch_terms(
                    program,
                    &constituent_constraint,
                    terms,
                    alias_stack,
                );
            }
            alias_stack.pop();
            return;
        }
        if definition.predicate_body.is_present()
            && definition.semantic_roles.is_empty()
            && definition.establishment_routes.is_empty()
        {
            return;
        }
        terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
            program,
            definition.semantic_id,
            definition.name.as_str(),
        )));
        return;
    }

    // Compiler-known or partially constructed constraints may not have a
    // declaration record yet. Their copied normalized metadata is still the
    // authority for dispatch-bearing classification.
    if constraint.predicate_body.is_present()
        && constraint.semantic_roles.is_empty()
        && constraint.establishment_routes.is_empty()
    {
        return;
    }
    terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
        program,
        constraint.semantic_id,
        constraint.name.as_str(),
    )));
}

fn declared_domain_name(
    program: &TypedTrees,
    semantic_id: language_semantics::SemanticDomainId,
    fallback: &str,
) -> String {
    semantic_id
        .is_valid()
        .then(|| program.semantic_domains.name(semantic_id))
        .flatten()
        .unwrap_or(fallback)
        .to_owned()
}
