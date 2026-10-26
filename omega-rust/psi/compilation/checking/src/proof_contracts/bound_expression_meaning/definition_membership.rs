//! Definition-owned subjects retain their declaration's parameter binding.
//! Rejoin that exact binding before interpreting a subject or its fields.

use super::{exact_case_membership_meaning, membership_subject_matches_owner};
use crate::value_custody::places::{exact_data_member_field, unwrapped_type_reference};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::{DataDefinition, DataField};
use typed_trees::domain::{DomainDefinition, ProofFact};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableBinaryExpression};
use typed_trees::types::TypeReferenceHandle;

/// Check case membership under the exact domain declaration's carrier binding.
pub fn has_exact_domain_case_membership_meaning(
    program: &dyn crate::CheckingInput,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
    comparison: &TableBinaryExpression,
) -> bool {
    if !has_exact_domain_definition(program, domain)
        || !owns_expression(program, domain.facts, expression, comparison)
    {
        return false;
    }
    exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
        membership_subject_matches_owner(program, subject, owner, |subject| {
            subject_type(program, DefinitionSubject::Domain(domain), subject, 0)
        })
    })
}

/// Check case membership under the exact data declaration's field bindings.
pub fn has_exact_data_case_membership_meaning(
    program: &dyn crate::CheckingInput,
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
        membership_subject_matches_owner(program, subject, owner, |subject| {
            subject_type(program, DefinitionSubject::Data(data), subject, 0)
        })
    })
}

fn owns_expression(
    program: &dyn crate::CheckingInput,
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

pub(super) fn facts_contain_expression(
    program: &dyn crate::CheckingInput,
    facts: HandleSpan<ProofFact>,
    expression: ExpressionHandle,
) -> bool {
    program.proof_facts.span_or_empty(facts).iter().any(|fact| {
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut collect = |root| {
            crate::expression_traversal::collect_expression_nodes(
                program, root, &mut nodes, &mut seen,
            )
        };
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

#[derive(Clone, Copy)]
enum DefinitionSubject<'program> {
    Domain(&'program DomainDefinition),
    Data(&'program DataDefinition),
}

/// The domain subject has meaning only under its declaration-owned binding.
/// This is a carrier lookup, not evidence that any runtime value is a member.
pub fn exact_domain_subject_type(
    program: &dyn crate::CheckingInput,
    domain: &DomainDefinition,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !has_exact_domain_definition(program, domain)
        || !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        )
    {
        return None;
    }
    subject_type(program, DefinitionSubject::Domain(domain), expression, 0)
}

pub(super) fn has_exact_domain_definition(
    program: &dyn crate::CheckingInput,
    domain: &DomainDefinition,
) -> bool {
    let mut definitions = program
        .domain_definitions()
        .iter()
        .filter(|candidate| candidate.symbol == domain.symbol);
    domain.symbol.is_valid()
        && program.symbols.get(domain.symbol).kind == SymbolKind::Domain
        && definitions.next() == Some(domain)
        && definitions.next().is_none()
}

fn subject_type(
    program: &dyn crate::CheckingInput,
    owner: DefinitionSubject<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    let reference = match node {
        ExpressionNode::Name(path) => {
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            match owner {
                DefinitionSubject::Domain(domain) => {
                    if !typed_trees::domain::domain_subject_reference(program, domain, expression) {
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
            let symbol = &program
                .type_reference_table
                .type_reference(receiver)
                .as_named_symbol()?;
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
    program: &'program dyn crate::CheckingInput,
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
