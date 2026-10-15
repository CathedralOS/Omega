//! Physical inputs to retained instructions and control, closed over edge bindings.
//! This drops no instruction, semantic argument, or charge. An unused scalar
//! binding needs no register copy, including a closed cycle of unused bindings.
use std::collections::{BTreeMap, BTreeSet};

use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarReturnValue, LegalizedScalarTerminator as Terminator,
};
use semantic_vocabulary::ValueId;

pub(crate) fn required_values(function: &LegalizedScalarFunction) -> BTreeSet<ValueId> {
    let mut pending = Vec::new();
    let mut incoming = BTreeMap::<ValueId, Vec<ValueId>>::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            instruction.visit_scalar_operands(|value| pending.push(value));
        }
        let mut bind = |successor: &legalized_operations::LegalizedScalarSuccessor,
                        pending: &mut Vec<ValueId>| {
            // These coordinates are physical reads even when no instruction
            // or ordinary scalar successor parameter otherwise uses them.
            pending.extend(
                successor
                    .structural_bindings
                    .iter()
                    .flat_map(|binding| binding.indices.iter().map(|index| index.operand.value)),
            );
            for binding in &successor.bindings {
                incoming
                    .entry(binding.parameter)
                    .or_default()
                    .push(binding.argument);
            }
        };
        match &block.terminator {
            Terminator::ExternalCompletion { arguments, .. } => {
                pending.extend(
                    arguments
                        .iter()
                        .map(|argument| argument.source.source_value()),
                );
            }
            Terminator::Return(returned) | Terminator::ReturnCleanup { returned, .. } => {
                if let LegalizedScalarReturnValue::Value { value, .. } = returned.value {
                    pending.push(value);
                }
            }
            Terminator::Jump { successor, .. } => bind(successor, &mut pending),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.push(*condition);
                bind(when_true, &mut pending);
                bind(when_false, &mut pending);
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
