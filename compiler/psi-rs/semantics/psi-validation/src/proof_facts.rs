use crate::StateSignatureOwner;
use psi_diagnostics::Diagnostic;
use psi_facts::FactPlan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::name::Identifier;
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
            && !evidence.is_valid()
        {
            diagnostics.push(Diagnostic::error(format!(
                "witness-bearing proposition `{}` has no resolved evidence interface",
                proposition.name.as_str()
            )));
        }
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
                    if !crate::expression_types::argument_matches_type_reference_handle(
                        program,
                        *argument,
                        parameter.type_reference,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} proposition `{}` argument {} does not match parameter `{}` type `{}`",
                            application.name.as_str(),
                            index + 1,
                            parameter.name.as_str(),
                            program.display_type_reference(parameter.type_reference),
                        )));
                    }
                }
            }
        }
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
        ExpressionNode::Mutable(inner) => is_boolean_fact_expression(program, *inner),
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
        ExpressionNode::Mutable(inner) => {
            is_boolean_asm_fact_expression(program, machine, state, *inner)
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
        .is_some_and(|predicate| predicate.holds_for(literal.as_bytes()))
}

pub(crate) fn domain_admits_empty_bytes(program: &TypedTrees, domain_symbol: SymbolHandle) -> bool {
    psi_typed_trees::byte_predicates::domain_byte_predicate(program, domain_symbol)
        .is_some_and(|predicate| predicate.holds_for(&[]))
}
