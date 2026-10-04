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
