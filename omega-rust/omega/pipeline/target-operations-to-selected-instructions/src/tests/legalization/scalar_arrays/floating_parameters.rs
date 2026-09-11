//! Float-bank entry parameters retain IEEE identity in integer-ABI array results.
use super::*;
use abstract_operations::AbstractParameter;
use semantic_vocabulary::IeeeFloatFormat;

#[test]
fn floating_array_parameters_replay_exact_scalar_type_and_distinct_aggregate_abi() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar_type = ScalarType::IeeeFloat(format);
        let (mut source, _, _) = fixture(0);
        let StructuralTypeShape::FixedArray { length, .. } =
            &mut source.structural_types.make_mut()[0].shape
        else {
            unreachable!();
        };
        *length = 1;
        source.structural_types.make_mut()[1].shape =
            StructuralTypeShape::PrimitiveScalar(scalar_type);
        let incoming = ValueId::new(10).unwrap();
        let function = &mut source.functions[0];
        function.parameters = vec![AbstractParameter {
            value: incoming,
            scalar_type,
        }];
        let O::EstablishScalarArray { elements, .. } = &mut function.operations[0] else {
            unreachable!();
        };
        *elements = vec![incoming];
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            target::NativeTarget::linux_x64(),
        )
        .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let graph = &target.functions[0].graph;
        assert_eq!(
            graph.call_plan.parameters[0].shape.class,
            calling_conventions::ValueClass::Float
        );
        assert_eq!(
            graph.call_plan.result.as_ref().unwrap().shape.class,
            calling_conventions::ValueClass::Integer
        );
        for mutation in 0..4 {
            let mut changed = target.clone();
            let graph = &mut changed.functions[0].graph;
            if mutation == 0 {
                graph.call_plan.parameters[0].shape.class =
                    calling_conventions::ValueClass::Integer;
            } else {
                let TargetUnitOperation::EstablishScalarArray { elements, .. } =
                    &mut graph.blocks[0].operations[0]
                else {
                    unreachable!();
                };
                match mutation {
                    1 => elements[0] = ValueId::new(99).unwrap(),
                    2 => graph.scalar_parameters[0].scalar_type = ScalarType::Boolean,
                    _ => {
                        graph.scalar_parameters[0].scalar_type =
                            ScalarType::IeeeFloat(if format == IeeeFloatFormat::Binary32 {
                                IeeeFloatFormat::Binary64
                            } else {
                                IeeeFloatFormat::Binary32
                            })
                    }
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
