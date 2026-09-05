//! Scalar-graph module assembly regressions.

use super::*;

#[test]
fn scalar_machine_builder_uses_a_disjoint_module_identity_namespace() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let lowered = build_scalar_graph_module(
        &[LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Boolean {
                    expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
                },
            },
        }],
        ScalarType::Boolean,
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
    )
    .expect("a nonentry machine should lower in its disjoint identity range");

    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("the isolated builder emits one machine")
    };
    assert_eq!(machine.id, machine_id(2));
    assert_eq!(machine.contract.id, contract_id(2));
    assert_eq!(machine.entry, block_id(identity_base + 1));
    assert_eq!(machine.parameters[0].id, value_id(identity_base + 1));
    assert_eq!(
        machine
            .result
            .scalar()
            .expect("the scalar fixture has a result")
            .id,
        value_id(identity_base + 2)
    );
    let Terminator::Return { edge, value, .. } = machine.blocks[0].terminator else {
        panic!("the fixture should retain its scalar return")
    };
    assert_eq!(edge, edge_id(identity_base + 1));
    assert_eq!(value, value_id(identity_base + 1));
}

#[test]
fn primitive_scalar_source_jump_emits_empty_affine_cleanup() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let parameter_expression = || LoweredDirectExpression::Boolean {
        expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
    };
    let lowered = build_scalar_graph_module(
        &[
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    target: 1,
                    arguments: vec![parameter_expression()],
                },
            },
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean],
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Return {
                    expression: parameter_expression(),
                },
            },
        ],
        ScalarType::Boolean,
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
    )
    .expect("primitive scalar jump should lower");

    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &lowered.semantic_module.machines[0].blocks[0].terminator
    else {
        panic!("first scalar block should jump")
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn primitive_scalar_source_conditional_emits_empty_affine_cleanup() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let parameter_expression = || LoweredDirectExpression::Boolean {
        expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
    };
    let states = [
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Conditional {
                condition: LoweredBooleanReturnExpression::Parameter { position: 0 },
                when_true_target: 1,
                when_true_arguments: vec![parameter_expression()],
                when_false_target: 2,
                when_false_arguments: vec![parameter_expression()],
            },
        },
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: parameter_expression(),
            },
        },
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: parameter_expression(),
            },
        },
    ];
    let lowered = build_scalar_graph_module(
        &states,
        ScalarType::Boolean,
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
    )
    .expect("primitive scalar conditional should lower");

    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &lowered.semantic_module.machines[0].blocks[0].terminator
    else {
        panic!("first scalar block should branch")
    };
    assert!(when_true.trivial_affine_discards.is_empty());
    assert!(when_false.trivial_affine_discards.is_empty());
}
