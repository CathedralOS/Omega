//! Rejoin an explicit scalar qualification to its authored selection and evidence.
//!
//! Equal payload carriers do not make two semantic domains interchangeable.
//! The checked node must preserve the cast occurrence and its normalized result;
//! the vacuous-use row alone cannot classify a routed or predicate-bearing
//! declaration as empty. Replay checks both owners, including expanded aliases
//! and canonical indexed instances. The scalar qualification owner supplies the
//! same declaration replay to the closure's canonical Terminal catalog.

use super::*;
use crate::scalar_qualifications::declared_atoms;
use checked_trees::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};

pub(super) fn operand(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    source: ExpressionHandle,
    operand: CheckedScalarComputationHandle,
    result_type: TypeReferenceHandle,
    primitive: PrimitiveType,
) -> Result<ExpressionHandle, LoweringError> {
    let ExpressionNode::Cast(cast) = checked.expression_table.expression(source) else {
        return unsupported("scalar qualification lost its authored cast");
    };
    let table = &checked.type_reference_table;
    let nodes = &checked.facts.values.scalar_computations.nodes;
    if cast.form.is_recast()
        || cast.semantic_domain.is_empty()
        || cast.domain != ArithmeticDomain::Exact
        || !table.contains_type_reference(result_type)
        || result_type != cast.result_type
        || !matches!(
            table.type_reference(cast.target_type),
            TypeReferenceNode::Named { .. }
        )
        || checked.primitive_type_reference(cast.target_type) != Some(primitive)
        || !nodes.is_valid(operand)
        || nodes.get(operand).primitive_type != primitive
    {
        return unsupported("scalar qualification changed its cast, result, or operand carrier");
    }
    let mut selections = checked
        .expression_table
        .authored_selection_occurrences(source)
        .filter_map(|occurrence| checked.authored_declaration_selections().get(occurrence))
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership);
    let selected = selections.next().ok_or(LoweringError::Unsupported(
        "scalar qualification has no authored domain selection",
    ))?;
    if selections.next().is_some()
        || !matches!(selected.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == cast.semantic_domain_symbol)
    {
        return unsupported("scalar qualification changed its selected declaration");
    }
    let mut uses = checked
        .facts
        .qualifications
        .vacuous_uses
        .iter()
        .filter(|usage| {
            usage.machine == machine
                && usage.state == state
                && usage.statement_index == statement
                && usage.expression == source
        });
    let usage = uses.next().ok_or(LoweringError::Unsupported(
        "scalar qualification has no exact checked membership use",
    ))?;
    if uses.next().is_some()
        || usage.domain != cast.semantic_domain_symbol
        || usage.semantic_domain != cast.semantic_domain_id
    {
        return unsupported("scalar qualification substituted its checked membership use");
    }
    let arguments = table.type_reference_handles(cast.semantic_domain_arguments);
    if arguments.len() != cast.semantic_domain_arguments.len() {
        return unsupported("scalar qualification has stale domain indices");
    }
    let mut expected = Vec::new();
    declared_atoms(
        checked,
        cast.semantic_domain_symbol,
        arguments,
        cast.semantic_domain_id,
        primitive,
        &mut Vec::new(),
        &mut expected,
    )?;
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = table.type_reference(result_type)
    else {
        return unsupported("scalar qualification erased its normalized result");
    };
    if *base_type != cast.target_type {
        return unsupported("scalar qualification changed its normalized result carrier");
    }
    let constraints = table
        .constraint_span(*constraints)
        .ok_or(LoweringError::Unsupported(
            "scalar qualification has stale normalized constraints",
        ))?;
    let mut actual = Vec::new();
    for constraint in constraints {
        let TypeConstraintNode::Domain(domain) = constraint else {
            return unsupported("scalar qualification introduced an unrelated result constraint");
        };
        if domain.subject != DomainConstraintSubject::Declared
            || domain.predicate_body.is_present()
            || !domain.establishment_routes.is_empty()
        {
            return unsupported("scalar qualification result needs non-vacuous evidence");
        }
        let mut atoms = Vec::new();
        declared_atoms(
            checked,
            domain.symbol,
            &domain.arguments,
            domain.semantic_id,
            primitive,
            &mut Vec::new(),
            &mut atoms,
        )?;
        if atoms != [(domain.symbol, domain.semantic_id)] {
            return unsupported("scalar qualification result retains an unexpanded alias");
        }
        let declaration = checked
            .domain_definitions()
            .iter()
            .find(|declaration| declaration.symbol == domain.symbol)
            .ok_or(LoweringError::Unsupported(
                "scalar qualification lost its domain definition",
            ))?;
        let roles = language_semantics::DomainSemanticRoles {
            denotation_dimension: declaration
                .semantic_roles
                .denotation_dimension
                .map(|_| domain.semantic_id),
            arithmetic_policy: declaration
                .semantic_roles
                .arithmetic_policy
                .map(|_| domain.semantic_id),
        };
        if domain.classification != declaration.classification || domain.semantic_roles != roles {
            return unsupported("scalar qualification changed its declared semantic roles");
        }
        actual.extend(atoms);
    }
    if actual.is_empty()
        || actual.iter().any(|atom| !expected.contains(atom))
        || expected.iter().any(|atom| !actual.contains(atom))
    {
        return unsupported("scalar qualification substituted its normalized domain instance");
    }
    Ok(cast.value)
}

#[cfg(test)]
mod tests;
