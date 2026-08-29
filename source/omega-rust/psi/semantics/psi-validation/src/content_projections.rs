//! Validation for the compiler-owned `Content<A>` qualification projection.
//!
//! A projection is selected by an exact domain owner, not by linearity, field
//! names, or operation names. This module establishes the ownership/uniqueness
//! firewall, the first closed-fragment gate, and canonical checked plans whose
//! fingerprints exclude arena-local symbols. Runtime-scalar embedding and the
//! conservation/backing consumers remain later P1c rungs.

use crate::type_references::type_references_match;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentFieldSegment,
    ContentIntervalExpression, ContentProjectionExpression, ContentProjectionPlan,
    ContentScalarExpression, projection_report_fingerprint,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::DomainDefinition;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy)]
struct ProjectionSubject {
    symbol: SymbolHandle,
    qualified: TypeReferenceHandle,
    carrier: TypeReferenceHandle,
}

#[derive(Debug, Clone, Copy)]
struct ProjectionDomainCandidate<'program> {
    definition: &'program DomainDefinition,
    semantic_domain: psi_language_semantics::SemanticDomainId,
}

pub(crate) fn validate_content_projection_conformances(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut published: Vec<(
        psi_language_semantics::SemanticDomainId,
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
            let candidate = match candidates.as_slice() {
                [candidate] => *candidate,
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
            let domain = candidate.definition;

            if program.type_multiplicity(subject.carrier)
                != psi_language_semantics::Multiplicity::Linear
            {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection `{}` targets `{}` whose carrier `{}` is not linear; fine-grained content accounting belongs to owned linear claims",
                    machine.name,
                    domain.name,
                    program.display_type_reference(subject.carrier),
                )));
                continue;
            }

            if domain.alias.is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "content projection machine `{}` targets transparent alias `{}`; publish content on one exact atomic qualification instead",
                    machine.name, domain.name
                )));
                continue;
            }
            if !matches!(
                machine.supply_mode,
                psi_language_semantics::MachineSupplyMode::CheckedBody
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
                    "content projection `{}` must select compiler-owned `IntervalSet<CoordinateSpace>` or `CountedQuantity<Unit>`, not `{}`",
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
                (candidate.semantic_domain.is_valid() && *semantic_id == candidate.semantic_domain)
                    || (!candidate.semantic_domain.is_valid() && *symbol == domain.symbol)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "exact qualification `{}` publishes more than one `Content<A>` projection (`{first}` and `{}`); one qualification has one owner-unique projection identity",
                    domain.name, machine.name
                )));
                continue;
            }
            published.push((
                candidate.semantic_domain,
                domain.symbol,
                machine.name.to_string(),
            ));
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
                    let candidates = projection_domain_candidates(program, machine, subject);
                    let [candidate] = candidates.as_slice() else {
                        return None;
                    };
                    let domain = candidate.definition;
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
                    let report_fingerprint = projection_report_fingerprint(&algebra, &expression);
                    Some(ContentProjectionPlan {
                        domain: domain.symbol,
                        semantic_domain: candidate.semantic_domain,
                        carrier_identity: program
                            .normalized_type_identity(subject.carrier)
                            .into_string(),
                        machine: machine.symbol,
                        algebra,
                        expression,
                        report_fingerprint,
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
    conformance: &psi_typed_trees::machine::TraitConformance,
) -> Option<ContentAlgebraIdentity> {
    let [argument] = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        return None;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        arguments,
        ..
    } = program.type_reference_table.type_reference(*argument)
    else {
        return None;
    };
    if !compiler_owned_symbol(program, *base_symbol) {
        return None;
    }
    let [identity] = program
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        return None;
    };
    let identity = program.normalized_type_identity(*identity).into_string();
    match base_name.as_str().rsplit("::").next() {
        Some("IntervalSet") => Some(ContentAlgebraIdentity::IntervalSet {
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
        ContentAlgebraIdentity::IntervalSet { .. } => "IntervalSet",
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
        ContentAlgebraIdentity::IntervalSet { .. } => {
            let [start, end] = ["start", "end"].map(|name| {
                fields
                    .iter()
                    .find(|field| field.name.as_str() == name)
                    .and_then(|field| normalize_projection_scalar(program, subject, field.value))
            });
            (fields.len() == 2).then_some(ContentProjectionExpression::IntervalSet {
                members: vec![ContentIntervalExpression::new(start?, end?)],
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
        // Content algebras are Nat-valued, while runtime integers and
        // addresses uniformly embed into proof Int. The explicit exact
        // conversion is semantic evidence at the source boundary; the
        // canonical content term retains only the embedded mathematical
        // scalar. Accept no other cast target or inner expression here.
        ExpressionNode::Cast(cast)
            if program
                .named_type_reference(cast.target_type)
                .is_some_and(|name| name.as_str() == "Nat")
                && cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Exact
                && cast.semantic_domain.count() == 0 =>
        {
            normalize_projection_scalar(program, subject, cast.value)
        }
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
        ExpressionNode::Call(call) if is_content_scalar_embedding(program, call) => {
            let [argument] = program.expression_table.expression_handles(call.arguments) else {
                return None;
            };
            let (path, field_type) = normalize_subject_field(program, subject, *argument)?;
            runtime_scalar_can_embed(program, field_type)
                .then_some(ContentScalarExpression::RuntimeScalarEmbedding(path))
        }
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
) -> Option<&'program psi_typed_trees::data::DataField> {
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
            psi_typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                Some(field)
            }
            _ => None,
        })
}

fn is_content_projection_trait(program: &TypedTrees, candidate: &TraitDefinition) -> bool {
    compiler_owned_symbol(program, candidate.symbol)
        && candidate
            .name
            .as_str()
            .rsplit("::")
            .next()
            .is_some_and(|name| name == "Content")
        && !candidate.is_boundary
        && program.trait_type_parameters(candidate).len() == 1
        && matches!(program.trait_machine_signatures(candidate), [requirement] if requirement.name.as_str() == "project")
}

fn is_content_scalar_embedding(program: &TypedTrees, call: &TableCallExpression) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str().rsplit("::").next() == Some("embed")
        && compiler_owned_symbol(program, call.target_symbol)
}

fn runtime_scalar_can_embed(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let type_reference = unconstrained(program, type_reference);
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { name, .. }
            if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "addr")
    )
}

fn compiler_owned_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    let owning_machine = program.machines().iter().find(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    });
    let declaration = owning_machine.map_or(symbol, |machine| machine.symbol);
    let authored = program
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == declaration)
        .map_or(declaration, |specialization| specialization.template);
    match program.symbols.symbol_source_origin(authored) {
        Some(psi_source::SourceOrigin::Toolchain) => true,
        Some(psi_source::SourceOrigin::User) => false,
        None => !program.symbols.has_source_metadata(),
    }
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
            referee, access, ..
        } if access.is_readable() && !access.is_exclusive() => Some(ProjectionSubject {
            symbol: parameter.symbol,
            qualified: *referee,
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
    subject: ProjectionSubject,
) -> Vec<ProjectionDomainCandidate<'program>> {
    let Some(owner) = machine.attached_data.as_ref().map(|name| name.as_str()) else {
        return Vec::new();
    };
    let owner_leaf = owner.rsplit("::").next().unwrap_or(owner);
    program
        .domain_definitions()
        .iter()
        .filter_map(|domain| {
            let name = domain.name.as_str();
            let leaf = name.rsplit("::").next().unwrap_or(name);
            if (name != owner && leaf != owner_leaf)
                || !domain_accepts_projection_carrier(program, domain, subject.carrier)
            {
                return None;
            }
            let exact = declared_domain_constraints(program, subject.qualified)
                .into_iter()
                .filter(|constraint| {
                    (constraint.symbol.is_valid() && constraint.symbol == domain.symbol)
                        || (!constraint.symbol.is_valid()
                            && constraint.name.as_str().rsplit("::").next() == Some(owner_leaf))
                })
                .collect::<Vec<_>>();
            match exact.as_slice() {
                [constraint] => Some(ProjectionDomainCandidate {
                    definition: domain,
                    semantic_domain: constraint.semantic_id,
                }),
                [] if domain.index_arguments.is_empty() => Some(ProjectionDomainCandidate {
                    definition: domain,
                    semantic_domain: domain.semantic_id,
                }),
                _ => None,
            }
        })
        .collect()
}

fn domain_accepts_projection_carrier(
    program: &TypedTrees,
    domain: &DomainDefinition,
    carrier: TypeReferenceHandle,
) -> bool {
    if type_references_match(program, carrier, domain.target_type) {
        return true;
    }
    let parameters = program.domain_type_parameters(domain);
    let TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(domain.target_type)
    else {
        return false;
    };
    carrier.is_valid()
        && parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
            ) && (*symbol == parameter.symbol || name == &parameter.name)
        })
}

fn declared_domain_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<&psi_typed_trees::types::DomainConstraint> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            declared_domain_constraints(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut domains = declared_domain_constraints(program, *base_type);
            domains.extend(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| match constraint {
                        psi_typed_trees::types::TypeConstraintNode::Domain(domain) => Some(domain),
                        _ => None,
                    }),
            );
            domains
        }
        _ => Vec::new(),
    }
}
