//! Source parameter custody for memberships carried outside scalar contracts.

use super::*;
use checked_trees::domain::ProofFact;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::{TypeConstraintNode, TypeReferenceNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
    fact: &ProofFact,
) -> Result<(), LoweringError> {
    let ProofFact::Membership(membership) = fact else {
        return unsupported("structural membership custody received another fact kind");
    };
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    if machine.symbol != plan.machine {
        return unsupported("structural membership belongs to another machine");
    }
    let (root, projected) = parameter_root(checked, membership.value).ok_or(
        LoweringError::Unsupported("structural membership has no exact parameter root"),
    )?;
    let mut parameters =
        checked
            .state_parameters(state)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                parameter.symbol == root || (parameter.is_self && root == machine.symbol)
            });
    let (position, source) = parameters.next().ok_or(LoweringError::Unsupported(
        "structural membership is not rooted in its entry signature",
    ))?;
    if parameters.next().is_some() {
        return unsupported("structural membership has ambiguous parameter ownership");
    }
    // Progress profiles belong to the existing progress contract lane, not an
    // outbound structural ABI qualification. Preserve its parameter/field-only
    // source fence without manufacturing a scalar requirement for that lane.
    if checked.domain_definitions().iter().any(|domain| {
        domain.symbol == membership.domain_symbol
            && domain.classification
                == Some(language_semantics::DomainClassification::ProgressProfile)
    }) {
        return Ok(());
    }
    if projected {
        return unsupported("structural qualification membership projects a parameter field");
    }

    let mut matching_domains = Vec::new();
    let mut source_domains = Vec::new();
    let mut visited = Vec::new();
    let mut type_reference = source.type_reference;
    loop {
        if !type_reference.is_valid() || visited.contains(&type_reference) {
            return unsupported("structural membership has an invalid parameter type chain");
        }
        visited.push(type_reference);
        match checked.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in checked.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        if !domain.semantic_id.is_valid() {
                            return unsupported("structural membership has an invalid domain");
                        }
                        source_domains.push(domain.semantic_id);
                        if domain.symbol == membership.domain_symbol
                            && !matching_domains.contains(&domain.semantic_id)
                        {
                            matching_domains.push(domain.semantic_id);
                        }
                    }
                }
                type_reference = *base_type;
            }
            _ => break,
        }
    }
    let domain = match matching_domains.as_slice() {
        [domain] => Some(*domain),
        [] => checked
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
            .filter(|domain| domain.index_arguments.is_empty())
            .map(|domain| domain.semantic_id)
            .filter(|domain| domain.is_valid()),
        _ => None,
    }
    .ok_or(LoweringError::Unsupported(
        "structural membership has no unique declared domain identity",
    ))?;
    if !source_domains.contains(&domain)
        || !plan.structural_parameters.iter().any(|parameter| {
            usize::try_from(parameter.position).ok() == Some(position)
                && parameter.qualifications.contains(&domain)
        })
    {
        return unsupported("structural membership is absent from its parameter qualification");
    }
    Ok(())
}

/// This is the bounded declaration-shaped contract surface, not a general
/// place evaluator: no locals, indexing, calls, or recovered source spellings.
fn parameter_root(
    checked: &CheckedTrees,
    mut expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, bool)> {
    let mut projected = false;
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return None;
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member)
                if member.case_variant.is_none()
                    && checked.symbols.get(member.member_symbol).kind
                        == symbols::SymbolKind::Field =>
            {
                projected = true;
                expression = member.receiver;
            }
            ExpressionNode::Name(path) => {
                let members = checked.expression_table.name_path_members(path.members);
                let symbols = checked
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                if !path.head_symbol.is_valid() || members.is_empty() {
                    return None;
                }
                if members.len() == 1 {
                    return (path.symbol == path.head_symbol)
                        .then_some((path.head_symbol, projected));
                }
                if symbols.len() != members.len()
                    || symbols.first().copied() != Some(path.head_symbol)
                    || symbols.last().copied() != Some(path.symbol)
                    || symbols.iter().skip(1).any(|symbol| {
                        checked.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                    })
                {
                    return None;
                }
                return Some((path.head_symbol, true));
            }
            _ => return None,
        }
    }
}
