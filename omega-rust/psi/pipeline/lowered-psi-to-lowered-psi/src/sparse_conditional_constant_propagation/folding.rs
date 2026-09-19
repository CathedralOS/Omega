//! Constant folding over literal-resolved scalar leaves.
//!
//! Iterating to a fixed point reaches the same unique maximal fold set a
//! single reverse-postorder pass computes: folding only adds literal bindings
//! and never removes one, so the scan order cannot change which leaves fold.
//! The independent verifier replays the relation in graph order.

use semantic_vocabulary::{ScalarType, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{OperationKind, TerminalMachine};
use terminal_semantics::ScalarLeafLiteral;

pub(super) fn fold(machine: &mut TerminalMachine) {
    // Ranking evidence is stated over this machine's exact execution rows;
    // leave it unchanged rather than reason about evidence outside its terms.
    if machine.ranked_scc.is_some() {
        return;
    }
    let mut value_types: BTreeMap<ValueId, ScalarType> = BTreeMap::new();
    for parameter in &machine.parameters {
        value_types.insert(parameter.id, parameter.scalar_type);
    }
    if let Some(result) = machine.result.scalar() {
        value_types.insert(result.id, result.scalar_type);
    }
    for block in &machine.blocks {
        for parameter in &block.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                value_types.insert(result.id, result.scalar_type);
            }
        }
    }
    let mut literals: BTreeMap<ValueId, ScalarLeafLiteral> = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let result = operation.result.scalar()?;
            let literal = match &operation.kind {
                OperationKind::IntegerConstant { value } => ScalarLeafLiteral::Integer(*value),
                OperationKind::BooleanConstant { value } => ScalarLeafLiteral::Boolean(*value),
                _ => return None,
            };
            Some((result.id, literal))
        })
        .collect();
    loop {
        let mut changed = false;
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                let Some(result) = operation.result.scalar() else {
                    continue;
                };
                if literals.contains_key(&result.id) {
                    continue;
                }
                // A static reach binder position is semantic call evidence
                // carried by the operation row, never a fold candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Some(literal) = terminal_semantics::constant_goal_free_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                ) else {
                    continue;
                };
                operation.kind = match literal {
                    ScalarLeafLiteral::Integer(value) => OperationKind::IntegerConstant { value },
                    ScalarLeafLiteral::Boolean(value) => OperationKind::BooleanConstant { value },
                };
                literals.insert(result.id, literal);
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Machine-level boundary cases a validated module cannot express.
    use super::{OperationKind, TerminalMachine, ValueId, fold};
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationResult, TerminalMachineResult,
        TerminalRankedScc, Terminator, ValueDeclaration,
    };

    fn i32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    }

    fn i32(ordinal: u64) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: i32_type(),
        }
    }

    fn boolean(ordinal: u64) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(ordinal).unwrap(),
            scalar_type: ScalarType::Boolean,
        }
    }

    fn machine(blocks: Vec<Block>) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: vec![i32(1)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(i32(9)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks,
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn block(ordinal: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(ordinal).unwrap(),
            parameters: Vec::new(),
            operations,
            terminator,
        }
    }

    fn constant(ordinal: u64, result: u64, value: i128) -> Operation {
        Operation {
            static_reach_binding: None,
            id: OperationId::new(ordinal).unwrap(),
            result: OperationResult::Scalar(i32(result)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(value),
            },
        }
    }

    fn add(ordinal: u64, result: u64, left: u64, right: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            id: OperationId::new(ordinal).unwrap(),
            result: OperationResult::Scalar(i32(result)),
            kind: OperationKind::WrappingIntegerAdd {
                left: ValueId::new(left).unwrap(),
                right: ValueId::new(right).unwrap(),
            },
        }
    }

    fn return_value(ordinal: u64, value: u64) -> Terminator {
        Terminator::Return {
            edge: EdgeId::new(ordinal).unwrap(),
            value: ValueId::new(value).unwrap(),
            cleanup_actions: Vec::new(),
        }
    }

    #[test]
    fn goal_bearing_operations_never_fold_even_with_literal_operands() {
        // A checked divide denotes a proof question, not a literal: literal
        // operands do not make it a constant computation.
        let mut machine = machine(vec![block(
            1,
            vec![
                constant(10, 10, 6),
                constant(11, 11, 3),
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(12).unwrap(),
                    result: OperationResult::Scalar(i32(12)),
                    kind: OperationKind::ExactIntegerDivide {
                        left: ValueId::new(10).unwrap(),
                        right: ValueId::new(11).unwrap(),
                        obligation: ObligationId::new(1).unwrap(),
                    },
                },
            ],
            return_value(1, 12),
        )]);
        let before = machine.clone();
        fold(&mut machine);
        assert_eq!(
            machine, before,
            "the obligation row is not a fold candidate"
        );
    }

    #[test]
    fn reach_bound_operation_is_never_folded() {
        let mut bound = add(12, 12, 10, 11);
        bound.static_reach_binding = Some(0);
        let mut machine = machine(vec![block(
            1,
            vec![constant(10, 10, 2), constant(11, 11, 3), bound],
            return_value(1, 12),
        )]);
        fold(&mut machine);
        assert!(
            matches!(
                machine.blocks[0].operations[2].kind,
                OperationKind::WrappingIntegerAdd { .. }
            ),
            "a static reach binder position is call evidence, not a leaf"
        );
    }

    #[test]
    fn block_parameters_are_not_literal_bindings() {
        // Edge arguments never seed the literal map: a parameter bound to the
        // same constant on every edge is still a parameter, so its consumers
        // do not fold. Constant-carrying edges belong to the control-flow
        // rule's lattice, not this bounded fold.
        let mut machine = machine(vec![
            block(
                1,
                vec![constant(10, 10, 7)],
                Terminator::Jump {
                    edge: EdgeId::new(1).unwrap(),
                    target: BlockId::new(2).unwrap(),
                    arguments: vec![ValueId::new(10).unwrap()],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            ),
            block(2, vec![add(20, 20, 41, 41)], return_value(2, 20)),
        ]);
        machine.blocks[1].parameters = vec![i32(41)];
        fold(&mut machine);
        assert!(
            matches!(
                machine.blocks[1].operations[0].kind,
                OperationKind::WrappingIntegerAdd { .. }
            ),
            "the parameter operand keeps the add unfolded"
        );
    }

    #[test]
    fn scan_order_cannot_change_the_fold_set() {
        // A validated module never declares a use before its definition; this
        // machine-level case proves the fixed point reaches the same set when
        // declaration order puts the constant producers last.
        let mut machine = machine(vec![block(
            1,
            vec![
                add(30, 30, 10, 11),
                constant(10, 10, 2),
                constant(11, 11, 3),
            ],
            return_value(1, 30),
        )]);
        fold(&mut machine);
        assert_eq!(
            machine.blocks[0].operations[0].kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(5)
            },
            "the second pass folds v30 once its operands are literals"
        );
    }

    #[test]
    fn ranked_machine_is_left_unchanged() {
        let mut machine = machine(vec![block(
            1,
            vec![constant(10, 10, 2), add(12, 12, 10, 10)],
            return_value(1, 12),
        )]);
        machine.ranked_scc = Some(TerminalRankedScc::Natural(Vec::new()));
        let before = machine.clone();
        fold(&mut machine);
        assert_eq!(machine, before, "ranking evidence freezes the machine");
    }

    #[test]
    fn non_scalar_results_and_wrong_denotations_do_not_fold() {
        // A Boolean literal bound to an integer-typed result is a malformed
        // leaf: the evaluator's shape check refuses it instead of folding.
        let mut machine = machine(vec![block(
            1,
            vec![
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(10).unwrap(),
                    result: OperationResult::Scalar(i32(10)),
                    kind: OperationKind::BooleanConstant { value: true },
                },
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(11).unwrap(),
                    result: OperationResult::Scalar(boolean(11)),
                    kind: OperationKind::WrappingIntegerAdd {
                        left: ValueId::new(1).unwrap(),
                        right: ValueId::new(1).unwrap(),
                    },
                },
            ],
            return_value(1, 10),
        )]);
        let before = machine.clone();
        fold(&mut machine);
        assert_eq!(
            machine, before,
            "shape-mismatched rows stay untouched rather than fold wrongly"
        );
    }
}
