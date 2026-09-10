//! Declaration-backed scalar tags whose membership has no predicate or route.
//!
//! Source flow and scalar graph custody share this boundary. Membership facts
//! currently name declarations, not indexed instances; accepting a family here
//! would silently equate distinct applications. The full qualified Terminal
//! signature carries these tags, so their exact parameter requirements need no
//! duplicate executable Boolean predicate. Other state contracts still need
//! their own graph evidence and must not disappear during this classification.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::{DomainDefinition, ProofFact};
use typed_trees::expression::ExpressionNode;
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::{
    DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};

pub fn scalar_type_index_free_tags(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    let Some(primitive) = program.primitive_type_reference(reference) else {
        return Vec::new();
    };
    let mut domains = Vec::new();
    let mut seen = Vec::new();
    while !seen.contains(&reference) {
        seen.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            break;
        };
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let TypeConstraintNode::Domain(domain) = constraint
                && domain.subject == DomainConstraintSubject::Declared
                && domain.arguments.is_empty()
                && program.domain_definitions().iter().any(|declaration| {
                    declaration.symbol == domain.symbol
                        && declaration.semantic_id == domain.semantic_id
                        && index_free_tag(program, declaration, primitive, &mut Vec::new())
                })
                && !domains.contains(&domain.symbol)
            {
                domains.push(domain.symbol);
            }
        }
        reference = *base_type;
    }
    domains
}

pub fn scalar_state_contracts_are_qualifications(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    let Some(contracts) = program.signature_contracts.span(state.contracts) else {
        return false;
    };
    contracts.iter().all(|contract| {
        if contract.kind != SignatureContractKind::Requires || contract.binding.is_some() {
            return false;
        }
        let Some(facts) = program.proof_facts.span(contract.facts) else {
            return false;
        };
        !facts.is_empty()
            && facts.iter().all(|fact| {
                let ProofFact::Membership(membership) = fact else {
                    return false;
                };
                if !program
                    .expression_table
                    .expression_is_valid(membership.value)
                {
                    return false;
                }
                let ExpressionNode::Name(path) =
                    program.expression_table.expression(membership.value)
                else {
                    return false;
                };
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.symbol.is_valid()
                        && parameter.symbol == path.symbol
                        && !parameter.is_self
                        && !parameter.is_mutable
                        && scalar_type_index_free_tags(program, parameter.type_reference)
                            .contains(&membership.domain_symbol)
                })
            })
    })
}

fn index_free_tag(
    program: &TypedTrees,
    declaration: &DomainDefinition,
    primitive: PrimitiveType,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if !declaration.symbol.is_valid()
        || !declaration.semantic_id.is_valid()
        || active.contains(&declaration.symbol)
        || !typed_trees::domain::index_parameters(program, declaration).is_empty()
        || declaration.predicate_body.is_present()
        || !declaration.facts.is_empty()
        || !declaration.establishment_routes.is_empty()
        || declaration.semantic_roles != Default::default()
        || (!typed_trees::domain::has_generic_carrier(program, declaration)
            && program.primitive_type_reference(declaration.target_type) != Some(primitive))
    {
        return false;
    }
    let Some(alias) = &declaration.alias else {
        return true;
    };
    active.push(declaration.symbol);
    let valid = !alias.constituents.is_empty()
        && alias.constituents.iter().all(|constituent| {
            program
                .domain_definitions()
                .iter()
                .find(|candidate| candidate.symbol == constituent.domain_symbol)
                .is_some_and(|candidate| index_free_tag(program, candidate, primitive, active))
        });
    active.pop();
    valid
}
