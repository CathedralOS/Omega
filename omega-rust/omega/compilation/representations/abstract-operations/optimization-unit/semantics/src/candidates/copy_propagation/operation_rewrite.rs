//! Exhaustive scalar-use and incoming-binding rewrite mechanics.
use optimization_unit::RedundantBlockParameterRewrite;
use semantic_vocabulary::ValueId;

pub(crate) fn rewrite_block_parameter_operation(
    operation: &mut abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use abstract_operations::AbstractOperation as O;

    let mut replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<abstract_operations::ValueBinding>| {
        for binding in bindings.iter_mut() {
            if binding.argument == patch.parameter {
                binding.argument = patch.replacement;
            }
        }
    };
match operation {
        O::Jump {
            target, bindings, ..
        } => {
            rewrite_bindings(bindings);
            if *target == patch.block {
                bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
            }
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            for successor in [when_true, when_false] {
                rewrite_bindings(&mut successor.bindings);
                if successor.target == patch.block {
                    successor
                        .bindings
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        O::StructuralCase { cases, .. } => {
            for successor in cases {
                if successor.target == patch.block {
                    successor
                        .payloads
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        // The receiving commitment names the completed source RHS. Until
        // this rewrite transports that correspondence through its validated
        // equality, retain the operand and let use validation keep its binding.
        O::WriteOnlyPrimitiveStore { range_obligation: Some(_), .. } => {}
        _ => crate::candidates::value_operand_rewrites::rewrite_value_operands(
            operation,
            &mut replace,
        ),
    }
}
