use crate::expressions::expression::lower_expression_handle_from_table_with_symbol_substitution;
use crate::tables::type_constraints;
use crate::tables::type_references;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees as typed;

// Bound expressions may themselves select structural type arguments. Both
// constraint representations must retain the resolved program for that nested
// lowering; an expression table alone cannot resolve its child type handles.
pub(super) fn lower_type_constraint_node_span_from_table(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraintNode>,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in type_references(source_trees).constraints(constraints) {
        let constraint = match constraint {
            resolved::types::TypeConstraintNode::Named(name) => {
                typed::types::TypeConstraintNode::Named(crate::lowerer::name::lower_name(name))
            }
            resolved::types::TypeConstraintNode::Domain(domain) => {
                let arguments = type_references(source_trees)
                    .type_reference_handles(domain.arguments)
                    .iter()
                    .map(|argument| {
                        super::table::lower_type_reference_handle_from_table_with_context(
                            source_trees,
                            typed_trees,
                            *argument,
                            exposure,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                lower_domain_constraint_node(&domain.name, arguments, exposure)
            }
            resolved::types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                lower_range_constraint(source_trees, typed_trees, minimum, maximum, *end_inclusive)?
            }
            resolved::types::TypeConstraintNode::ArithmeticDomain(domain) => {
                typed::types::TypeConstraintNode::ArithmeticDomain(*domain)
            }
        };
        typed_trees
            .type_reference_table
            .push_constraint(&mut span, constraint);
    }

    Ok(span)
}

pub(crate) fn lower_type_constraint_node_span_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let exposure = exposure.into();
    let mut span = HandleSpan::empty();

    for constraint in type_constraints(source_trees).span_or_empty(constraints) {
        let constraint = lower_type_constraint_node_with_context(
            source_trees,
            typed_trees,
            constraint,
            exposure,
        )?;
        typed_trees
            .type_reference_table
            .push_constraint(&mut span, constraint);
    }

    Ok(span)
}

/// The typed `Domain` constraint leaf shared by the direct and table-backed
/// constraint lowerings — both resolved enums carry the same fields here.
/// Lower an inclusive-or-exclusive integer range constraint from authored
/// expression handles.
fn lower_range_constraint(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    minimum: &resolved::expression::ExpressionHandle,
    maximum: &resolved::expression::ExpressionHandle,
    end_inclusive: bool,
) -> Result<typed::types::TypeConstraintNode, Diagnostic> {
    Ok(typed::types::TypeConstraintNode::Range {
        end_inclusive,
        minimum: lower_expression_handle_from_table_with_symbol_substitution(
            Some(source_trees),
            &source_trees.tables.bodies.expressions,
            typed_trees,
            *minimum,
            None,
        )?,
        maximum: lower_expression_handle_from_table_with_symbol_substitution(
            Some(source_trees),
            &source_trees.tables.bodies.expressions,
            typed_trees,
            *maximum,
            None,
        )?,
    })
}

fn lower_domain_constraint_node(
    domain_name: &resolved::name::DiagnosticName,
    arguments: Vec<typed::types::TypeReferenceHandle>,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> typed::types::TypeConstraintNode {
    let name = crate::lowerer::name::lower_name(domain_name);
    typed::types::TypeConstraintNode::Domain(typed::types::DomainConstraint {
        subject: classify_domain_constraint_subject(name.as_str(), arguments.len()),
        name,
        arguments,
        authored_selection: exposure
            .filter(|_| domain_name.is_source_backed())
            .map(|exposure| typed::types::AuthoredDomainConstraintSelection {
                source_span: domain_name.source_span(),
                exposure,
            }),
        ..Default::default()
    })
}

fn lower_type_constraint_node_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraint: &resolved::types::TypeConstraint,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeConstraintNode, Diagnostic> {
    match constraint {
        resolved::types::TypeConstraint::Named(name) => Ok(
            typed::types::TypeConstraintNode::Named(crate::lowerer::name::lower_name(name)),
        ),
        resolved::types::TypeConstraint::Domain(domain) => {
            let arguments = source_trees
                .tables
                .declarations
                .child_type_references
                .span_or_empty(domain.arguments)
                .iter()
                .map(|argument| {
                    crate::type_reference::lower_type_reference_into_trees_with_exposure(
                        source_trees,
                        typed_trees,
                        argument,
                        exposure,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(lower_domain_constraint_node(
                &domain.name,
                arguments,
                exposure,
            ))
        }
        resolved::types::TypeConstraint::Range {
            minimum,
            maximum,
            end_inclusive,
        } => lower_range_constraint(source_trees, typed_trees, minimum, maximum, *end_inclusive),
        resolved::types::TypeConstraint::ArithmeticDomain(domain) => {
            Ok(typed::types::TypeConstraintNode::ArithmeticDomain(*domain))
        }
    }
}

pub(crate) fn classify_domain_constraint_subject(
    name: &str,
    argument_count: usize,
) -> typed::types::DomainConstraintSubject {
    use typed::types::{DomainConstraintSubject, OmegaLayoutGrammar};

    if argument_count == 0 {
        if let Some(permission) = language_semantics::CarryPermission::from_name(name) {
            return DomainConstraintSubject::Carry(permission);
        }
        if let Some(domain) = language_semantics::value_domain::ValueDomain::from_name(name) {
            return DomainConstraintSubject::Value(domain);
        }
    }
    if name == "OmegaLayout" && argument_count == 1 {
        return DomainConstraintSubject::OmegaLayout {
            grammar: OmegaLayoutGrammar::Derived,
        };
    }
    DomainConstraintSubject::Declared
}
