//! Physical inputs to retained instructions and control, closed over edge bindings.
//! This drops no instruction, semantic argument, or charge. An unused scalar
//! binding needs no register copy, including a closed cycle of unused bindings.
use std::collections::{BTreeMap, BTreeSet};

use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarReturnValue, LegalizedScalarTerminator as Terminator,
};
use semantic_vocabulary::ValueId;

#[cfg(test)]
mod tests;

pub(crate) fn required_values(function: &LegalizedScalarFunction) -> BTreeSet<ValueId> {
    let mut pending = Vec::new();
    let mut incoming = BTreeMap::<ValueId, Vec<ValueId>>::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            instruction.visit_scalar_operands(|value| pending.push(value));
        }
        let mut bind = |successor: &legalized_operations::LegalizedScalarSuccessor| {
            for binding in &successor.bindings {
                incoming
                    .entry(binding.parameter)
                    .or_default()
                    .push(binding.argument);
            }
        };
        match &block.terminator {
            Terminator::Return(returned) => {
                if let LegalizedScalarReturnValue::Value { value, .. } = returned.value {
                    pending.push(value);
                }
            }
            Terminator::Jump { successor, .. } => bind(successor),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.push(*condition);
                bind(when_true);
                bind(when_false);
            }
            Terminator::Crash { .. } | Terminator::StructuralCase { .. } => {}
        }
    }
    let mut required = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if required.insert(value)
            && let Some(arguments) = incoming.get(&value)
        {
            pending.extend(arguments);
        }
    }
    required
}
