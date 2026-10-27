//! Checked reconstruction for declaration selections copied into proof/static
//! contracts.
//!
//! These expressions can lack executable operator-use facts and can retain
//! late-bound call symbols after contract cloning. Exact checked owners supply
//! the missing parameter/result types; ambiguous reconstruction stays closed.

use checked_trees::{CheckFacts, ContractProofFactOwner};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckedContractOperatorResolution {
    Declaration(SymbolHandle),
    Builtin,
}

/// `owner_index` is the caller's exact-owner index, when it holds one for this
/// program and these facts; without it each operand query rescans the program.
pub(crate) fn checked_operator_resolution(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<CheckedContractOperatorResolution> {
    // Closed content equations select proof algebra equality, not an
    // executable overload. Their normalized source plan authenticates the
    // exact equation and its owner; composed terms have no runtime type.
    if validation::content_conservation_plans(program)
        .iter()
        .any(|plan| plan.source_expression == expression)
    {
        return Some(CheckedContractOperatorResolution::Builtin);
    }
    if checked_float_meaning_equality(program, facts, expression, node, owner_index)
        || checked_resultless_law_equality(program, facts, expression, node, owner_index)
    {
        return Some(CheckedContractOperatorResolution::Builtin);
    }
    checked_spelled_operator_resolution(program, facts, expression, node, owner_index)
}

pub(super) fn checked_operand_type(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    containing_expression: typed_trees::expression::ExpressionHandle,
    operand: typed_trees::expression::ExpressionHandle,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    crate::authored_selections::operator_targets::authored_operand_type(program, operand)
        .or_else(|| {
            validation::expression_owners::checked_expression_type_reference_from_exact_owner(
                program,
                &facts.proof.contract_facts,
                containing_expression,
                operand,
                owner_index,
            )
        })
        .or_else(|| {
            let call = program.expression_table.call_node(operand)?;
            validation::selection_resolution::exact_named_operator_call(program, call)
                .or_else(|| {
                    checked_named_operator_call(
                        program,
                        facts,
                        operand,
                        call,
                        program.expression_table.source_span(operand),
                        owner_index,
                    )
                })
                .map(|operator| operator.return_type)
        })
}

pub(crate) fn checked_named_operator_call<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    source_span: source::SourceSpan,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    let arguments = program.expression_table.expression_handles(call.arguments);
    let operand_types = arguments
        .iter()
        .map(|argument| {
            crate::authored_selections::operator_targets::authored_operand_type(program, *argument)
                .or_else(|| {
                    validation::expression_owners::checked_expression_type_reference_from_exact_owner(
                        program,
                        &facts.proof.contract_facts,
                        expression,
                        *argument,
                        owner_index,
                    )
                })
        })
        .collect::<Vec<_>>();
    if operand_types.iter().all(Option::is_none) {
        return None;
    }

    let candidates = typed_trees::operator::named_expression_call_candidates(program, call)
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<CheckedContractOperatorResolution> {
    // Proof-static operators have no executable use event. Their exact checked
    // contract owner supplies the same operand types for binary, indexed and
    // range forms; an indexed member must not depend on a runtime origin merely
    // to retain the declaration whose formation checking already admitted.
    let (spelling, operands) = match node {
        ExpressionNode::Binary(binary) => (
            typed_trees::operator::binary_operator_spelling(binary.operator)?,
            vec![binary.left, binary.right],
        ),
        ExpressionNode::Indexed(indexed) => {
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    language_core::OperatorSpelling::Range,
                    vec![indexed.collection, range.start, range.end],
                ),
                _ => (
                    language_core::OperatorSpelling::Index,
                    vec![indexed.collection, indexed.index],
                ),
            }
        }
        _ => return None,
    };
    let operand_types = operands
        .into_iter()
        .map(|operand| checked_operand_type(program, facts, expression, operand, owner_index))
        .collect::<Vec<_>>();
    if operand_types.iter().all(Option::is_none) {
        return None;
    }
    let candidates = typed_trees::operator::resolve_spelling_for_operands(
        program,
        spelling,
        &operand_types,
        program.expression_table.source_span(expression),
    );
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> bool {
    let ExpressionNode::Binary(binary) = node else {
        return false;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return false;
    }
    [binary.left, binary.right].into_iter().all(|operand| {
        checked_operand_type(program, facts, expression, operand, owner_index).is_some_and(
            |type_reference| {
                validation::is_exact_toolchain_float_meaning_type(program, type_reference)
            },
        )
    })
}

/// A resultless signature contract is a theorem slot. Its `==` is proposition
/// equality over retained terms, including copies on exact realizing machines.
fn checked_resultless_law_equality(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> bool {
    if !matches!(
        node,
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Equal
    ) {
        return false;
    }

    let containing = match owner_index {
        Some(owner_index) => owner_index.containing_contract_facts(expression),
        None => facts
            .proof
            .contract_facts
            .iter()
            .filter(|(_, contract)| {
                contract_contains_expression(program, contract.fact, expression)
            })
            .map(|(handle, _)| handle)
            .collect(),
    };
    let mut found = false;
    for handle in containing {
        let contract = facts.proof.contract_facts.get(handle);
        let resultless = match contract.owner {
            ContractProofFactOwner::CallSignature { .. } => {
                crate::proof::contracts::call_signatures::signature(&facts.proof, contract.owner)
                    .is_some_and(|signature| type_reference_is_unit(program, signature.return_type))
            }
            ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol,
            } => resultless_state_signature(program, owner_symbol, state_symbol),
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } => resultless_machine_state(program, machine_symbol, state_symbol),
            ContractProofFactOwner::Machine { machine_symbol } => program
                .machine_by_symbol(machine_symbol)
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
    program: &dyn ::validation::CheckingInput,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    validation::expression_owners::proof_fact_contains_expression(program, fact, expression)
}

fn resultless_state_signature(
    program: &dyn ::validation::CheckingInput,
    owner_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    let return_type = program
        .trait_definition_by_symbol(owner_symbol)
        .and_then(|definition| {
            program
                .trait_machine_signature_by_symbol(definition, state_symbol)
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
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    program
        .machine_by_symbol(machine_symbol)
        .and_then(|machine| program.state_in_machine(machine, state_symbol))
        .is_some_and(|state| type_reference_is_unit(program, state.return_type))
}

fn type_reference_is_unit(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        typed_trees::types::TypeReferenceNode::Unit
    )
}
