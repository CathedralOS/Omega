//! Definition-owned subjects have no executable machine or parameter binding.
//! Rejoin their exact authored occurrence before interpreting self or fields.

use super::{exact_case_membership_meaning, membership_subject_matches_owner};
use crate::places::{exact_data_member_field, unwrapped_type_reference};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField};
use typed_trees::domain::{DomainDefinition, ProofFact};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableBinaryExpression};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Check case membership under the exact domain declaration's carrier binding.
pub fn has_exact_domain_case_membership_meaning(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
    comparison: &TableBinaryExpression,
) -> bool {
    let mut definitions = program
        .domain_definitions()
        .iter()
        .filter(|row| row.symbol == domain.symbol);
    if !domain.symbol.is_valid()
        || program.symbols.get(domain.symbol).kind != SymbolKind::Domain
        || definitions.next() != Some(domain)
        || definitions.next().is_some()
        || !owns_expression(program, domain.facts, expression, comparison)
    {
        return false;
    }
    exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
        membership_subject_matches_owner(program, None, subject, owner, |subject| {
            subject_type(program, DefinitionSubject::Domain(domain), subject, 0)
        })
    })
}

/// Check case membership under the exact data declaration's field bindings.
pub fn has_exact_data_case_membership_meaning(
    program: &TypedTrees,
    data: &DataDefinition,
    expression: ExpressionHandle,
    comparison: &TableBinaryExpression,
) -> bool {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|row| row.symbol == data.symbol);
    if !data.symbol.is_valid()
        || program.symbols.get(data.symbol).kind != SymbolKind::Data
        || definitions.next() != Some(data)
        || definitions.next().is_some()
        || !owns_expression(program, data.where_facts, expression, comparison)
    {
        return false;
    }
    exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
        membership_subject_matches_owner(program, None, subject, owner, |subject| {
            subject_type(program, DefinitionSubject::Data(data), subject, 0)
        })
    })
}

fn owns_expression(
    program: &TypedTrees,
    facts: HandleSpan<ProofFact>,
    expression: ExpressionHandle,
    comparison: &TableBinaryExpression,
) -> bool {
    if !program.expression_table.expression_is_valid(expression)
        || !matches!(program.expression_table.expression(expression),
            ExpressionNode::Binary(retained) if retained == comparison)
    {
        return false;
    }
    facts_contain_expression(program, facts, expression)
}

fn facts_contain_expression(
    program: &TypedTrees,
    facts: HandleSpan<ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    program.proof_facts.span_or_empty(facts).iter().any(|fact| {
        let mut nodes = Vec::new();
        let mut collect =
            |root| crate::expression_types::collect_expression_nodes(program, root, &mut nodes);
        match fact {
            ProofFact::Expression(root) => collect(*root),
            ProofFact::Membership(membership) => collect(membership.value),
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect(*argument);
                }
            }
        }
        nodes.contains(&expression)
    })
}

fn domain_self_has_exact_owner(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
) -> bool {
    // Unresolved self spelling is only the reserved-form discriminator. A
    // sibling declaration's occurrence cannot acquire this carrier when its
    // expression handle is grafted into an otherwise correctly owned predicate.
    facts_contain_expression(program, domain.facts, expression)
        && !program.domain_definitions().iter().any(|other| {
            other.symbol != domain.symbol
                && facts_contain_expression(program, other.facts, expression)
        })
        && !program
            .data_definitions()
            .iter()
            .any(|data| facts_contain_expression(program, data.where_facts, expression))
}

#[derive(Clone, Copy)]
enum DefinitionSubject<'program> {
    Domain(&'program DomainDefinition),
    Data(&'program DataDefinition),
}

fn subject_type(
    program: &TypedTrees,
    owner: DefinitionSubject<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            match owner {
                DefinitionSubject::Domain(domain) => {
                    if name.as_str() != "self"
                        || path.symbol.is_valid()
                        || path.head_symbol.is_valid()
                        || program
                            .expression_table
                            .name_path_member_symbols(path.member_symbols)
                            .iter()
                            .any(|symbol| symbol.is_valid())
                        || !domain_self_has_exact_owner(program, domain, expression)
                    {
                        return None;
                    }
                    domain.target_type
                }
                DefinitionSubject::Data(data) => {
                    if !path.symbol.is_valid()
                        || path.head_symbol != path.symbol
                        || program
                            .expression_table
                            .name_path_member_symbols(path.member_symbols)
                            != [path.symbol]
                    {
                        return None;
                    }
                    checked_field(program, data, path.symbol, name.as_str())?.type_reference
                }
            }
        }
        ExpressionNode::Borrow(borrow) => subject_type(program, owner, borrow.target, depth + 1)?,
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let receiver = unwrapped_type_reference(
                program,
                subject_type(program, owner, member.receiver, depth + 1)?,
            )?;
            // Generic bases do not instantiate their field telescope. The
            // producer must retain a concrete named specialization first.
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(receiver)
            else {
                return None;
            };
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|row| row.symbol == *symbol);
            let data = definitions.next()?;
            if definitions.next().is_some() {
                return None;
            }
            checked_field(program, data, member.member_symbol, member.member.as_str())?
                .type_reference
        }
        _ => return None,
    };
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

fn checked_field<'program>(
    program: &'program TypedTrees,
    data: &'program DataDefinition,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    if !data.symbol.is_valid() || program.symbols.get(data.symbol).kind != SymbolKind::Data {
        return None;
    }
    let field = exact_data_member_field(program, data, symbol, name, None)?;
    let declaration = program.symbols.get(field.symbol);
    (declaration.kind == SymbolKind::Field
        && declaration.parent == data.symbol
        && program.symbols.name(field.symbol) == field.name.as_str())
    .then_some(field)
}

#[cfg(test)]
mod tests;
