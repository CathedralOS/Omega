use crate::tests::*;
use omega_selected_instructions_to_register_homes::LiteralFoldError;

use super::fixture::*;

#[test]
fn exact_subtract_rule_accepts_u12_max_and_rejects_the_first_value_above_it() {
    for (target, sole_view_name) in targets() {
        let admitted = run_selected_lowering_optimizations(source_with_values(
            target,
            sole_view_name,
            [5_000, 4_095],
            [6_000, 4_095],
            selected_lowering_budget(),
        ))
        .unwrap();
        let immediates = admitted.attempt().fold().transformed().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.kind {
                SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                    Some(immediate)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            immediates,
            [IntegerValue::Unsigned(4_095), IntegerValue::Unsigned(4_095)]
        );

        let first_over = || {
            run_selected_lowering_optimizations(source_with_values(
                target,
                sole_view_name,
                [5_000, 4_096],
                [6_000, 4_096],
                selected_lowering_budget(),
            ))
        };
        let first = first_over().unwrap_err();
        let repeated = first_over().unwrap_err();
        assert_eq!(first, repeated);
        assert_eq!(
            first,
            OptimizedLiteralFoldCustodyError::Fold(LiteralFoldError::UnsupportedImmediate {
                function: 0,
            })
        );
    }
}
