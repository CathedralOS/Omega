//! Replay demand by following each declared value forward to a real observer.
//! Independent of construction's backward worklist; cycles alone create no demand.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarReturnValue, LegalizedScalarTerminator as Terminator,
};
use semantic_vocabulary::ValueId;
use std::collections::BTreeSet;

pub(in crate::selection) fn required_values(
    function: &LegalizedScalarFunction,
) -> BTreeSet<ValueId> {
    let declared = function
        .parameters
        .iter()
        .map(|parameter| parameter.value)
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter().map(|parameter| parameter.value)),
        )
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .instructions
                .iter()
                .filter_map(|instruction| instruction.result.map(|result| result.value))
        }))
        .collect::<BTreeSet<_>>();
    declared
        .into_iter()
        .filter(|value| reaches_observer(function, *value))
        .collect()
}

fn reaches_observer(function: &LegalizedScalarFunction, value: ValueId) -> bool {
    let mut seen = BTreeSet::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        for block in &function.blocks {
            if block
                .instructions
                .iter()
                .any(|instruction| instruction.references_value(value))
            {
                return true;
            }
            let mut follow = |successor: &legalized_operations::LegalizedScalarSuccessor| {
                pending.extend(
                    successor
                        .bindings
                        .iter()
                        .filter(|binding| binding.argument == value)
                        .map(|binding| binding.parameter),
                );
            };
            match &block.terminator {
                Terminator::Return(returned) => {
                    if matches!(returned.value, LegalizedScalarReturnValue::Value { value: returned, .. } if returned == value)
                    {
                        return true;
                    }
                }
                Terminator::Jump { successor, .. } => follow(successor),
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    if *condition == value {
                        return true;
                    }
                    follow(when_true);
                    follow(when_false);
                }
                Terminator::Crash { .. } | Terminator::StructuralCase { .. } => {}
            }
        }
    }
    false
}
