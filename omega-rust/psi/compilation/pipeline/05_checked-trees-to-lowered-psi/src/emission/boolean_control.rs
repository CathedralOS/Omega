//! Short-circuit Boolean decision lowering and terminal control emission.
use crate::emission::case_payload_dispatch::CaseDispatch;
use crate::emission::expression_validation::contains_short_circuit;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::boolean::emit_boolean_expression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::terminal_identities::{block_id, edge_id};
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{
    Block, StructuralCaseSuccessorEdge, SuccessorEdge, Terminator, ValueDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredBooleanDecision {
    Value(LoweredBooleanReturnExpression),
    Test {
        condition: LoweredBooleanReturnExpression,
        when_true: Box<LoweredBooleanDecision>,
        when_false: Box<LoweredBooleanDecision>,
    },
    /// A guard's whole-root case test that binds the selected payload; see
    /// `case_payload_dispatch`. Only guard decision emission realizes it.
    CaseDispatch {
        dispatch: CaseDispatch,
        when_true: Box<LoweredBooleanDecision>,
        when_false: Box<LoweredBooleanDecision>,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_boolean_decision<F>(
    decision: LoweredBooleanDecision,
    continuation: &F,
) -> LoweredBooleanDecision
where
    F: Fn(&LoweredBooleanReturnExpression) -> LoweredBooleanDecision,
{
    match decision {
        LoweredBooleanDecision::Value(expression) => continuation(&expression),
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
            when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
        },
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true,
            when_false,
        } => LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
            when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
        },
    }
}

fn branch_boolean_decision(
    decision: LoweredBooleanDecision,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
            if value {
                when_true
            } else {
                when_false
            }
        }
        LoweredBooleanDecision::Value(condition) => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
        LoweredBooleanDecision::Test {
            condition,
            when_true: nested_true,
            when_false: nested_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(branch_boolean_decision(
                *nested_true,
                when_true.clone(),
                when_false.clone(),
            )),
            when_false: Box::new(branch_boolean_decision(
                *nested_false,
                when_true,
                when_false,
            )),
        },
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true: nested_true,
            when_false: nested_false,
        } => LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true: Box::new(branch_boolean_decision(
                *nested_true,
                when_true.clone(),
                when_false.clone(),
            )),
            when_false: Box::new(branch_boolean_decision(
                *nested_false,
                when_true,
                when_false,
            )),
        },
    }
}

pub(crate) fn lower_boolean_control_decision(
    expression: &LoweredBooleanReturnExpression,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match expression {
        LoweredBooleanReturnExpression::And { left, right } => {
            let right = lower_boolean_control_decision(right, when_true, when_false.clone());
            lower_boolean_control_decision(left, right, when_false)
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let right = lower_boolean_control_decision(right, when_true.clone(), when_false);
            lower_boolean_control_decision(left, when_true, right)
        }
        LoweredBooleanReturnExpression::Not { operand } if contains_short_circuit(operand) => {
            lower_boolean_control_decision(operand, when_false, when_true)
        }
        expression if contains_short_circuit(expression) => branch_boolean_decision(
            lower_boolean_value_decision(expression),
            when_true,
            when_false,
        ),
        expression => LoweredBooleanDecision::Test {
            condition: expression.clone(),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
    }
}

pub(crate) fn lower_boolean_value_decision(
    expression: &LoweredBooleanReturnExpression,
) -> LoweredBooleanDecision {
    if !contains_short_circuit(expression) {
        return LoweredBooleanDecision::Value(expression.clone());
    }
    match expression {
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            lower_boolean_control_decision(
                expression,
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: true,
                }),
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: false,
                }),
            )
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            bind_boolean_decision(lower_boolean_value_decision(operand), &|operand| {
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Not {
                    operand: Box::new(operand.clone()),
                })
            })
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            bind_boolean_decision(lower_boolean_value_decision(left), &|left| {
                bind_boolean_decision(lower_boolean_value_decision(right), &|right| {
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Equal {
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    })
                })
            })
        }
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
        | LoweredBooleanReturnExpression::ScalarObservation { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
        | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => {
            unreachable!("non-short-circuit expressions return above")
        }
    }
}

pub(crate) fn boolean_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 1,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        }
        | LoweredBooleanDecision::CaseDispatch {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_block_count(when_true) + boolean_decision_block_count(when_false),
    }
}

/// The blocks a guard decision's emission reserves: one per test or case
/// dispatch, plus the entry block each case edge takes into an outcome that is
/// not its own reserved test: the selected case's into a shared successor, and
/// every unselected case's into the false outcome.
pub(crate) fn boolean_guard_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 0,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => {
            1 + boolean_guard_decision_block_count(when_true)
                + boolean_guard_decision_block_count(when_false)
        }
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true,
            when_false,
        } => {
            let selected_entry = usize::from(matches!(
                when_true.as_ref(),
                LoweredBooleanDecision::Value(_)
            ));
            let unselected_entries = dispatch.cases.len().saturating_sub(1);
            1 + selected_entry
                + unselected_entries
                + boolean_guard_decision_block_count(when_true)
                + boolean_guard_decision_block_count(when_false)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredBooleanDecisionTarget {
    pub(crate) block: BlockId,
    pub(crate) arguments: Vec<ValueId>,
}

#[allow(clippy::too_many_arguments)]
/// A fresh block a case edge enters with exactly `parameters`, jumping on to
/// `target` with its arguments. The arguments are values of the dispatching
/// block's namespace, which dominates this one.
fn reserve_case_entry(
    target: LoweredBooleanDecisionTarget,
    parameters: Vec<ValueDeclaration>,
    first_block_identity: u64,
    next_edge_identity: &mut u64,
    blocks: &mut Vec<Option<Block>>,
) -> LoweredBooleanDecisionTarget {
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved guard block count fits a semantic identity"),
            )
            .expect("reserved guard block identity advances"),
    );
    let edge = edge_id(*next_edge_identity);
    *next_edge_identity = next_edge_identity
        .checked_add(1)
        .expect("reserved guard case entry edge identity advances");
    blocks.push(Some(Block::plain(
        block,
        parameters,
        Terminator::Jump {
            edge,
            target: target.block,
            arguments: target.arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        },
    )));
    LoweredBooleanDecisionTarget {
        block,
        arguments: Vec::new(),
    }
}

fn emit_reserved_boolean_guard_decision_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> LoweredBooleanDecisionTarget {
    // These observation-only decisions inherit their parent's scope; neither
    // sibling nor caller may reuse lengths first emitted in a private child.
    let incoming_observations = all_operations.storage_observations.clone();
    let target = match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: true }) => {
            when_true_target.clone()
        }
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
            value: false,
        }) => when_false_target.clone(),
        LoweredBooleanDecision::Value(_) => {
            unreachable!("guard control decisions end in canonical Boolean choices")
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let block_index = blocks.len();
            let block = block_id(
                first_block_identity
                    .checked_add(
                        u64::try_from(block_index)
                            .expect("reserved guard block count fits a semantic identity"),
                    )
                    .expect("reserved guard block identity advances"),
            );
            blocks.push(None);
            let operation_start = all_operations.len();
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved guard false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved guard decision edge identities advance");
            let when_true = emit_reserved_boolean_guard_decision_blocks(
                when_true,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_guard_decision_blocks(
                when_false,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            blocks[block_index] = Some(Block {
                structural_parameters: Vec::new(),
                id: block,
                parameters: block_parameters,
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: all_operations[operation_start..operation_end].to_vec(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: true_edge,
                        target: when_true.block,
                        arguments: when_true.arguments,
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                },
            });
            LoweredBooleanDecisionTarget {
                block,
                arguments: Vec::new(),
            }
        }
        LoweredBooleanDecision::CaseDispatch {
            dispatch,
            when_true,
            when_false,
        } => {
            let block_index = blocks.len();
            let block = block_id(
                first_block_identity
                    .checked_add(
                        u64::try_from(block_index)
                            .expect("reserved guard block count fits a semantic identity"),
                    )
                    .expect("reserved guard block identity advances"),
            );
            blocks.push(None);
            // A case edge binds exactly its case's payload as the target
            // block's parameters: the selected block receives the payload and
            // every unselected case's block receives nothing. A nested test
            // reserves such a block itself; a selected outcome that is a
            // shared successor, and every unselected case, is entered through
            // a fresh block that jumps on.
            let payload_parameters = dispatch
                .payloads
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>();
            let when_true = if matches!(when_true.as_ref(), LoweredBooleanDecision::Value(_)) {
                let target = emit_reserved_boolean_guard_decision_blocks(
                    when_true,
                    parameters,
                    Vec::new(),
                    when_true_target,
                    when_false_target,
                    first_block_identity,
                    next_value_identity,
                    next_edge_identity,
                    all_operations,
                    blocks,
                );
                reserve_case_entry(
                    target,
                    payload_parameters,
                    first_block_identity,
                    next_edge_identity,
                    blocks,
                )
            } else {
                emit_reserved_boolean_guard_decision_blocks(
                    when_true,
                    parameters,
                    payload_parameters,
                    when_true_target,
                    when_false_target,
                    first_block_identity,
                    next_value_identity,
                    next_edge_identity,
                    all_operations,
                    blocks,
                )
            };
            let when_false = emit_reserved_boolean_guard_decision_blocks(
                when_false,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            assert!(
                when_true.arguments.is_empty() && when_false.arguments.is_empty(),
                "case dispatch successors bind only payload parameters"
            );
            let cases = dispatch
                .cases
                .iter()
                .map(|case| {
                    let selected = *case == dispatch.selected;
                    // Every unselected case enters its own block, as each case
                    // of a closed-sum dispatch does, before reaching the shared
                    // false outcome.
                    let target = if selected {
                        when_true.block
                    } else {
                        reserve_case_entry(
                            when_false.clone(),
                            Vec::new(),
                            first_block_identity,
                            next_edge_identity,
                            blocks,
                        )
                        .block
                    };
                    let edge = edge_id(*next_edge_identity);
                    *next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("reserved guard case edge identity advances");
                    StructuralCaseSuccessorEdge {
                        edge,
                        target,
                        case: *case,
                        payload_fields: if selected {
                            dispatch.payloads.iter().map(|(field, _)| *field).collect()
                        } else {
                            Vec::new()
                        },
                        cleanup_actions: Vec::new(),
                    }
                })
                .collect();
            blocks[block_index] = Some(Block::plain(
                block,
                block_parameters,
                Terminator::StructuralCase {
                    source: dispatch.source,
                    path: Vec::new(),
                    cases,
                },
            ));
            LoweredBooleanDecisionTarget {
                block,
                arguments: Vec::new(),
            }
        }
    };
    all_operations.storage_observations = incoming_observations;
    target
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_inlined_boolean_guard_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    source_block: BlockId,
    first_synthetic_block: BlockId,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
) -> (Block, Vec<Block>) {
    let first_reserved_identity = first_synthetic_block
        .get()
        .checked_sub(1)
        .expect("synthetic Boolean blocks follow source blocks");
    let mut reserved = Vec::new();
    let entry = emit_reserved_boolean_guard_decision_blocks(
        decision,
        parameters,
        block_parameters,
        when_true_target,
        when_false_target,
        first_reserved_identity,
        next_value_identity,
        next_edge_identity,
        all_operations,
        &mut reserved,
    );
    assert_eq!(entry.block.get(), first_reserved_identity);
    assert!(entry.arguments.is_empty());
    let mut reserved = reserved
        .into_iter()
        .map(|block| block.expect("every inlined Boolean guard block is finalized"));
    let mut root = reserved
        .next()
        .expect("short-circuit Boolean guard has a decision root");
    root.id = source_block;
    (root, reserved.collect())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_reserved_boolean_tuple_stage_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    next_stage: BlockId,
    carried_arguments: &[ValueId],
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    // These observation-only decisions inherit their parent's scope; neither
    // sibling nor caller may reuse lengths first emitted in a private child.
    let incoming_observations = all_operations.storage_observations.clone();
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved Boolean tuple block count fits a semantic identity"),
            )
            .expect("reserved Boolean tuple block identity advances"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let value = emit_boolean_expression(
                expression,
                parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("reserved Boolean tuple value edge identity advances");
            let mut arguments = carried_arguments.to_vec();
            arguments.push(value);
            (
                Terminator::Jump {
                    structural_arguments: Vec::new(),
                    edge,
                    target: next_stage,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    arguments,
                    cleanup_actions: Vec::new(),
                },
                all_operations.len(),
            )
        }
        LoweredBooleanDecision::CaseDispatch { .. } => {
            unreachable!("case dispatch is planned only for guard decisions")
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved Boolean tuple false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved Boolean tuple decision edges advance");
            let when_true = emit_reserved_boolean_tuple_stage_blocks(
                when_true,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_tuple_stage_blocks(
                when_false,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            (
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: true_edge,
                        target: when_true,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        cleanup_actions: Vec::new(),
                    },
                },
                operation_end,
            )
        }
    };
    blocks[block_index] = Some(Block {
        structural_parameters: Vec::new(),
        id: block,
        parameters: block_parameters,
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: all_operations[operation_start..operation_end].to_vec(),
        terminator,
    });
    all_operations.storage_observations = incoming_observations;
    block
}
