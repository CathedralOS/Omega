use super::fixtures::{id, plan};
use crate::{
    OptimizationUnitIdentity, ScalarConstantFactIdentity, ValueRangeRegion, ValueRangeScope,
    ValueRangeSupport, reconstruct_psi_optimization_unit_seed, value_range_fact_identity,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractOperation, AbstractParameter,
    AbstractResult, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ValueId,
};

#[test]
fn block_parameters_keep_terminal_declaration_order() {
    let mut plan = plan();
    let function = &mut plan.functions[0];
    let entry = function.entry;
    let target = id(20, BlockId::new);
    // Deliberately descending identities prove this is declaration order,
    // not the previous BTreeMap order.
    let first_parameter = id(90, ValueId::new);
    let second_parameter = id(80, ValueId::new);
    let first_argument = function.parameters[0].value;
    let second_argument = id(70, ValueId::new);
    let scalar_type = function.parameters[0].scalar_type;
    function.parameters.push(AbstractParameter {
        value: second_argument,
        scalar_type,
    });
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: first_parameter,
        scalar_type,
    });
    function.block_entries = vec![
        AbstractBlockEntry {
            block: entry,
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            block: target,
            parameters: vec![
                AbstractParameter {
                    value: first_parameter,
                    scalar_type,
                },
                AbstractParameter {
                    value: second_parameter,
                    scalar_type,
                },
            ],
            operation_offset: 1,
        },
    ];
    function.operations = vec![
        AbstractOperation::Jump {
            psi_edge: id(60, EdgeId::new),
            target,
            bindings: vec![
                ValueBinding {
                    parameter: first_parameter,
                    argument: first_argument,
                    scalar_type,
                },
                ValueBinding {
                    parameter: second_parameter,
                    argument: second_argument,
                    scalar_type,
                },
            ],
            trivial_affine_discards: Vec::new(),
        },
        AbstractOperation::Return {
            psi_edge: id(61, EdgeId::new),
            result: first_parameter,
            value: first_parameter,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];

    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("ordered block parameters");
    assert_eq!(
        unit.functions[0].blocks[1]
            .parameters
            .iter()
            .map(|parameter| parameter.value)
            .collect::<Vec<_>>(),
        vec![first_parameter, second_parameter]
    );
}

#[test]
fn value_range_identity_rejects_malformed_type_support_and_region_axes() {
    let revision = OptimizationUnitIdentity::from_canonical_bytes(b"range revision");
    let machine = id(91, MachineId::new);
    let block = id(92, BlockId::new);
    let value = id(93, ValueId::new);
    let other_value = id(94, ValueId::new);
    let operation = id(95, OperationId::new);
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let support = ValueRangeSupport::ScalarConstant(
        ScalarConstantFactIdentity::from_canonical_bytes(b"range constant"),
    );
    let entire = ValueRangeRegion {
        revision,
        machine,
        value,
        scope: ValueRangeScope::EntireValue,
        dominated_blocks: Vec::new(),
    };
    let baseline = value_range_fact_identity(
        value,
        scalar_type,
        IntegerValue::Unsigned(1),
        IntegerValue::Unsigned(7),
        &support,
        &entire,
    )
    .expect("well-formed range identity");
    assert_ne!(
        baseline,
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(8),
            &support,
            &entire,
        )
        .unwrap()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                value: other_value,
                ..entire.clone()
            },
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(8),
            IntegerValue::Unsigned(7),
            &support,
            &entire,
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                scope: ValueRangeScope::DominatedOperationEntry {
                    block,
                    node: 1,
                    operation,
                },
                dominated_blocks: vec![block],
                ..entire.clone()
            },
        )
        .is_none()
    );
}
