//! Validation for the compiler-owned `Content<A>` qualification projection.
//!
//! A projection is selected by an exact domain owner, not by linearity, field
//! names, or operation names. This module establishes the ownership/uniqueness
//! firewall and the first closed-fragment gate; canonical expression retention
//! and algebra fingerprinting are later P1c rungs.

use crate::type_references::type_references_match;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::DomainDefinition;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_typed_trees::trait_definition::TraitDefinition;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentAlgebraKind {
    Interval,
    CountedQuantity,
}

pub(crate) fn validate_content_projection_conformances(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut published: Vec<(
        omega_core::semantics::SemanticDomainId,
        SymbolHandle,
        String,
    )> = Vec::new();

    for machine in program.machines() {
        for conformance in program.machine_trait_conformances(machine) {
            let Some(content) = program
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == conformance.symbol)
                .filter(|candidate| is_content_projection_trait(program, candidate))
            else {
                continue;
            };

            if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project") {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` must explicitly satisfy `{}::project`",
                    machine.name, content.name
                )));
                continue;
            }

            let Some(subject) = projection_subject(program, machine) else {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` must take exactly one shared borrowed subject",
                    machine.name
                )));
                continue;
            };
            let candidates = projection_domain_candidates(program, machine, subject);
            let domain = match candidates.as_slice() {
                [domain] => *domain,
                [] => {
                    diagnostics.push(Diagnostic::error(format!(
                        "content projection machine `{}` is not attached to the exact qualification it projects; use `<Qualification>::content` with a subject matching that domain's carrier",
                        machine.name
                    )));
                    continue;
                }
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                        "content projection machine `{}` ambiguously names more than one exact qualification for subject `{}`",
                        machine.name,
                        program.display_type_reference(subject)
                    )));
                    continue;
                }
            };

            if domain.alias.is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` targets transparent alias `{}`; publish content on one exact atomic qualification instead",
                    machine.name, domain.name
                )));
                continue;
            }
            if !matches!(
                machine.supply_mode,
                omega_core::semantics::MachineSupplyMode::CheckedBody
            ) || program.machine_states(machine).first().is_none_or(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` for `{}` must be a bodyful checked machine",
                    machine.name, domain.name
                )));
                continue;
            }

            let Some(algebra) = selected_content_algebra(program, conformance) else {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` must select compiler-owned `Interval<CoordinateSpace>` or `CountedQuantity<Unit>`, not `{}`",
                    machine.name,
                    program
                        .type_reference_table
                        .type_reference_handles(conformance.arguments)
                        .first()
                        .map(|argument| program.display_type_reference(*argument))
                        .unwrap_or_else(|| "<missing>".to_owned())
                )));
                continue;
            };
            if projection_result_expression(program, machine).is_none_or(|expression| {
                !projection_expression_is_closed(program, subject, algebra, expression)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` is outside the closed projection fragment; use one selected algebra constructor over subject field reads, proof-natural constructors, and closed arithmetic only",
                    machine.name
                )));
                continue;
            }

            if let Some((_, _, first)) = published.iter().find(|(semantic_id, symbol, _)| {
                (domain.semantic_id.is_valid() && *semantic_id == domain.semantic_id)
                    || (!domain.semantic_id.is_valid() && *symbol == domain.symbol)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "exact qualification `{}` publishes more than one `Content<A>` projection (`{first}` and `{}`); one qualification has one owner-unique projection identity",
                    domain.name, machine.name
                )));
                continue;
            }
            published.push((domain.semantic_id, domain.symbol, machine.name.to_string()));
        }
    }
}

pub(crate) fn is_content_projection_machine(program: &TypedTrees, machine: &Machine) -> bool {
    program
        .machine_trait_conformances(machine)
        .iter()
        .any(|conformance| {
            if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project")
                || !program
                    .traits()
                    .iter()
                    .find(|candidate| candidate.symbol == conformance.symbol)
                    .is_some_and(|candidate| is_content_projection_trait(program, candidate))
            {
                return false;
            }
            let Some(subject) = projection_subject(program, machine) else {
                return false;
            };
            let Some(algebra) = selected_content_algebra(program, conformance) else {
                return false;
            };
            projection_result_expression(program, machine).is_some_and(|expression| {
                projection_expression_is_closed(program, subject, algebra, expression)
            })
        })
}

fn selected_content_algebra(
    program: &TypedTrees,
    conformance: &omega_typed_trees::machine::TraitConformance,
) -> Option<ContentAlgebraKind> {
    let [argument] = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        return None;
    };
    let label = match program.type_reference_table.type_reference(*argument) {
        TypeReferenceNode::Generic { base_name, .. } => base_name.as_str(),
        TypeReferenceNode::Named { name, .. } => name.as_str(),
        _ => return None,
    };
    let leaf = label.rsplit("::").next().unwrap_or(label);
    if leaf.starts_with("Interval<") || leaf == "Interval" {
        Some(ContentAlgebraKind::Interval)
    } else if leaf.starts_with("CountedQuantity<") || leaf == "CountedQuantity" {
        Some(ContentAlgebraKind::CountedQuantity)
    } else {
        None
    }
}

fn projection_result_expression(
    program: &TypedTrees,
    machine: &Machine,
) -> Option<ExpressionHandle> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [statement] = program.statement_table.statements(state.statement_nodes) else {
        return None;
    };
    match statement {
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid() =>
        {
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) => Some(*expression),
                _ => None,
            }
        }
        _ => None,
    }
}

fn projection_expression_is_closed(
    program: &TypedTrees,
    subject: TypeReferenceHandle,
    algebra: ContentAlgebraKind,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return false;
    };
    let expected_name = match algebra {
        ContentAlgebraKind::Interval => "Interval",
        ContentAlgebraKind::CountedQuantity => "CountedQuantity",
    };
    let literal_leaf = literal
        .type_name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(literal.type_name.as_str());
    if literal_leaf != expected_name || literal.case_name.is_some() {
        return false;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let expected_fields: &[&str] = match algebra {
        ContentAlgebraKind::Interval => &["start", "end"],
        ContentAlgebraKind::CountedQuantity => &["magnitude"],
    };
    fields.len() == expected_fields.len()
        && expected_fields.iter().all(|expected| {
            fields.iter().any(|field| {
                field.name.as_str() == *expected
                    && projection_scalar_is_closed(program, subject, field.value)
            })
        })
}

fn projection_scalar_is_closed(
    program: &TypedTrees,
    subject: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            matches!(members, [first, second] if first.as_str() == "Nat" && second.as_str() == "Zero")
        }
        ExpressionNode::Member(member) => {
            projection_subject_field_root(program, member.receiver, subject)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            projection_scalar_is_closed(program, subject, binary.left)
                && projection_scalar_is_closed(program, subject, binary.right)
        }
        ExpressionNode::StructLiteral(literal)
            if literal.type_name.as_str().rsplit("::").next() == Some("Nat") =>
        {
            match literal.case_name.as_ref().map(|name| name.as_str()) {
                Some("Zero") => program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty(),
                Some("Succ") => {
                    let fields = program.expression_table.struct_fields(literal.fields);
                    matches!(fields, [field] if field.name.as_str() == "prev" && projection_scalar_is_closed(program, subject, field.value))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn projection_subject_field_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    subject: TypeReferenceHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program.machines().iter().any(|machine| {
            program.machine_states(machine).iter().any(|state| {
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.symbol == path.symbol
                        && projection_subject_type_matches(
                            program,
                            parameter.type_reference,
                            subject,
                        )
                })
            })
        }),
        ExpressionNode::Member(member) => {
            projection_subject_field_root(program, member.receiver, subject)
        }
        _ => false,
    }
}

fn projection_subject_type_matches(
    program: &TypedTrees,
    parameter: TypeReferenceHandle,
    subject: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(parameter) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_references_match(program, unconstrained(program, *referee), subject)
        }
        _ => false,
    }
}

fn is_content_projection_trait(program: &TypedTrees, candidate: &TraitDefinition) -> bool {
    candidate
        .name
        .as_str()
        .rsplit("::")
        .next()
        .is_some_and(|name| name == "Content")
        && !candidate.is_boundary
        && program.trait_type_parameters(candidate).len() == 1
        && matches!(program.trait_machine_signatures(candidate), [requirement] if requirement.name.as_str() == "project")
}

fn projection_subject(program: &TypedTrees, machine: &Machine) -> Option<TypeReferenceHandle> {
    let state = program.machine_states(machine).first()?;
    let [parameter] = program.state_parameters(state) else {
        return None;
    };
    if parameter.is_self || parameter.is_mutable {
        return None;
    }
    match program
        .type_reference_table
        .type_reference(parameter.type_reference)
    {
        TypeReferenceNode::Reference {
            referee,
            is_mutable: false,
            ..
        } => Some(unconstrained(program, *referee)),
        _ => None,
    }
}

fn unconstrained(program: &TypedTrees, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}

fn projection_domain_candidates<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    subject: TypeReferenceHandle,
) -> Vec<&'program DomainDefinition> {
    let Some(owner) = machine.attached_data.as_ref().map(|name| name.as_str()) else {
        return Vec::new();
    };
    let owner_leaf = owner.rsplit("::").next().unwrap_or(owner);
    program
        .domain_definitions()
        .iter()
        .filter(|domain| {
            let name = domain.name.as_str();
            let leaf = name.rsplit("::").next().unwrap_or(name);
            (name == owner || leaf == owner_leaf)
                && type_references_match(program, subject, domain.target_type)
        })
        .collect()
}
