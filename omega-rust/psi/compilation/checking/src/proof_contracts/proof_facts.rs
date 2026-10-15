use crate::StateSignatureOwner;
use diagnostics::Diagnostic;
use facts::FactPlan;
use std::fmt;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod proposition_parameters;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProofFactOwner<'program> {
    DataDefaultDomain(&'program str),
    Domain(&'program str),
    MachineContract {
        machine: &'program str,
        kind: &'static str,
    },
    StateSignatureContract {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        kind: &'static str,
    },
}

/// `Some(diagnostic)` when the membership names a domain that is neither
/// resolved, implicit, nor a carried permission — the unknown-domain
/// rejection both validation loops emit.
fn unknown_membership_domain_diagnostic(
    program: &dyn crate::CheckingInput,
    owner: ProofFactOwner<'_>,
    domain: arena::HandleSpan<Identifier>,
    domain_symbol: symbols::SymbolHandle,
) -> Option<Diagnostic> {
    if domain_symbol.is_valid()
        || is_implicit_case_domain(program, domain)
        || carry_permission(program, domain).is_some()
    {
        return None;
    }
    Some(Diagnostic::error(format!(
        "{owner} references unknown domain `{}`",
        domain_path_label(program, domain)
    )))
}

pub(crate) fn validate_proposition_definitions(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for proposition in program.propositions() {
        let mut visiting = Vec::new();
        if transparent_proposition_cycle(program, proposition.symbol, &mut visiting) {
            diagnostics.push(Diagnostic::error(format!(
                "transparent proposition `{}` participates in an alias cycle",
                proposition.name.as_str()
            )));
        }
        if let typed_trees::proposition::PropositionBody::Witness { evidence } = proposition.body {
            validate_witness_evidence_interface(
                program,
                proposition.name.as_str(),
                evidence,
                diagnostics,
            );
        }
    }
}

fn validate_witness_evidence_interface(
    program: &dyn crate::CheckingInput,
    proposition: &str,
    evidence: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !evidence.is_valid() {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` has no resolved evidence interface"
        )));
        return;
    }

    let (trait_symbol, trait_name, lifetime_argument_count, arguments) = match program
        .type_reference_table
        .type_reference(evidence)
    {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str(), 0, &[][..]),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => (
            *base_symbol,
            base_name.as_str(),
            lifetime_arguments.len(),
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        ),
        TypeReferenceNode::DynamicTrait { name, .. } => {
            diagnostics.push(Diagnostic::error(format!(
                    "witness-bearing proposition `{proposition}` names selected dynamic evidence `dyn {name}`; name the trait interface directly"
                )));
            return;
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                    "witness-bearing proposition `{proposition}` evidence `{}` must name one proof evidence trait interface",
                    program.display_type_reference(evidence)
                )));
            return;
        }
    };

    let Some(trait_definition) = program.traits().iter().find(|candidate| {
        (trait_symbol.is_valid() && candidate.symbol == trait_symbol)
            || candidate.name.as_str() == trait_name
    }) else {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence `{}` is not a trait interface",
            program.display_type_reference(evidence)
        )));
        return;
    };

    if lifetime_argument_count != trait_definition.lifetime_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` expects {} lifetime argument(s), but the interface supplies {lifetime_argument_count}",
            trait_definition.lifetime_parameters.len()
        )));
    }

    let parameter_count = program.trait_type_parameters(trait_definition).len();
    if arguments.len() != parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` expects {parameter_count} generic argument(s), but the interface supplies {}",
            arguments.len()
        )));
    }
    for argument in arguments {
        if !evidence_argument_is_resolved(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` has unresolved generic argument `{}`",
                program.display_type_reference(*argument)
            )));
        }
    }

    let mut visiting = Vec::new();
    if let Some(reason) = evidence_trait_violation(program, trait_definition, &mut visiting) {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` is not a proof evidence interface: {reason}"
        )));
    }
}

fn evidence_argument_is_resolved(
    program: &dyn crate::CheckingInput,
    argument: TypeReferenceHandle,
) -> bool {
    if !argument.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { symbol, .. } => symbol.is_valid(),
        TypeReferenceNode::Reference { referee, .. } => {
            evidence_argument_is_resolved(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            evidence_argument_is_resolved(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            evidence_argument_is_resolved(program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            base_symbol.is_valid()
                && program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|argument| evidence_argument_is_resolved(program, *argument))
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => true,
        TypeReferenceNode::DynamicTrait { symbol, .. } => symbol.is_valid(),
    }
}

/// A witness-bearing evidence interface must describe proof-only machines:
/// a boundary trait, directly or through an inherited parent, describes
/// externally executed services instead.
fn evidence_trait_violation(
    program: &dyn crate::CheckingInput,
    trait_definition: &TraitDefinition,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<String> {
    if trait_definition.is_boundary {
        return Some("boundary traits describe externally executed services".to_owned());
    }
    if visiting.contains(&trait_definition.symbol) {
        return None;
    }
    visiting.push(trait_definition.symbol);
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_definition) = program.trait_definition_by_symbol(parent.symbol) else {
            continue;
        };
        if let Some(reason) = evidence_trait_violation(program, parent_definition, visiting) {
            visiting.pop();
            return Some(format!(
                "parent interface `{}` is not a proof evidence interface: {reason}",
                parent.name
            ));
        }
    }
    visiting.pop();
    None
}

fn transparent_proposition_cycle(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    if visiting.contains(&symbol) {
        return true;
    }
    let Some(proposition) = program.proposition_definition_by_symbol(symbol) else {
        return false;
    };
    let typed_trees::proposition::PropositionBody::Transparent {
        proposition: typed_trees::proposition::PropositionFormula::Application(application),
    } = &proposition.body
    else {
        return false;
    };
    visiting.push(symbol);
    let cyclic = transparent_proposition_cycle(program, application.proposition, visiting);
    visiting.pop();
    cyclic
}

impl fmt::Display for ProofFactOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataDefaultDomain(data) => write!(formatter, "data `{data}` default domain"),
            Self::Domain(domain) => write!(formatter, "domain `{domain}`"),
            Self::MachineContract { machine, kind } => {
                write!(formatter, "machine `{machine}` {kind} contract")
            }
            Self::StateSignatureContract { owner, state, kind } => {
                write!(formatter, "{owner} state `{state}` {kind} contract")
            }
        }
    }
}

pub(crate) fn validate_domain_fact_payloads(
    program: &dyn crate::CheckingInput,
    fact_plan: &FactPlan,
    domain: &typed_trees::domain::DomainDefinition,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in fact_plan.boolean_facts_for_symbol(domain.symbol) {
        if !is_boolean_fact_expression(program, fact.expression) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} proof fact `{}` is not boolean-shaped",
                program.expression_table.display_name(fact.expression)
            )));
        } else if !is_boolean_domain_fact_expression(program, domain, fact.expression) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} proof fact `{}` must have type `bool`",
                program.expression_table.display_name(fact.expression)
            )));
        }
    }

    for membership in fact_plan.domain_memberships_for_symbol(domain.symbol) {
        if let Some(diagnostic) = unknown_membership_domain_diagnostic(
            program,
            owner,
            membership.domain,
            membership.domain_symbol,
        ) {
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) fn validate_proof_facts(
    program: &dyn crate::CheckingInput,
    facts: &[ProofFact],
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in facts {
        if matches!(
            owner,
            ProofFactOwner::StateSignatureContract {
                owner: StateSignatureOwner::Trait(_),
                ..
            }
        ) && let Some(member) = abstract_trait_self_member_in_fact(program, fact)
        {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} references undeclared structural member `{member}`; abstract `Self` \
                 has no field namespace -- use an explicit proposition parameter or a declared \
                 accessor requirement"
            )));
        }
        match fact {
            ProofFact::Expression(expression) => {
                if !is_boolean_fact_expression(program, *expression) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} proof fact `{}` is not boolean-shaped",
                        program.expression_table.display_name(*expression)
                    )));
                }
            }
            ProofFact::Membership(membership) => {
                if let Some(diagnostic) = unknown_membership_domain_diagnostic(
                    program,
                    owner,
                    membership.domain,
                    membership.domain_symbol,
                ) {
                    diagnostics.push(diagnostic);
                }
            }
            ProofFact::Proposition(application) => {
                let declaration = program.proposition_definition_by_symbol(application.proposition);
                let parameter_signature = program
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .find_map(|parameter| match &parameter.kind {
                        typed_trees::data::TypeParameterKind::Proposition { contract }
                            if parameter.symbol == application.proposition =>
                        {
                            Some(contract)
                        }
                        _ => None,
                    });
                if declaration.is_none() && parameter_signature.is_none() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} references an unknown proposition `{}`",
                        application.name.as_str()
                    )));
                    continue;
                }
                let parameters = if let Some(declaration) = declaration {
                    program.proposition_parameters(declaration)
                } else if let Some(signature) = parameter_signature {
                    program.state_parameters.span_or_empty(signature.parameters)
                } else {
                    &[]
                };
                for (index, (argument, parameter)) in program
                    .expression_table
                    .expression_handles(application.arguments)
                    .iter()
                    .zip(parameters)
                    .enumerate()
                {
                    let matches = proposition_parameters::argument_matches(
                        program,
                        *argument,
                        parameter.type_reference,
                        declaration,
                        application,
                        owner,
                    );
                    if !matches {
                        let expected_name =
                            program.display_type_reference(parameter.type_reference);
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} proposition `{}` argument {} does not match parameter `{}` type `{}`",
                            application.name.as_str(),
                            index + 1,
                            parameter.name.as_str(),
                            expected_name,
                        )));
                    }
                }
            }
        }
    }
}

fn abstract_trait_self_member_in_fact(
    program: &dyn crate::CheckingInput,
    fact: &ProofFact,
) -> Option<String> {
    let mut visited = Vec::new();
    match fact {
        ProofFact::Expression(expression) => {
            abstract_trait_self_member_in_expression(program, *expression, &mut visited)
        }
        ProofFact::Membership(membership) => {
            abstract_trait_self_member_in_expression(program, membership.value, &mut visited)
        }
        ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .find_map(|argument| {
                abstract_trait_self_member_in_expression(program, *argument, &mut visited)
            }),
    }
}

fn abstract_trait_self_member_in_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
) -> Option<String> {
    if !expression.is_valid() || visited.contains(&expression) {
        return None;
    }
    visited.push(expression);
    if let Some(member) = self_member_name(program, expression) {
        return Some(member.to_owned());
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            crate::expression_traversal::match_children(program, *dispatch)
                .find_map(|child| abstract_trait_self_member_in_expression(program, child, visited))
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .find_map(|value| abstract_trait_self_member_in_expression(program, *value, visited)),
        ExpressionNode::Atomic(atomic) => {
            abstract_trait_self_member_in_expression(program, atomic.value, visited).or_else(|| {
                abstract_trait_self_member_in_expression(program, atomic.result, visited)
            })
        }
        ExpressionNode::Binary(binary) => {
            abstract_trait_self_member_in_expression(program, binary.left, visited).or_else(|| {
                abstract_trait_self_member_in_expression(program, binary.right, visited)
            })
        }
        ExpressionNode::Cast(cast) => {
            abstract_trait_self_member_in_expression(program, cast.value, visited)
        }
        ExpressionNode::Call(call) => {
            abstract_trait_self_member_in_expression(program, call.receiver, visited).or_else(
                || {
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .find_map(|argument| {
                            abstract_trait_self_member_in_expression(program, *argument, visited)
                        })
                },
            )
        }
        ExpressionNode::Indexed(indexed) => {
            abstract_trait_self_member_in_expression(program, indexed.collection, visited).or_else(
                || abstract_trait_self_member_in_expression(program, indexed.index, visited),
            )
        }
        ExpressionNode::Member(member) => {
            abstract_trait_self_member_in_expression(program, member.receiver, visited)
        }
        ExpressionNode::Borrow(value) => {
            abstract_trait_self_member_in_expression(program, value.target, visited)
        }
        ExpressionNode::Range(range) => {
            abstract_trait_self_member_in_expression(program, range.start, visited)
                .or_else(|| abstract_trait_self_member_in_expression(program, range.end, visited))
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find_map(|field| {
                abstract_trait_self_member_in_expression(program, field.value, visited)
            }),
        ExpressionNode::Unary(unary) => {
            abstract_trait_self_member_in_expression(program, unary.operand, visited)
        }
        typed_trees::literal_expression_pattern!() => None,
    }
}

fn proof_argument_declared_type(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    owner: ProofFactOwner<'_>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(member_name) = self_member_name(program, argument)
        && let Some(machine_name) = proof_fact_machine_name(owner)
        && let Some(attached_data) = program
            .machine_by_name_str(machine_name)
            .and_then(|machine| machine.attached_data.as_ref())
        && let Some(definition) = program.data_definition_by_name(attached_data)
        && let Some(type_reference) =
            program
                .data_members(definition)
                .iter()
                .find_map(|field| match field {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member_name =>
                    {
                        Some(field.type_reference)
                    }
                    _ => None,
                })
    {
        return Some(type_reference);
    }
    let symbol = match program.expression_table.expression(argument) {
        ExpressionNode::Name(path) => path.symbol,
        ExpressionNode::Member(member) => member.member_symbol,
        _ => return None,
    };
    if !symbol.is_valid() {
        return None;
    }
    program
        .state_parameters
        .iter()
        .map(|(_, parameter)| parameter)
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .field_owner(symbol)
                .map(|(_, field)| field.type_reference)
        })
}

fn proof_fact_machine_name(owner: ProofFactOwner<'_>) -> Option<&str> {
    match owner {
        ProofFactOwner::MachineContract { machine, .. } => Some(machine),
        ProofFactOwner::StateSignatureContract {
            owner: StateSignatureOwner::Machine(machine),
            ..
        } => Some(machine.name.as_str()),
        ProofFactOwner::DataDefaultDomain(_)
        | ProofFactOwner::Domain(_)
        | ProofFactOwner::StateSignatureContract { .. } => None,
    }
}

fn self_member_name(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<&str> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let Some(path) = program.expression_table.name_path_node(member.receiver) else {
                return None;
            };
            matches!(
                program.expression_table.name_path_members(path.members),
                [name] if name.is_self_receiver()
            )
            .then_some(member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            match program.expression_table.name_path_members(path.members) {
                [root, member] if root.is_self_receiver() => Some(member.as_str()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn carry_permission(
    program: &dyn crate::CheckingInput,
    domain: arena::HandleSpan<Identifier>,
) -> Option<language_semantics::CarryPermission> {
    let name = domain_path_label(program, domain);
    language_semantics::CarryPermission::from_name(&name)
}

fn is_implicit_case_domain(
    program: &dyn crate::CheckingInput,
    domain: arena::HandleSpan<Identifier>,
) -> bool {
    let [type_name, case_name] = program.domain_path_members(domain) else {
        return false;
    };
    program.data_definitions().iter().any(|definition| {
        definition.name.as_str() == type_name.as_str()
            && program.data_members(definition).iter().any(|member| {
                matches!(
                    member,
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case_name.as_str()
                )
            })
    })
}

/// Domain predicates know their carrier, unlike the context-free signature
/// shape check. Resolve value places through that carrier and check each
/// logical operand: wrapping an integer field in `!` or `&&` does not turn it
/// into a proposition. Relations and proof-call formation keep their existing
/// validators; this does not demand executable Boolean results from logical
/// proposition applications.
fn is_boolean_domain_fact_expression(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            crate::value_custody::expression_types::domain_expression_result_type_reference(
                program, domain, expression,
            )
            .and_then(|reference| program.primitive_type_reference(reference))
                == Some(typed_trees::types::PrimitiveType::Bool)
        }
        ExpressionNode::Atomic(atomic) => {
            is_boolean_domain_fact_expression(program, domain, atomic.value)
        }
        ExpressionNode::Unary(unary) => {
            unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
                && is_boolean_domain_fact_expression(program, domain, unary.operand)
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            is_boolean_domain_fact_expression(program, domain, binary.left)
                && is_boolean_domain_fact_expression(program, domain, binary.right)
        }
        ExpressionNode::Borrow(_) => false,
        _ => is_boolean_fact_expression(program, expression),
    }
}

fn is_boolean_fact_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(_) => false,
        ExpressionNode::Atomic(atomic) => is_boolean_fact_expression(program, atomic.value),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::CaseMembership => true,
            BinaryOperator::And
            | BinaryOperator::Equal
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::NotEqual
            | BinaryOperator::Or => true,
            BinaryOperator::Add
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract => false,
        },
        ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_) => true,
        ExpressionNode::Range(_) => false,
        ExpressionNode::Borrow(inner) => is_boolean_fact_expression(program, inner.target),
        ExpressionNode::Unary(unary) => is_boolean_fact_expression(program, unary.operand),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Statement-position assembly facts have machine/state type context, so a
/// bare place must actually be declared `bool`; the older signature-fact
/// shape check cannot resolve those scoped names. Computed predicates keep the
/// same accepted proof-expression surface as ordinary contracts.
pub(crate) fn is_boolean_asm_fact_expression(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            crate::value_custody::places::declared_place_type_raw(
                program, machine, state, expression,
            )
            .and_then(|handle| program.primitive_type_reference(handle))
                == Some(typed_trees::types::PrimitiveType::Bool)
        }
        ExpressionNode::Borrow(inner) => {
            is_boolean_asm_fact_expression(program, machine, state, inner.target)
        }
        _ => is_boolean_fact_expression(program, expression),
    }
}

fn domain_path_label(
    program: &dyn crate::CheckingInput,
    domain: arena::HandleSpan<Identifier>,
) -> String {
    let path = program.domain_path_members(domain);
    if path.is_empty() {
        return "<unknown>".to_owned();
    }

    path.joined_path()
}

pub(crate) fn string_literal_grants_domain(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(literal) = typed_trees::byte_predicates::literal_bytes(program, expression) else {
        return false;
    };
    typed_trees::byte_predicates::domain_byte_predicate(program, domain_symbol)
        .is_some_and(|predicate| predicate.holds_for(&literal))
}

pub(crate) fn domain_admits_zero_bytes(
    program: &dyn crate::CheckingInput,
    domain_symbol: SymbolHandle,
) -> bool {
    program
        .domain_definition(domain_symbol)
        .is_some_and(|domain| {
            crate::value_custody::field_domains::domain_admits_zero_byte_carrier(
                program,
                domain_symbol,
                domain.target_type,
            )
        })
}

/// The `Expression` facts carried by `Ensures` contracts in `contracts`, in
/// order — the entitlement surface every entailment/seed loop filters down to.
pub(crate) fn ensures_expression_facts<'a>(
    program: &'a dyn crate::CheckingInput,
    contracts: &'a [typed_trees::signature::SignatureContract],
) -> impl Iterator<Item = ExpressionHandle> + 'a {
    ensures_facts(program, contracts).filter_map(|fact| {
        let expression = fact.as_expression()?;
        Some(*expression)
    })
}

/// Every fact carried by `Ensures` contracts in `contracts`, in order.
pub(crate) fn ensures_facts<'a>(
    program: &'a dyn crate::CheckingInput,
    contracts: &'a [typed_trees::signature::SignatureContract],
) -> impl Iterator<Item = &'a ProofFact> + 'a {
    contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
}

/// Every `Expression` fact inside `contracts` of `kind` — Requires and
/// Ensures bodies read through the same vocabulary.
pub(crate) fn contract_expression_facts<'a>(
    program: &'a dyn crate::CheckingInput,
    contracts: impl Iterator<Item = &'a typed_trees::signature::SignatureContract> + 'a,
    kind: typed_trees::signature::SignatureContractKind,
) -> impl Iterator<Item = ExpressionHandle> + 'a {
    contracts
        .filter(move |contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| {
            let expression = fact.as_expression()?;
            Some(*expression)
        })
}
