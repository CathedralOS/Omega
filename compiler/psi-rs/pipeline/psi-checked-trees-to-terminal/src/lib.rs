#![forbid(unsafe_code)]

//! Psi checked-semantics to terminal-Psi lowering for the first exact source slice.
//!
//! This producer proves a real source program can cross the terminal boundary
//! without retaining source trees. Its accepted surface is deliberately tiny
//! and exact; unsupported source constructs fail closed instead of being
//! dropped. General terminal lowering must widen this Psi-owned producer rather
//! than introduce an Omega-to-Psi stage.

use psi_checked_trees::{
    CheckedTrees,
    expression::{BinaryOperator, ExpressionNode},
    signature::SignatureContractKind,
    statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    types::PrimitiveType,
};
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{EvidenceRoute, PrimitiveJudgment};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, SemanticVersion,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};
use psi_typed_trees::domain::ProofFact;

/// Semantic module and separate replaceable proof artifact produced by the
/// Psi frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTerminalPsi {
    pub semantic_module: TerminalModule,
    pub proof_bundle: ProofBundle,
}

/// Lower one named checked free machine through the first terminal-Psi slice.
///
/// Accepted shape:
///
/// ```text
/// machine name() -> integer
/// requires L == L
/// ensures L == L
/// {
///     transition { _ -> done(L) }
///     state done(value: integer) -> integer { L }
/// }
/// ```
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let mut matches = checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == machine_name);
    let machine = matches
        .next()
        .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
    if matches.next().is_some() {
        return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
    }
    if machine.attached_data.is_some() {
        return unsupported("attached machines are not in the first terminal-Psi source slice");
    }
    if !machine.type_parameters.is_empty()
        || !machine.owned_data.is_empty()
        || !machine.satisfies.is_empty()
        || !machine.decreases.is_empty()
        || !machine.decrease_view_arguments.is_empty()
        || machine.decrease_range.is_valid()
        || !machine.service_reaches.is_empty()
        || !machine.invokes.is_empty()
        || machine.suspends
        || machine.blocks
        || machine.boundary
    {
        return unsupported("machine signature is outside the first terminal-Psi source slice");
    }

    let states = checked.machine_states(machine);
    let [entry_state, return_state] = states else {
        return unsupported("machine must contain exactly an entry state and one return state");
    };
    if !checked.state_parameters(entry_state).is_empty() {
        return unsupported("entry-state parameters are not supported");
    }
    let [return_parameter] = checked.state_parameters(return_state) else {
        return unsupported("return state must have exactly one parameter");
    };
    if return_parameter.is_self || return_parameter.is_const || return_parameter.is_mutable {
        return unsupported("qualified return-state parameters are not supported");
    }
    if !checked.state_contracts(entry_state).is_empty()
        || !checked.state_contracts(return_state).is_empty()
    {
        return unsupported("state contracts are not supported");
    }

    let return_type = integer_scalar_type(
        checked
            .primitive_type_reference(entry_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "machine result must be a primitive integer",
            ))?,
    )?;
    if integer_scalar_type(
        checked
            .primitive_type_reference(return_state.return_type)
            .ok_or(LoweringError::Unsupported(
                "return-state result must be a primitive integer",
            ))?,
    )? != return_type
        || integer_scalar_type(
            checked
                .primitive_type_reference(return_parameter.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "return-state parameter must be a primitive integer",
                ))?,
        )? != return_type
    {
        return unsupported("machine, return-state, and parameter types must match exactly");
    }

    let entry_statements = checked
        .statement_table
        .statements(entry_state.statement_nodes);
    let [StatementNode::Transition(transition)] = entry_statements else {
        return unsupported("entry state must contain exactly one transition");
    };
    if transition.guard != TransitionGuardNode::Always || transition.continuation.is_valid() {
        return unsupported("entry transition must be unconditional and have no continuation");
    }
    let TransitionTargetNode::Named { path, arguments } =
        checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("entry transition must target the return state by name");
    };
    if path.symbol != return_state.symbol {
        return unsupported("entry transition must target the sole return state");
    }
    let [argument] = checked.statement_table.expression_handles(*arguments) else {
        return unsupported("entry transition must carry exactly one argument");
    };
    let ExpressionNode::Integer(argument_literal) = checked.expression_table.expression(*argument)
    else {
        return unsupported("entry transition argument must be an integer literal");
    };
    let value = integer_value(argument_literal, return_type)?;

    let return_statements = checked
        .statement_table
        .statements(return_state.statement_nodes);
    let [StatementNode::Expression(return_expression)] = return_statements else {
        return unsupported("return state must contain exactly one value expression");
    };
    let ExpressionNode::Integer(return_literal) =
        checked.expression_table.expression(*return_expression)
    else {
        return unsupported("return state must return an integer literal");
    };
    if integer_value(return_literal, return_type)? != value {
        return unsupported("jump and return literals must be equal");
    }

    validate_contract(checked, machine, return_type, value)?;
    Ok(build_module(return_type, value))
}

fn validate_contract(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    result_type: ScalarType,
    expected_value: IntegerValue,
) -> Result<(), LoweringError> {
    let contracts = checked.machine_contracts(machine);
    if contracts.len() != 2 {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    for kind in [
        SignatureContractKind::Requires,
        SignatureContractKind::Ensures,
    ] {
        let contract = contracts
            .iter()
            .find(|contract| contract.kind == kind)
            .ok_or(LoweringError::Unsupported(
                "machine must have exactly one requires and one ensures clause",
            ))?;
        let facts = checked.proof_facts.span_or_empty(contract.facts);
        let [ProofFact::Expression(fact)] = facts else {
            return unsupported("each contract clause must contain exactly one expression fact");
        };
        let ExpressionNode::Binary(binary) = checked.expression_table.expression(*fact) else {
            return unsupported("contract facts must be equalities");
        };
        if binary.operator != BinaryOperator::Equal {
            return unsupported("contract facts must be equalities");
        }
        let (left_literal, right_literal) = match (
            checked.expression_table.expression(binary.left),
            checked.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => (left, right),
            _ => {
                return unsupported(
                    "contract facts must have the form `integer-literal == integer-literal`",
                );
            }
        };
        if integer_value(left_literal, result_type)? != expected_value
            || integer_value(right_literal, result_type)? != expected_value
        {
            return unsupported("contract literals must equal the executed literal");
        }
    }
    Ok(())
}

fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 | PrimitiveType::Addr => (IntegerSign::Unsigned, 64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return unsupported("only primitive integers are supported");
        }
    };
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| LoweringError::InvalidPsiIntegerType)
}

fn integer_value(
    literal: &psi_numerics::literals::IntegerLiteral,
    scalar_type: ScalarType,
) -> Result<IntegerValue, LoweringError> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return Err(LoweringError::InvalidPsiIntegerType);
    };
    let landing = literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?;
    if landing.landed_type.bit_width() != u32::from(integer_type.bits())
        || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
    {
        return Err(LoweringError::IntegerLandingMismatch);
    }
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(
            literal
                .value_i64()
                .map(i128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            literal
                .value_u64()
                .map(u128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
    };
    if !integer_type.admits(value) {
        return Err(LoweringError::IntegerLiteralOutsidePsiType);
    }
    Ok(value)
}

fn build_module(result_type: ScalarType, value: IntegerValue) -> LoweredTerminalPsi {
    let jump_constant_id = value_id(1);
    let parameter_id = value_id(2);
    let return_constant_id = value_id(3);
    let result_id = value_id(4);
    let ScalarType::Integer(integer_type) = result_type else {
        unreachable!("source slice accepts only integer results");
    };
    let literal = ScalarTerm::integer(integer_type, value)
        .expect("validated source literal fits its terminal integer type");
    let goal = Proposition::Equal(literal.clone(), literal);

    let obligation = obligation_id(1);
    LoweredTerminalPsi {
        semantic_module: TerminalModule {
            semantic_version: SemanticVersion::CURRENT,
            entry: machine_id(1),
            machines: vec![TerminalMachine {
                id: machine_id(1),
                parameters: Vec::new(),
                result: ValueDeclaration {
                    id: result_id,
                    scalar_type: result_type,
                },
                structural_places: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![
                    Block {
                        id: block_id(1),
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: operation_id(1),
                            result: ValueDeclaration {
                                id: jump_constant_id,
                                scalar_type: result_type,
                            },
                            kind: OperationKind::IntegerConstant { value },
                        }],
                        terminator: Terminator::Jump {
                            edge: edge_id(1),
                            target: block_id(2),
                            arguments: vec![jump_constant_id],
                        },
                    },
                    Block {
                        id: block_id(2),
                        parameters: vec![ValueDeclaration {
                            id: parameter_id,
                            scalar_type: result_type,
                        }],
                        operations: vec![Operation {
                            id: operation_id(2),
                            result: ValueDeclaration {
                                id: return_constant_id,
                                scalar_type: result_type,
                            },
                            kind: OperationKind::IntegerConstant { value },
                        }],
                        terminator: Terminator::Return {
                            edge: edge_id(2),
                            value: return_constant_id,
                        },
                    },
                ],
                contract: MachineContract {
                    id: contract_id(1),
                    requires: vec![goal.clone()],
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal,
                    }],
                },
            }],
        },
        proof_bundle: ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
            }],
        },
    }
}

fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    Unsupported(&'static str),
    InvalidPsiIntegerType,
    UnlandedIntegerLiteral,
    IntegerLandingMismatch,
    IntegerLiteralOutsideSupportedMagnitude,
    IntegerLiteralOutsidePsiType,
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
