//! Validation for the compiler-owned `Content<A>` qualification projection.
//!
//! A projection is selected by an exact domain owner, not by linearity, field
//! names, or operation names. This module establishes the ownership/uniqueness
//! firewall, the first closed-fragment gate, and canonical checked plans whose
//! fingerprints exclude arena-local symbols. Runtime-scalar embedding and the
//! conservation/backing consumers remain later P1c rungs.

use crate::type_references::type_references_match;
use omega_core::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentFieldSegment,
    ContentProjectionExpression, ContentProjectionPlan, ContentScalarExpression,
    projection_fingerprint,
};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::DomainDefinition;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_typed_trees::trait_definition::TraitDefinition;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy)]
struct ProjectionSubject {
    symbol: SymbolHandle,
    carrier: TypeReferenceHandle,
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
            let candidates = projection_domain_candidates(program, machine, subject.carrier);
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
                        program.display_type_reference(subject.carrier)
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
                normalize_projection_expression(program, subject, &algebra, expression).is_none()
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

/// Materialize the validated, compiler-owned content projection plans. The
/// checked-tree builder calls this only after program validation succeeds;
/// invalid or incomplete candidates are therefore omitted rather than
/// represented as partial facts.
pub fn build_content_projection_plans(program: &TypedTrees) -> Vec<ContentProjectionPlan> {
    program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_trait_conformances(machine)
                .iter()
                .filter_map(move |conformance| {
                    let content = program
                        .traits()
                        .iter()
                        .find(|candidate| candidate.symbol == conformance.symbol)
                        .filter(|candidate| is_content_projection_trait(program, candidate))?;
                    if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project")
                    {
                        return None;
                    }
                    let subject = projection_subject(program, machine)?;
                    let candidates =
                        projection_domain_candidates(program, machine, subject.carrier);
                    let [domain] = candidates.as_slice() else {
                        return None;
                    };
                    if domain.alias.is_some() || content.is_boundary {
                        return None;
                    }
                    let algebra = selected_content_algebra(program, conformance)?;
                    let expression = normalize_projection_expression(
                        program,
                        subject,
                        &algebra,
                        projection_result_expression(program, machine)?,
                    )?;
                    let fingerprint = projection_fingerprint(&algebra, &expression);
                    Some(ContentProjectionPlan {
                        domain: domain.symbol,
                        semantic_domain: domain.semantic_id,
                        carrier_identity: program
                            .normalized_type_identity(subject.carrier)
                            .into_string(),
                        machine: machine.symbol,
                        algebra,
                        expression,
                        fingerprint,
                    })
                })
        })
        .collect()
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
                normalize_projection_expression(program, subject, &algebra, expression).is_some()
            })
        })
}

fn selected_content_algebra(
    program: &TypedTrees,
    conformance: &omega_typed_trees::machine::TraitConformance,
) -> Option<ContentAlgebraIdentity> {
    let [argument] = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        return None;
    };
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = program.type_reference_table.type_reference(*argument)
    else {
        return None;
    };
    let [identity] = program
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        return None;
    };
    let identity = program.normalized_type_identity(*identity).into_string();
    match base_name.as_str().rsplit("::").next() {
        Some("Interval") => Some(ContentAlgebraIdentity::Interval {
            coordinate_space: identity,
        }),
        Some("CountedQuantity") => Some(ContentAlgebraIdentity::CountedQuantity { unit: identity }),
        _ => None,
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

fn normalize_projection_expression(
    program: &TypedTrees,
    subject: ProjectionSubject,
    algebra: &ContentAlgebraIdentity,
    expression: ExpressionHandle,
) -> Option<ContentProjectionExpression> {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return None;
    };
    let expected_name = match algebra {
        ContentAlgebraIdentity::Interval { .. } => "Interval",
        ContentAlgebraIdentity::CountedQuantity { .. } => "CountedQuantity",
    };
    let literal_leaf = literal
        .type_name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(literal.type_name.as_str());
    if literal_leaf != expected_name || literal.case_name.is_some() {
        return None;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    match algebra {
        ContentAlgebraIdentity::Interval { .. } => {
            let [start, end] = ["start", "end"].map(|name| {
                fields
                    .iter()
                    .find(|field| field.name.as_str() == name)
                    .and_then(|field| normalize_projection_scalar(program, subject, field.value))
            });
            (fields.len() == 2).then_some(ContentProjectionExpression::Interval {
                start: start?,
                end: end?,
            })
        }
        ContentAlgebraIdentity::CountedQuantity { .. } => {
            let magnitude = fields
                .iter()
                .find(|field| field.name.as_str() == "magnitude")
                .and_then(|field| normalize_projection_scalar(program, subject, field.value))?;
            (fields.len() == 1)
                .then_some(ContentProjectionExpression::CountedQuantity { magnitude })
        }
    }
}

fn normalize_projection_scalar(
    program: &TypedTrees,
    subject: ProjectionSubject,
    expression: ExpressionHandle,
) -> Option<ContentScalarExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => {
            let value = value.value_bignum()?;
            (!value.is_negative()).then(|| ContentScalarExpression::Natural(value.to_string()))
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            matches!(members, [first, second] if first.as_str() == "Nat" && second.as_str() == "Zero")
                .then(|| ContentScalarExpression::Natural("0".to_owned()))
        }
        ExpressionNode::Member(_) => normalize_subject_field(program, subject, expression)
            .map(|(path, _)| ContentScalarExpression::SubjectField(path)),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            Some(ContentScalarExpression::Arithmetic {
                operator: match binary.operator {
                    BinaryOperator::Add => ContentArithmeticOperator::Add,
                    BinaryOperator::Subtract => ContentArithmeticOperator::Subtract,
                    BinaryOperator::Multiply => ContentArithmeticOperator::Multiply,
                    _ => unreachable!("guarded closed arithmetic operator"),
                },
                left: Box::new(normalize_projection_scalar(program, subject, binary.left)?),
                right: Box::new(normalize_projection_scalar(program, subject, binary.right)?),
            })
        }
        ExpressionNode::StructLiteral(literal)
            if literal.type_name.as_str().rsplit("::").next() == Some("Nat") =>
        {
            match literal.case_name.as_ref().map(|name| name.as_str()) {
                Some("Zero")
                    if program
                        .expression_table
                        .struct_fields(literal.fields)
                        .is_empty() =>
                {
                    Some(ContentScalarExpression::Natural("0".to_owned()))
                }
                Some("Succ") => {
                    let fields = program.expression_table.struct_fields(literal.fields);
                    let [field] = fields else {
                        return None;
                    };
                    if field.name.as_str() != "prev" {
                        return None;
                    }
                    Some(ContentScalarExpression::Successor(Box::new(
                        normalize_projection_scalar(program, subject, field.value)?,
                    )))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn normalize_subject_field(
    program: &TypedTrees,
    subject: ProjectionSubject,
    expression: ExpressionHandle,
) -> Option<(Vec<ContentFieldSegment>, TypeReferenceHandle)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol == subject.symbol => {
            Some((Vec::new(), subject.carrier))
        }
        ExpressionNode::Member(member) => {
            let (mut path, receiver_type) =
                normalize_subject_field(program, subject, member.receiver)?;
            let field = data_field(program, receiver_type, member.member.as_str())?;
            path.push(ContentFieldSegment {
                symbol: field.symbol,
                name: member.member.as_str().to_owned(),
            });
            Some((path, field.type_reference))
        }
        _ => None,
    }
}

fn data_field<'program>(
    program: &'program TypedTrees,
    mut receiver_type: TypeReferenceHandle,
    name: &str,
) -> Option<&'program omega_typed_trees::data::DataField> {
    loop {
        receiver_type = match program.type_reference_table.type_reference(receiver_type) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => *referee,
            _ => break,
        };
    }
    let (symbol, fallback) = match program.type_reference_table.type_reference(receiver_type) {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str()),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name.as_str()),
        _ => return None,
    };
    let definition = program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name.as_str() == fallback
    })?;
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                Some(field)
            }
            _ => None,
        })
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

fn projection_subject(program: &TypedTrees, machine: &Machine) -> Option<ProjectionSubject> {
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
        } => Some(ProjectionSubject {
            symbol: parameter.symbol,
            carrier: unconstrained(program, *referee),
        }),
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
