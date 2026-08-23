use crate::StateSignatureOwner;
use psi_diagnostics::Diagnostic;
use psi_facts::FactPlan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProofFactOwner<'program> {
    DataDefaultDomain(&'program str),
    Domain(&'program str),
    MachineContract {
        machine: &'program str,
        kind: &'static str,
    },
    TraitInvariant {
        trait_definition: &'program str,
    },
    StateSignatureContract {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        kind: &'static str,
    },
}

pub(crate) fn validate_proposition_definitions(
    program: &TypedTrees,
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
        if let psi_typed_trees::proposition::PropositionBody::Witness { evidence } =
            proposition.body
        {
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
    program: &TypedTrees,
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
                    "witness-bearing proposition `{proposition}` names selected dynamic evidence `dyn {name}`; name the carrierless trait interface directly"
                )));
            return;
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                    "witness-bearing proposition `{proposition}` evidence `{}` must name one carrierless trait interface",
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
    if let Some(reason) = carrierless_trait_violation(program, trait_definition, &mut visiting) {
        diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` is not carrierless: {reason}"
        )));
    }
}

fn evidence_argument_is_resolved(program: &TypedTrees, argument: TypeReferenceHandle) -> bool {
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

fn carrierless_trait_violation(
    program: &TypedTrees,
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

    if trait_definition
        .conformance_bounds
        .iter()
        .any(|bound| bound.subject_name.as_str() == "Self")
    {
        visiting.pop();
        return Some("a conformance bound selects `Self` as its subject".to_owned());
    }

    for requirement in program.trait_machine_signatures(trait_definition) {
        for parameter in program.state_signature_parameters(requirement) {
            if parameter.is_self || type_reference_contains_self(program, parameter.type_reference)
            {
                visiting.pop();
                return Some(format!(
                    "requirement `{}` has a carrier-dependent parameter",
                    requirement.name
                ));
            }
        }
        if requirement.return_type.is_valid()
            && type_reference_contains_self(program, requirement.return_type)
        {
            visiting.pop();
            return Some(format!(
                "requirement `{}` has a carrier-dependent result",
                requirement.name
            ));
        }
    }

    for parent in program.trait_requirements(trait_definition) {
        if program
            .type_reference_table
            .type_reference_handles(parent.arguments)
            .iter()
            .any(|argument| type_reference_contains_self(program, *argument))
        {
            visiting.pop();
            return Some(format!(
                "parent interface `{}` is applied to `Self`",
                parent.name
            ));
        }
        let Some(parent_definition) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == parent.symbol)
        else {
            continue;
        };
        if let Some(reason) = carrierless_trait_violation(program, parent_definition, visiting) {
            visiting.pop();
            return Some(format!(
                "parent interface `{}` is not carrierless: {reason}",
                parent.name
            ));
        }
    }

    visiting.pop();
    None
}

fn type_reference_contains_self(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => name.as_str() == "Self",
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_self(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_self(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_self(program, *element_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_reference_contains_self(program, *argument)),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn transparent_proposition_cycle(
    program: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> bool {
    if visiting.contains(&symbol) {
        return true;
    }
    let Some(proposition) = program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == symbol)
    else {
        return false;
    };
    let psi_typed_trees::proposition::PropositionBody::Transparent {
        proposition: psi_typed_trees::proposition::PropositionFormula::Application(application),
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
            Self::TraitInvariant { trait_definition } => {
                write!(formatter, "trait `{trait_definition}` invariant")
            }
            Self::StateSignatureContract { owner, state, kind } => {
                write!(formatter, "{owner} state `{state}` {kind} contract")
            }
        }
    }
}

pub(crate) fn validate_domain_fact_payloads(
    program: &TypedTrees,
    fact_plan: &FactPlan,
    symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in fact_plan.boolean_facts_for_symbol(symbol) {
        if !is_boolean_fact_expression(program, fact.expression) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} proof fact `{}` is not boolean-shaped",
                program.expression_table.display_name(fact.expression)
            )));
        }
    }

    for membership in fact_plan.domain_memberships_for_symbol(symbol) {
        if membership.domain_symbol.is_valid()
            || is_implicit_case_domain(program, membership.domain)
            || carry_permission(program, membership.domain).is_some()
        {
            continue;
        }

        diagnostics.push(Diagnostic::error(format!(
            "{owner} references unknown domain `{}`",
            domain_path_label(program, membership.domain)
        )));
    }
}

pub(crate) fn validate_proof_facts(
    program: &TypedTrees,
    facts: &[ProofFact],
    diagnostics: &mut Vec<Diagnostic>,
    owner: ProofFactOwner<'_>,
) {
    for fact in facts {
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
                if membership.domain_symbol.is_valid()
                    || is_implicit_case_domain(program, membership.domain)
                    || carry_permission(program, membership.domain).is_some()
                {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown domain `{}`",
                    domain_path_label(program, membership.domain)
                )));
            }
            ProofFact::Proposition(application) => {
                let declaration = program
                    .propositions()
                    .iter()
                    .find(|proposition| proposition.symbol == application.proposition);
                let parameter_signature = program
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .find_map(|parameter| match &parameter.kind {
                        psi_typed_trees::data::TypeParameterKind::Proposition { contract }
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
                    let instantiated_type = declaration.map(|declaration| {
                        proposition_parameter_instantiation(
                            program,
                            declaration,
                            application,
                            parameter.type_reference,
                        )
                    });
                    let instantiated_type = instantiated_type.unwrap_or(
                        PropositionParameterInstantiation::Unchanged(parameter.type_reference),
                    );
                    let matches = match instantiated_type {
                        PropositionParameterInstantiation::Unchanged(expected_type) => {
                            crate::expression_types::argument_matches_type_reference_handle(
                                program,
                                *argument,
                                expected_type,
                            )
                        }
                        PropositionParameterInstantiation::Selected(selected) => {
                            argument_matches_selected_proposition_type(
                                program, *argument, selected, owner,
                            )
                        }
                        PropositionParameterInstantiation::Invalid => false,
                    };
                    if !matches {
                        let expected_name = match instantiated_type {
                            PropositionParameterInstantiation::Unchanged(expected_type) => {
                                program.display_type_reference(expected_type)
                            }
                            PropositionParameterInstantiation::Selected(selected) => {
                                selected.display_name()
                            }
                            PropositionParameterInstantiation::Invalid => {
                                "<invalid proposition binder argument>".to_owned()
                            }
                        };
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

#[derive(Debug, Clone, Copy)]
enum PropositionParameterInstantiation<'application> {
    Unchanged(psi_typed_trees::types::TypeReferenceHandle),
    Selected(&'application psi_typed_trees::proposition::PropositionBinderArgument),
    Invalid,
}

fn proposition_parameter_instantiation<'application>(
    program: &TypedTrees,
    declaration: &psi_typed_trees::proposition::PropositionDefinition,
    application: &'application psi_typed_trees::proposition::PropositionApplication,
    parameter_type: psi_typed_trees::types::TypeReferenceHandle,
) -> PropositionParameterInstantiation<'application> {
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(parameter_type)
    else {
        return PropositionParameterInstantiation::Unchanged(parameter_type);
    };
    let Some((binder_index, _)) = program
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .find(|(_, binder)| {
            binder.symbol == *symbol
                && matches!(
                    binder.kind,
                    psi_typed_trees::proposition::PropositionBinderKind::Type
                )
        })
    else {
        return PropositionParameterInstantiation::Unchanged(parameter_type);
    };
    let Some(selected) = application.binder_arguments.get(binder_index) else {
        return PropositionParameterInstantiation::Invalid;
    };
    if !matches!(
        selected.kind,
        psi_typed_trees::proposition::PropositionBinderArgumentKind::Type
    ) {
        return PropositionParameterInstantiation::Invalid;
    }
    PropositionParameterInstantiation::Selected(selected)
}

fn argument_matches_selected_proposition_type(
    program: &TypedTrees,
    argument: ExpressionHandle,
    selected: &psi_typed_trees::proposition::PropositionBinderArgument,
    owner: ProofFactOwner<'_>,
) -> bool {
    if let Some(actual) = proof_argument_declared_type(program, argument, owner) {
        if let (Some(actual), Some(selected)) = (
            program.type_reference_table.primitive_type(actual),
            psi_typed_trees::types::PrimitiveType::from_name(&selected.display_name()),
        ) {
            return actual == selected;
        }
        return program.type_reference_table.type_symbol(actual) == selected.symbol;
    }
    let selected_name = selected.display_name();
    if let Some(primitive) = psi_typed_trees::types::PrimitiveType::from_name(&selected_name) {
        return match program.expression_table.expression(argument) {
            ExpressionNode::Boolean(_) => primitive == psi_typed_trees::types::PrimitiveType::Bool,
            ExpressionNode::Float(_) => primitive.accepts_float_literal(),
            ExpressionNode::Integer(_) => primitive.accepts_integer_literal(),
            ExpressionNode::Borrow(inner) => {
                argument_matches_selected_proposition_type(program, inner.target, selected, owner)
            }
            _ => false,
        };
    }
    program
        .type_reference_table
        .find_named_type_reference(selected.symbol)
        .is_some_and(|expected| {
            crate::expression_types::argument_matches_type_reference_handle(
                program, argument, expected,
            )
        })
}

fn proof_argument_declared_type(
    program: &TypedTrees,
    argument: ExpressionHandle,
    owner: ProofFactOwner<'_>,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if let Some(member_name) = self_member_name(program, argument)
        && let Some(machine_name) = proof_fact_machine_name(owner)
        && let Some(attached_data) = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .and_then(|machine| machine.attached_data.as_ref())
        && let Some(definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached_data)
        && let Some(type_reference) =
            program
                .data_members(definition)
                .iter()
                .find_map(|field| match field {
                    psi_typed_trees::data::DataMember::Field(field)
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
        ExpressionNode::Borrow(inner) => {
            return proof_argument_declared_type(program, inner.target, owner);
        }
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
            program.data_definitions().iter().find_map(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Field(field)
                            if field.symbol == symbol =>
                        {
                            Some(field.type_reference)
                        }
                        _ => None,
                    })
            })
        })
}

fn proof_fact_machine_name(owner: ProofFactOwner<'_>) -> Option<&str> {
    match owner {
        ProofFactOwner::MachineContract { machine, .. } => Some(machine),
        ProofFactOwner::StateSignatureContract {
            owner: StateSignatureOwner::Machine(machine),
            ..
        } => Some(machine),
        ProofFactOwner::DataDefaultDomain(_)
        | ProofFactOwner::Domain(_)
        | ProofFactOwner::TraitInvariant { .. }
        | ProofFactOwner::StateSignatureContract { .. } => None,
    }
}

fn self_member_name(program: &TypedTrees, expression: ExpressionHandle) -> Option<&str> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            matches!(
                program.expression_table.name_path_members(path.members),
                [name] if name.as_str() == "self"
            )
            .then_some(member.member.as_str())
        }
        ExpressionNode::Name(path) => {
            match program.expression_table.name_path_members(path.members) {
                [root, member] if root.as_str() == "self" => Some(member.as_str()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn carry_permission(
    program: &TypedTrees,
    domain: psi_arena::HandleSpan<Identifier>,
) -> Option<psi_language_semantics::CarryPermission> {
    let name = domain_path_label(program, domain);
    psi_language_semantics::CarryPermission::from_name(&name)
}

fn is_implicit_case_domain(
    program: &TypedTrees,
    domain: psi_arena::HandleSpan<Identifier>,
) -> bool {
    let [type_name, case_name] = program.domain_path_members(domain) else {
        return false;
    };
    program.data_definitions().iter().any(|definition| {
        definition.name.as_str() == type_name.as_str()
            && program.data_members(definition).iter().any(|member| {
                matches!(
                    member,
                    psi_typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case_name.as_str()
                )
            })
    })
}

fn is_boolean_fact_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => is_boolean_fact_expression(program, atomic.value),
        ExpressionNode::Binary(binary) => match binary.operator {
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
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            crate::places::declared_place_type_raw(program, machine, state, expression)
                .and_then(|handle| program.primitive_type_reference(handle))
                == Some(psi_typed_trees::types::PrimitiveType::Bool)
        }
        ExpressionNode::Borrow(inner) => {
            is_boolean_asm_fact_expression(program, machine, state, inner.target)
        }
        _ => is_boolean_fact_expression(program, expression),
    }
}

fn domain_path_label(program: &TypedTrees, domain: psi_arena::HandleSpan<Identifier>) -> String {
    let path = program.domain_path_members(domain);
    if path.is_empty() {
        return "<unknown>".to_owned();
    }

    path.iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(crate) fn string_literal_grants_domain(
    program: &TypedTrees,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let ExpressionNode::String(literal) = program.expression_table.expression(expression) else {
        return false;
    };
    psi_typed_trees::byte_predicates::domain_byte_predicate(program, domain_symbol)
        .is_some_and(|predicate| predicate.holds_for(literal))
}

pub(crate) fn domain_admits_empty_bytes(program: &TypedTrees, domain_symbol: SymbolHandle) -> bool {
    psi_typed_trees::byte_predicates::domain_byte_predicate(program, domain_symbol)
        .is_some_and(|predicate| predicate.holds_for(&[]))
}
