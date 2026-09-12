//! Stores use the value selected by the exact receiving block's scalar telescope.
use super::*;
use abstract_operations::{AbstractBlockEntry, AbstractSuccessor, ValueBinding};
use semantic_vocabulary::{BlockId, EdgeId};

#[test]
fn branch_join_store_replays_exact_block_value_on_four_targets() {
    for scalar in [
        ScalarType::Boolean,
        integer(IntegerSign::Unsigned, 64),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
    ] {
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (mut source, _, _) = fixture(native, scalar, false);
            let function = &mut source.functions[0];
            let joined = ValueId::new(90).unwrap();
            let condition = ValueId::new(91).unwrap();
            let alternative = ValueId::new(92).unwrap();
            let join = BlockId::new(90).unwrap();
            let mut store = function.operations[0].clone();
            let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } = &mut store else {
                panic!("store fixture");
            };
            let first = value.value;
            value.value = joined;
            function.parameters.extend([
                AbstractParameter {
                    value: alternative,
                    scalar_type: scalar,
                },
                AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                },
            ]);
            let successor = |edge, source| AbstractSuccessor {
                psi_edge: EdgeId::new(edge).unwrap(),
                target: join,
                bindings: vec![ValueBinding {
                    parameter: joined,
                    argument: source,
                    scalar_type: scalar,
                }],
                structural_bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            };
            let returned = function.operations.last().unwrap().clone();
            function.operations = vec![
                AbstractOperation::Conditional {
                    condition,
                    when_true: successor(90, first),
                    when_false: successor(91, alternative),
                },
                store,
                returned,
            ];
            function.block_entries.push(AbstractBlockEntry {
                block: join,
                parameters: vec![AbstractParameter {
                    value: joined,
                    scalar_type: scalar,
                }],
                structural_parameters: Vec::new(),
                operation_offset: 1,
            });
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            for mutation in ["block", "value", "type"] {
                let mut changed = target.clone();
                let TargetUnitOperation::WriteOnlyPrimitiveStore {
                    source: TargetUnitWriteOnlyPrimitiveStoreSource::BlockParameter(value),
                    ..
                } = &mut changed.functions[0].graph.blocks[1].operations[0]
                else {
                    panic!("exact block store");
                };
                match mutation {
                    "block" => value.block = source.functions[0].entry,
                    "value" => value.value = alternative,
                    "type" => {
                        value.scalar_type = if scalar == ScalarType::Boolean {
                            integer(IntegerSign::Unsigned, 8)
                        } else {
                            ScalarType::Boolean
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
                assert!(
                    validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
            }
        }
    }
}
