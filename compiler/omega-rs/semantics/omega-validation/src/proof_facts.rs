use crate::StateSignatureOwner;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::FactPlan;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::name::Identifier;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProofFactOwner<'program> {
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

impl fmt::Display for ProofFactOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        if membership.domain_symbol.is_valid() {
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
                if membership.domain_symbol.is_valid() {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown domain `{}`",
                    domain_path_label(program, membership.domain)
                )));
            }
        }
    }
}

fn is_boolean_fact_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
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
        | ExpressionNode::StructLiteral(_) => false,
    }
}

fn domain_path_label(
    program: &TypedTrees,
    domain: omega_core::arena::HandleSpan<Identifier>,
) -> String {
    let path = program.domain_path_members(domain);
    if path.is_empty() {
        return "<unknown>".to_owned();
    }

    path.iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
