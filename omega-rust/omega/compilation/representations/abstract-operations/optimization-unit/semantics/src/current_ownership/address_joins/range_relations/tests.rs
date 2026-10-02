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
    let relations = RangeRelations::new(&function, &BTreeMap::new());
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
    assert!(
        RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(3), 0),
            value(6),
            value(7)
        )
    );
    let O::Conditional { when_false, .. } = &mut function.blocks[0].nodes[1].operation else {
        panic!()
    };
    *when_false = successor(2, &[(4, 2), (5, 1)]);
    let relations = RangeRelations::new(&function, &BTreeMap::new());
    assert!(!relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_or_equal((block(3), 0), value(7), value(6)));
}

#[test]
fn range_relations_kill_stale_comparisons_and_captured_endpoint_values() {
    let mut function = guarded_states();
    function.blocks[0].nodes.insert(1, node(constant(1)));
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(3), 0),
            value(6),
            value(7)
        )
    );
    let mut function = guarded_states();
    function.blocks[2].nodes[0] = node(constant(6));
    let relations = RangeRelations::new(&function, &BTreeMap::new());
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
    let relations = RangeRelations::new(&function, &BTreeMap::new());
    assert!(relations.proves_less_than((block(2), 0), value(2), value(1)));
    assert!(!relations.proves_less_than((block(2), 0), value(1), value(2)));
    assert!(!relations.values_unchanged_since((block(1), 1), (block(2), 0), &[value(1)]));
}

#[test]
fn range_relations_require_index_types_complete_bindings_and_acyclic_support() {
    let mut function = guarded_states();
    function.parameters[0].scalar_type = ScalarType::Boolean;
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(3), 0),
            value(6),
            value(7)
        )
    );
    let mut function = guarded_states();
    let O::Jump { bindings, .. } = &mut function.blocks[1].nodes[0].operation else {
        panic!()
    };
    bindings.pop();
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(3), 0),
            value(6),
            value(7)
        )
    );
    let mut function = guarded_states();
    function.blocks[2].nodes[1] = node(jump(3, &[(6, 6), (7, 7)]));
    let relations = RangeRelations::new(&function, &BTreeMap::new());
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
    let relations = RangeRelations::new(&function, &BTreeMap::new());
    assert!(relations.proves_less_than((block(3), 0), value(6), value(7)));
    assert!(!relations.proves_less_than((block(2), 0), value(4), value(5)));
    // Merely evaluating a comparison establishes no fact until selected.
}

fn contract(
    requires: Vec<Proposition>,
    ensures: Vec<Proposition>,
) -> terminal_psi::MachineContract {
    terminal_psi::MachineContract {
        id: semantic_vocabulary::ContractId::new(100).unwrap(),
        crash_routes: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        requires,
        ensures: ensures
            .into_iter()
            .enumerate()
            .map(|(index, proposition)| terminal_psi::ContractClause {
                obligation: semantic_vocabulary::ObligationId::new(index as u64 + 1).unwrap(),
                proposition,
            })
            .collect(),
        outcome_specific_ensures: Vec::new(),
    }
}
fn term(number: u64) -> semantic_vocabulary::ScalarTerm {
    semantic_vocabulary::ScalarTerm::value(value(number), scalar_type())
}
fn literal(number: u64, amount: u128) -> O {
    let mut operation = constant(number);
    let O::IntegerConstant { value, .. } = &mut operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(amount);
    operation
}

#[test]
fn range_relations_seed_only_current_verified_entry_values() {
    let mut function = guarded_states();
    function.parameters = vec![parameter(2, None, 0), parameter(3, None, 1)];
    function.blocks = vec![body(1, &[], vec![literal(1, 1), finish()])];
    function.verified_contract = Some(contract(
        vec![Proposition::LessThan(term(1), term(2))],
        Vec::new(),
    ));
    // A body-defined value with the same ID is not an invocation input.
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(1), 1),
            value(1),
            value(2)
        )
    );
    function.verified_contract = Some(contract(
        vec![Proposition::LessThan(term(2), term(3))],
        Vec::new(),
    ));
    assert!(
        RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(1), 1),
            value(2),
            value(3)
        )
    );
    function.verified_contract = None;
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(1), 1),
            value(2),
            value(3)
        )
    );
    function.verified_contract = Some(contract(
        vec![Proposition::LessThan(term(2), term(3))],
        Vec::new(),
    ));
    function.blocks[0].nodes[0] = node(literal(2, 4));
    assert!(
        !RangeRelations::new(&function, &BTreeMap::new()).proves_less_than(
            (block(1), 1),
            value(2),
            value(3)
        )
    );
}

fn contract_call() -> (PsiOptimizationFunction, PsiOptimizationFunction) {
    let mut caller = guarded_states();
    caller.parameters.clear();
    let mut callee = guarded_states();
    callee.machine = MachineId::new(202).unwrap();
    callee.parameters = vec![parameter(21, None, 0), parameter(22, None, 1)];
    callee.result =
        abstract_operations::AbstractFunctionResult::Scalar(abstract_operations::AbstractResult {
            value: value(23),
            scalar_type: scalar_type(),
        });
    callee.verified_contract = Some(contract(
        Vec::new(),
        vec![Proposition::LessOrEqual(term(22), term(23))],
    ));
    caller.blocks = vec![body(
        1,
        &[],
        vec![
            literal(1, 1),
            literal(2, 2),
            O::Call {
                psi_operation: OperationId::new(200).unwrap(),
                result: value(30),
                scalar_type: scalar_type(),
                callee: callee.machine,
                arguments: vec![value(1), value(2)],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            finish(),
        ],
    )];
    (caller, callee)
}
fn call_proves_bound(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    index: u32,
) -> bool {
    RangeRelations::new(caller, &BTreeMap::from([(callee.machine, callee)])).proves_less_than(
        (block(1), index),
        value(1),
        value(30),
    )
}

#[test]
fn range_relations_instantiate_current_call_formals_and_result_after_return() {
    let (mut caller, callee) = contract_call();
    assert!(!call_proves_bound(&caller, &callee, 2));
    assert!(call_proves_bound(&caller, &callee, 3));
    caller.blocks[0].nodes[2].operation = O::CallStructural {
        psi_operation: OperationId::new(200).unwrap(),
        callee: callee.machine,
        arguments: vec![value(1), value(2)],
        structural_arguments: Vec::new(),
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        result: abstract_operations::AbstractStructuralCallResult::Scalar(
            abstract_operations::AbstractResult {
                value: value(30),
                scalar_type: scalar_type(),
            },
        ),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    assert!(call_proves_bound(&caller, &callee, 3));
    caller.blocks[0].nodes.insert(3, node(literal(30, 0)));
    assert!(!call_proves_bound(&caller, &callee, 4));
}

#[test]
fn range_relations_reject_stale_or_unbound_call_contracts() {
    let (caller, callee) = contract_call();
    for mutation in 0..8 {
        let mut caller = caller.clone();
        let mut callee = callee.clone();
        let O::Call {
            callee: target,
            arguments,
            scalar_type,
            ..
        } = &mut caller.blocks[0].nodes[2].operation
        else {
            unreachable!()
        };
        match mutation {
            0 => callee.verified_contract = None,
            1 => callee.verified_contract.as_mut().unwrap().ensures.clear(),
            2 => *target = MachineId::new(203).unwrap(),
            3 => arguments.swap(0, 1),
            4 => arguments.pop().map(|_| ()).unwrap(),
            5 => *scalar_type = ScalarType::Boolean,
            // The unknown callee value happens to equal the caller result ID.
            6 => {
                callee.verified_contract.as_mut().unwrap().ensures[0].proposition =
                    Proposition::LessOrEqual(term(22), term(30))
            }
            7 => arguments[1] = value(99),
            _ => unreachable!(),
        }
        assert!(
            !call_proves_bound(&caller, &callee, 3),
            "mutation {mutation}"
        );
    }
}
