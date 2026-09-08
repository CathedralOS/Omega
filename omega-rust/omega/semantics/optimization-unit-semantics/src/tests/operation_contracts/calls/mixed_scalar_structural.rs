use crate::tests::*;
use optimization_unit::{ValueDefinition, ValueDefinitionSite};

#[test]
fn mixed_structural_scalar_call_validates_ordered_types_and_dominating_uses() {
    let baseline = mixed_call();
    validate_psi_optimization_unit(&baseline).unwrap();
    let read = &baseline.functions[0].blocks[0].nodes[0];
    assert_eq!(
        read.uses
            .iter()
            .map(|usage| usage.value)
            .collect::<Vec<_>>(),
        vec![value(501), value(502), value(501)]
    );

    for mutation in 0..7 {
        let mut changed = baseline.clone();
        let caller = &mut changed.functions[0];
        let block = &mut caller.blocks[0];
        let call = &mut block.nodes[0];
        let AbstractOperation::CallStructuralScalar {
            arguments,
            result,
            structural_arguments,
            ..
        } = &mut call.operation
        else {
            panic!("mixed call")
        };
        match mutation {
            0 => {
                arguments.pop();
            }
            1 => arguments.push(value(501)),
            2 => arguments.swap(0, 1),
            3 => arguments[0] = value(599),
            4 => arguments[1] = result.value,
            5 => result.scalar_type = count_type(),
            6 => structural_arguments[0].access = terminal_psi::StructuralAccess::Owned,
            _ => panic!("bounded mutations"),
        }
        call.uses = arguments
            .iter()
            .map(|argument| ValueUse {
                value: *argument,
                block: block.id,
                node: 0,
            })
            .collect();
        changed.identity = recompute_psi_optimization_unit_identity(&changed);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn mixed_structural_scalar_call_rewrites_all_scalar_occurrences_only() {
    let unit = mixed_call();
    let original = unit.functions[0].blocks[0].nodes[0].operation.clone();
    let mut expected = original.clone();
    let AbstractOperation::CallStructuralScalar { arguments, .. } = &mut expected else {
        panic!("mixed call")
    };
    *arguments = vec![value(590), value(502), value(590)];
    let mut substitution = original.clone();
    crate::candidates::rewrite_scalar_value_uses(&mut substitution, value(501), value(590));
    assert_eq!(substitution, expected);
    let mut parameter_rewrite = original;
    crate::candidates::rewrite_block_parameter_operation(
        &mut parameter_rewrite,
        optimization_unit::RedundantBlockParameterRewrite {
            machine: unit.functions[0].machine,
            block: unit.functions[0].entry,
            position: 0,
            parameter: value(501),
            replacement: value(590),
            scalar_type: count_type(),
        },
    );
    assert_eq!(parameter_rewrite, expected);
}

#[test]
fn mixed_structural_scalar_call_unit_counterpart_keeps_scalar_uses() {
    let mut unit = structural_call_unit();
    unit.functions[0].parameters = vec![parameter(501, ScalarType::Boolean, 0)];
    unit.functions[1].parameters = vec![parameter(503, ScalarType::Boolean, 0)];
    let block = &mut unit.functions[0].blocks[0];
    let call = &mut block.nodes[0];
    let AbstractOperation::CallUnit { arguments, .. } = &mut call.operation else {
        panic!("Unit call")
    };
    *arguments = vec![value(501)];
    call.uses = vec![ValueUse {
        value: value(501),
        block: block.id,
        node: 0,
    }];
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    let mut rewritten = unit.functions[0].blocks[0].nodes[0].operation.clone();
    let mut expected = rewritten.clone();
    let AbstractOperation::CallUnit { arguments, .. } = &mut expected else {
        panic!("Unit call")
    };
    *arguments = vec![value(590)];
    crate::candidates::rewrite_scalar_value_uses(&mut rewritten, value(501), value(590));
    assert_eq!(rewritten, expected);
}

fn mixed_call() -> PsiOptimizationUnit {
    let mut unit = projected_shared_structural_scalar_call_unit();
    unit.functions[0].parameters = vec![
        parameter(501, count_type(), 0),
        parameter(502, ScalarType::Boolean, 1),
    ];
    unit.functions[1].parameters = vec![
        parameter(503, count_type(), 0),
        parameter(504, ScalarType::Boolean, 1),
        parameter(505, count_type(), 2),
    ];
    let block = &mut unit.functions[0].blocks[0];
    let call = &mut block.nodes[0];
    let AbstractOperation::CallStructuralScalar { arguments, .. } = &mut call.operation else {
        panic!("mixed call")
    };
    *arguments = vec![value(501), value(502), value(501)];
    call.uses = arguments
        .iter()
        .map(|argument| ValueUse {
            value: *argument,
            block: block.id,
            node: 0,
        })
        .collect();
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

fn parameter(ordinal: u64, scalar_type: ScalarType, position: u32) -> ValueDefinition {
    ValueDefinition {
        value: value(ordinal),
        scalar_type,
        site: ValueDefinitionSite::FunctionParameter(position),
    }
}

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
