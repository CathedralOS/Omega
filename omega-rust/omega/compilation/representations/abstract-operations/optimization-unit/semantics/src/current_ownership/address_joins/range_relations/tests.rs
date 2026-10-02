use super::*;
use abstract_operations::AbstractSuccessor;
use optimization_unit::{
    EffectLink, OptimizationBlock, OptimizationNode, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::{EdgeId, IntegerValue, OperationId};

fn value(number: u64) -> ValueId {
    ValueId::new(number).unwrap()
}
fn block(number: u64) -> BlockId {
    BlockId::new(number).unwrap()
}
fn scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn parameter(number: u64, owner: Option<u64>, position: u32) -> ValueDefinition {
    ValueDefinition {
        value: value(number),
        scalar_type: scalar_type(),
        site: owner.map_or(ValueDefinitionSite::FunctionParameter(position), |owner| {
            ValueDefinitionSite::BlockParameter {
                block: block(owner),
                position,
            }
        }),
    }
}
fn bindings(pairs: &[(u64, u64)]) -> Vec<ValueBinding> {
    pairs
        .iter()
        .map(|(parameter, argument)| ValueBinding {
            parameter: value(*parameter),
            argument: value(*argument),
            scalar_type: scalar_type(),
        })
        .collect()
}
fn successor(target: u64, pairs: &[(u64, u64)]) -> AbstractSuccessor {
    AbstractSuccessor {
        psi_edge: EdgeId::new(target).unwrap(),
        target: block(target),
        bindings: bindings(pairs),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}
fn jump(target: u64, pairs: &[(u64, u64)]) -> O {
    O::Jump {
        psi_edge: EdgeId::new(target).unwrap(),
        target: block(target),
        bindings: bindings(pairs),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}
fn finish() -> O {
    O::ReturnUnit {
        psi_edge: EdgeId::new(99).unwrap(),
        cleanup_actions: Vec::new(),
    }
}
fn constant(number: u64) -> O {
    O::IntegerConstant {
        psi_operation: OperationId::new(number + 100).unwrap(),
        result: value(number),
        scalar_type: scalar_type(),
        value: IntegerValue::Unsigned(0),
    }
}
fn node(operation: O) -> OptimizationNode {
    OptimizationNode {
        operation,
        provenance: Vec::new(),
        fuel: Vec::new(),
        effect: EffectLink {
            input: 0,
            output: 0,
        },
        definitions: Vec::new(),
        uses: Vec::new(),
        successors: Vec::new(),
        ownership: Vec::new(),
    }
}
fn body(number: u64, parameters: &[u64], operations: Vec<O>) -> OptimizationBlock {
    OptimizationBlock {
        id: block(number),
        structural_parameters: Vec::new(),
        parameters: parameters
            .iter()
            .enumerate()
            .map(|(position, value)| parameter(*value, Some(number), position as u32))
            .collect(),
        nodes: operations.into_iter().map(node).collect(),
    }
}
fn guarded_states() -> PsiOptimizationFunction {
    let mut function = crate::tests::fixtures::scalar_units::unit()
        .functions
        .remove(0);
    function.entry = block(1);
    function.parameters = vec![parameter(1, None, 0), parameter(2, None, 1)];
    function.blocks = vec![
        body(
            1,
            &[],
            vec![
                O::IntegerLessThan {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: value(3),
                    left: value(1),
                    right: value(2),
                },
                O::Conditional {
                    condition: value(3),
                    when_true: successor(2, &[(4, 2), (5, 1)]),
                    when_false: successor(4, &[]),
                },
            ],
        ),
        body(2, &[4, 5], vec![jump(3, &[(6, 5), (7, 4)])]),
        body(3, &[6, 7], vec![constant(9), finish()]),
        body(4, &[], vec![finish()]),
    ];
    function
}

#[test]
fn range_relations_follow_guard_through_reordered_state_bindings() {
    let function = guarded_states();
    let relations = RangeRelations::new(&function);
    assert!(relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(relations.proves_less_or_equal((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_than((block(3), 0), value(7), value(6)));
    assert!(relations.proves_less_or_equal((block(4), 0), value(2), value(1)));
    assert!(!relations.proves_less_than((block(4), 0), value(2), value(1)));
    assert!(relations.values_unchanged_since((block(3), 0), (block(3), 1), &[value(6), value(7)]));
    assert!(!relations.values_unchanged_since((block(2), 0), (block(3), 1), &[value(6)]));
}

#[test]
fn range_relations_intersect_all_arrivals_and_ignore_unreachable_predecessors() {
    let mut function = guarded_states();
    function
        .blocks
        .push(body(5, &[], vec![jump(3, &[(6, 2), (7, 1)])]));
    assert!(RangeRelations::new(&function).proves_less_than((block(3), 0), value(6), value(7)));
    let O::Conditional { when_false, .. } = &mut function.blocks[0].nodes[1].operation else {
        panic!()
    };
    *when_false = successor(2, &[(4, 2), (5, 1)]);
    let relations = RangeRelations::new(&function);
    assert!(!relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_or_equal((block(3), 0), value(7), value(6)));
}

#[test]
fn range_relations_kill_stale_comparisons_and_captured_endpoint_values() {
    let mut function = guarded_states();
    function.blocks[0].nodes.insert(1, node(constant(1)));
    assert!(!RangeRelations::new(&function).proves_less_than((block(3), 0), value(6), value(7)));
    let mut function = guarded_states();
    function.blocks[2].nodes[0] = node(constant(6));
    let relations = RangeRelations::new(&function);
    assert!(relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_than((block(3), 1), value(6), value(7)));
    assert!(!relations.values_unchanged_since((block(3), 0), (block(3), 1), &[value(6)]));
    assert!(!relations.values_unchanged_since((block(2), 0), (block(4), 0), &[value(1)]));
}

#[test]
fn range_relations_parallel_swap_reads_the_pre_edge_values() {
    let mut function = guarded_states();
    let O::Conditional { when_true, .. } = &mut function.blocks[0].nodes[1].operation else {
        panic!()
    };
    *when_true = successor(2, &[(1, 2), (2, 1)]);
    function.blocks[1] = body(2, &[1, 2], vec![finish()]);
    let relations = RangeRelations::new(&function);
    assert!(relations.proves_less_than((block(2), 0), value(2), value(1)));
    assert!(!relations.proves_less_than((block(2), 0), value(1), value(2)));
    assert!(!relations.values_unchanged_since((block(1), 1), (block(2), 0), &[value(1)]));
}

#[test]
fn range_relations_require_index_types_complete_bindings_and_acyclic_support() {
    let mut function = guarded_states();
    function.parameters[0].scalar_type = ScalarType::Boolean;
    assert!(!RangeRelations::new(&function).proves_less_than((block(3), 0), value(6), value(7)));
    let mut function = guarded_states();
    let O::Jump { bindings, .. } = &mut function.blocks[1].nodes[0].operation else {
        panic!()
    };
    bindings.pop();
    assert!(!RangeRelations::new(&function).proves_less_than((block(3), 0), value(6), value(7)));
    let mut function = guarded_states();
    function.blocks[2].nodes[1] = node(jump(3, &[(6, 6), (7, 7)]));
    let relations = RangeRelations::new(&function);
    assert!(!relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.values_unchanged_since((block(3), 0), (block(3), 1), &[value(6), value(7)]));
}

#[test]
fn range_relations_transport_the_comparison_before_selecting_its_branch() {
    let mut function = guarded_states();
    let mut carried = bindings(&[(4, 1), (5, 2)]);
    carried.push(ValueBinding {
        parameter: value(8),
        argument: value(3),
        scalar_type: ScalarType::Boolean,
    });
    let mut transfer = jump(2, &[]);
    let O::Jump { bindings, .. } = &mut transfer else {
        panic!()
    };
    *bindings = carried;
    function.blocks[0].nodes[1] = node(transfer);
    function.blocks[1] = body(
        2,
        &[4, 5],
        vec![O::Conditional {
            condition: value(8),
            when_true: successor(3, &[(6, 4), (7, 5)]),
            when_false: successor(4, &[]),
        }],
    );
    let mut boolean = parameter(8, Some(2), 2);
    boolean.scalar_type = ScalarType::Boolean;
    function.blocks[1].parameters.push(boolean);
    let relations = RangeRelations::new(&function);
    assert!(relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_than((block(2), 0), value(4), value(5)));
    // Merely evaluating a comparison establishes no fact until selected.
}
