use crate::expression::lower_expression_handle_from_table;
use crate::lowerer::Lowerer;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_typed_trees as typed;

pub(super) fn lower_type_constraint_node_span_from_table(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraintNode>,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .references
        .constraints(constraints)
    {
        let constraint = match constraint {
            resolved::types::TypeConstraintNode::Named(name) => {
                typed::types::TypeConstraintNode::Named(crate::name::lower_name(name))
            }
            resolved::types::TypeConstraintNode::Domain(domain) => {
                let arguments = source_trees
                    .tables
                    .types
                    .references
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
                let name = crate::name::lower_name(&domain.name);
                typed::types::TypeConstraintNode::Domain(typed::types::DomainConstraint {
                    subject: classify_domain_constraint_subject(name.as_str(), arguments.len()),
                    name,
                    arguments,
                    authored_selection: domain.name.is_source_backed().then_some(
                        typed::types::AuthoredDomainConstraintSelection {
                            source_span: domain.name.source_span(),
                            exposure,
                        },
                    ),
                    ..Default::default()
                })
            }
            resolved::types::TypeConstraintNode::Range { minimum, maximum } => {
                typed::types::TypeConstraintNode::Range {
                    minimum: lower_expression_handle_from_table(
                        &source_trees.tables.bodies.expressions,
                        typed_trees,
                        *minimum,
                    )?,
                    maximum: lower_expression_handle_from_table(
                        &source_trees.tables.bodies.expressions,
                        typed_trees,
                        *maximum,
                    )?,
                }
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

pub(crate) fn lower_type_constraints(
    lowerer: &mut Lowerer,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    lower_type_constraints_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        constraints,
        lowerer.type_reference_exposure,
    )
}

pub(crate) fn lower_type_constraint_node_span_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constraints)
    {
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

/// Lower a COLLECTION's constraint span for re-application to an ELEMENT type,
/// dropping encoding-`Domain` constraints. A declared domain (`[u8] in Utf8`) is
/// bound to the collection's STORAGE type, not to each element, so re-applying it
/// to the element (`u8 in Utf8`) is ill-typed -- there is no `domain` named `Utf8`
/// declared for `u8`. Arithmetic domains (`[i32; N] in Trapping`) DO characterize
/// each element and are kept. Used by the indexed-read hoist to type its temp.
pub(crate) fn lower_element_applicable_constraints(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constraints)
    {
        if matches!(constraint, resolved::types::TypeConstraint::Domain(_)) {
            continue;
        }
        let constraint = lower_type_constraint_node_with_context(
            source_trees,
            typed_trees,
            constraint,
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
        )?;
        typed_trees
            .type_reference_table
            .push_constraint(&mut span, constraint);
    }

    Ok(span)
}

fn lower_type_constraints_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constraints)
    {
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

fn lower_type_constraint_node_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraint: &resolved::types::TypeConstraint,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<typed::types::TypeConstraintNode, Diagnostic> {
    match constraint {
        resolved::types::TypeConstraint::Named(name) => Ok(
            typed::types::TypeConstraintNode::Named(crate::name::lower_name(name)),
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
            let name = crate::name::lower_name(&domain.name);
            Ok(typed::types::TypeConstraintNode::Domain(
                typed::types::DomainConstraint {
                    subject: classify_domain_constraint_subject(name.as_str(), arguments.len()),
                    name,
                    arguments,
                    authored_selection: domain.name.is_source_backed().then_some(
                        typed::types::AuthoredDomainConstraintSelection {
                            source_span: domain.name.source_span(),
                            exposure,
                        },
                    ),
                    ..Default::default()
                },
            ))
        }
        resolved::types::TypeConstraint::Range { minimum, maximum } => {
            Ok(typed::types::TypeConstraintNode::Range {
                minimum: lower_expression_handle_from_table(
                    &source_trees.tables.bodies.expressions,
                    typed_trees,
                    *minimum,
                )?,
                maximum: lower_expression_handle_from_table(
                    &source_trees.tables.bodies.expressions,
                    typed_trees,
                    *maximum,
                )?,
            })
        }
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
        if let Some(permission) = psi_language_semantics::CarryPermission::from_name(name) {
            return DomainConstraintSubject::Carry(permission);
        }
        if let Some(domain) = psi_language_semantics::value_domain::ValueDomain::from_name(name) {
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

#[cfg(test)]
mod tests {
    use super::classify_domain_constraint_subject;
    use psi_language_semantics::CarryPermission;
    use psi_language_semantics::value_domain::ValueDomain;
    use psi_typed_trees::types::{DomainConstraintSubject, OmegaLayoutGrammar};

    #[test]
    fn classifies_only_closed_compiler_domain_shapes() {
        assert_eq!(
            classify_domain_constraint_subject("Carry::AnyCpu", 0),
            DomainConstraintSubject::Carry(CarryPermission::AnyCpu)
        );
        assert_eq!(
            classify_domain_constraint_subject("Finite", 0),
            DomainConstraintSubject::Value(ValueDomain::Finite)
        );
        assert_eq!(
            classify_domain_constraint_subject("OmegaLayout", 1),
            DomainConstraintSubject::OmegaLayout {
                grammar: OmegaLayoutGrammar::Derived,
            }
        );

        for (name, arguments) in [
            ("Carry::AnyCpu", 1),
            ("Finite", 1),
            ("OmegaLayout", 0),
            ("OmegaLayout", 2),
            ("OmegaLayout<Schema>", 0),
            ("Carry::Anywhere", 0),
            ("finite", 0),
        ] {
            assert_eq!(
                classify_domain_constraint_subject(name, arguments),
                DomainConstraintSubject::Declared,
                "{name} with {arguments} argument(s) must remain unclassified"
            );
        }
    }
}
