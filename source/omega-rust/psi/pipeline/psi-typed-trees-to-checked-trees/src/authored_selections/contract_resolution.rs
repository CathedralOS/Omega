//! Checked reconstruction for declaration selections copied into proof/static
//! contracts.
//!
//! These expressions can lack executable operator-use facts and can retain
//! late-bound call symbols after contract cloning. Exact checked owners supply
//! the missing parameter/result types; ambiguous reconstruction stays closed.

use psi_checked_trees::{CheckFacts, ContractProofFactOwner};
use psi_symbols::SymbolHandle;
use psi_typed_trees::{TypedTrees, expression::ExpressionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CheckedContractOperatorResolution {
    Declaration(SymbolHandle),
    Builtin,
}

pub(super) fn checked_operator_resolution(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<CheckedContractOperatorResolution> {
    if checked_float_meaning_equality(program, facts, expression, node)
        || checked_resultless_law_equality(program, facts, expression, node)
    {
        return Some(CheckedContractOperatorResolution::Builtin);
    }
    checked_spelled_operator_resolution(program, facts, expression, node)
}

pub(super) fn checked_operand_type(
    program: &TypedTrees,
    facts: &CheckFacts,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    operand: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    super::authored_operand_type(program, operand)
        .or_else(|| {
            super::contexts::checked_expression_type_reference_from_exact_owner(
                program,
                facts,
                containing_expression,
                operand,
            )
        })
        .or_else(|| {
            let ExpressionNode::Call(call) = program.expression_table.expression(operand) else {
                return None;
            };
            super::exact_named_operator_call(program, call)
                .or_else(|| {
                    checked_named_operator_call(
                        program,
                        facts,
                        operand,
                        call,
                        program.expression_table.source_span(operand),
                    )
                })
                .map(|operator| operator.return_type)
        })
}

pub(super) fn checked_named_operator_call<'program>(
    program: &'program TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
    source_span: psi_source::SourceSpan,
) -> Option<&'program psi_typed_trees::operator::OperatorDefinition> {
    let arguments = program.expression_table.expression_handles(call.arguments);
    let operand_types = arguments
        .iter()
        .map(|argument| {
            super::authored_operand_type(program, *argument).or_else(|| {
                super::contexts::checked_expression_type_reference_from_exact_owner(
                    program, facts, expression, *argument,
                )
            })
        })
        .collect::<Vec<_>>();
    if operand_types.iter().all(Option::is_none) {
        return None;
    }

    let candidates = psi_typed_trees::operator::named_expression_call_candidates(program, call)
        .into_iter()
        .filter(|operator| {
            program
                .symbols
                .source_reference_can_see_symbol(source_span, operator.symbol)
        })
        .filter(|operator| {
            let parameters = program.operator_parameters(operator);
            parameters.len() == operand_types.len()
                && parameters
                    .iter()
                    .zip(&operand_types)
                    .all(|(parameter, actual)| {
                        actual.is_none_or(|actual| {
                            program.normalized_type_identity(actual)
                                == program.normalized_type_identity(parameter.type_reference)
                        })
                    })
        })
        .collect::<Vec<_>>();
    let [selected] = candidates.as_slice() else {
        return None;
    };
    Some(*selected)
}

/// Proof/static contract expressions do not always produce an executable
/// operator-use fact. When the checked owner supplies an operand type, replay
/// ordinary operand-driven spelled dispatch: one candidate selects that
/// declaration, no candidate means the already-validated compiler builtin,
/// and ambiguity remains unresolved.
fn checked_spelled_operator_resolution(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<CheckedContractOperatorResolution> {
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;

    let ExpressionNode::Binary(binary) = node else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    };
    let operand_types = [
        checked_operand_type(program, facts, expression, binary.left),
        checked_operand_type(program, facts, expression, binary.right),
    ];
    if operand_types.iter().all(Option::is_none) {
        return None;
    }
    let candidates =
        psi_typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types);
    match candidates.as_slice() {
        [] => Some(CheckedContractOperatorResolution::Builtin),
        [candidate] => Some(CheckedContractOperatorResolution::Declaration(
            candidate.operator.symbol,
        )),
        _ => None,
    }
}

/// D40 equality is a proof-only compiler intrinsic, not an authored runtime
/// operator. Admit only the sealed toolchain `FloatMeaning` carrier.
fn checked_float_meaning_equality(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> bool {
    let ExpressionNode::Binary(binary) = node else {
        return false;
    };
    if binary.operator != psi_typed_trees::expression::BinaryOperator::Equal {
        return false;
    }
    [binary.left, binary.right].into_iter().all(|operand| {
        checked_operand_type(program, facts, expression, operand).is_some_and(|type_reference| {
            psi_validation::is_exact_toolchain_float_meaning_type(program, type_reference)
        })
    })
}

/// A resultless signature contract is a theorem slot. Its `==` is proposition
/// equality over retained terms, including copies on exact realizing machines.
fn checked_resultless_law_equality(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> bool {
    if !matches!(
        node,
        ExpressionNode::Binary(binary)
            if binary.operator == psi_typed_trees::expression::BinaryOperator::Equal
    ) {
        return false;
    }

    let mut found = false;
    for (_, contract) in facts.proof.contract_facts.iter() {
        if !contract_contains_expression(program, contract.fact, expression) {
            continue;
        }
        let resultless = match contract.owner {
            ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol,
            } => resultless_state_signature(program, owner_symbol, state_symbol),
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } => resultless_machine_state(program, machine_symbol, state_symbol),
            ContractProofFactOwner::Machine { machine_symbol } => program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
                .and_then(|machine| program.machine_states(machine).first())
                .is_some_and(|state| type_reference_is_unit(program, state.return_type)),
            _ => false,
        };
        if !resultless {
            return false;
        }
        found = true;
    }
    found
}

fn contract_contains_expression(
    program: &TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.proof_facts.get(fact) {
        psi_typed_trees::domain::ProofFact::Expression(root) => {
            super::expression_contains(program, *root, expression, &mut Vec::new())
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
            super::expression_contains(program, membership.value, expression, &mut Vec::new())
        }
        psi_typed_trees::domain::ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .any(|root| super::expression_contains(program, *root, expression, &mut Vec::new())),
    }
}

fn resultless_state_signature(
    program: &TypedTrees,
    owner_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    let return_type = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == owner_symbol)
        .and_then(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.symbol == state_symbol)
                .map(|signature| signature.return_type)
        })
        .or_else(|| {
            program
                .machine_parameter_signature(state_symbol)
                .map(|(_, signature)| signature.return_type)
        });
    return_type.is_some_and(|return_type| type_reference_is_unit(program, return_type))
}

fn resultless_machine_state(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
        })
        .is_some_and(|state| type_reference_is_unit(program, state.return_type))
}

fn type_reference_is_unit(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        psi_typed_trees::types::TypeReferenceNode::Unit
    )
}
