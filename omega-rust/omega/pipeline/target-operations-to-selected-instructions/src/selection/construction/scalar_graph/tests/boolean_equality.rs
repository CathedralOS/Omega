//! Boolean equality shares compare instructions, never integer semantic types.
use super::*;
use legalized_operations::LegalizedScalarComparison as Comparison;

#[test]
fn boolean_comparison_replay_rejects_operand_carrier_and_ordering_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let mut source = control::graph(target, Comparison::Equal, true);
        let entry = source
            .blocks
            .iter_mut()
            .find(|block| block.id == source.entry_block)
            .unwrap();
        for (ordinal, row) in entry.instructions[..2].iter_mut().enumerate() {
            row.result.as_mut().unwrap().scalar_type = ScalarType::Boolean;
            row.kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(ordinal as u128));
        }
        let LegalizedScalarInstructionKind::Compare { operand_type, .. } =
            &mut entry.instructions[2].kind
        else {
            unreachable!();
        };
        *operand_type = ScalarType::Boolean;
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &LegalizedScalarFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                candidate,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source).unwrap();
        for mutation in 0..6 {
            let mut changed = source.clone();
            let entry = changed
                .blocks
                .iter_mut()
                .find(|block| block.id == changed.entry_block)
                .unwrap();
            let LegalizedScalarInstructionKind::Compare {
                predicate,
                operand_type,
                left,
                right,
            } = &mut entry.instructions[2].kind
            else {
                unreachable!();
            };
            match mutation {
                0 => *left = *right,
                1 => {
                    *operand_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
                }
                2 => {
                    *operand_type =
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                }
                3 => *right = ValueId::new(999).unwrap(),
                4 | 5 => {
                    *left = *right;
                    *predicate = if mutation == 4 {
                        Comparison::LessThan
                    } else {
                        Comparison::LessOrEqual
                    };
                }
                _ => unreachable!(),
            }
            assert!(validate(&changed).is_err(), "mutation {mutation}");
            if mutation != 0 {
                assert!(
                    build(
                        0,
                        &changed,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints()
                    )
                    .is_err(),
                    "mutation {mutation}"
                );
            }
        }
    }
}
