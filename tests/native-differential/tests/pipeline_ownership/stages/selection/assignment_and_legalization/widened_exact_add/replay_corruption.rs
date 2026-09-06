//! Independent widened exact-add replay rejection for bridge and leaf custody corruption.

use crate::tests::*;

#[test]
fn widened_u8_exact_add_independent_replay_rejects_corrupted_bridge_custody() {
    let staged = staged_widened_u8_exact_add_conditional(NativeTarget::linux_x64());
    let original = staged.legalized().plan();
    let validate = |plan| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            plan,
        )
    };
    let false_fact = match original.functions[0].conditional().when_false.value {
        LegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } => accepted_fact,
        _ => panic!("fixture must retain its false-arm proof fact"),
    };

    macro_rules! corrupt_true_leaf {
        (|$value:ident| $body:block) => {{
            let mut corrupted = original.clone();
            let $value = &mut corrupted.functions[0].conditional_mut().when_true.value;
            $body
            assert_eq!(
                validate(corrupted),
                Err(LegalizationError::NonCanonicalLegalizedPlan)
            );
        }};
    }

    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { source_type, .. } = value else {
            unreachable!()
        };
        *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { target_type, .. } = value else {
            unreachable!()
        };
        *target_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } = value else {
            unreachable!()
        };
        *accepted_fact = false_fact;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { narrow_result, .. } = value else {
            unreachable!()
        };
        *narrow_result = ValueId::new(9_601).unwrap();
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            add_operation,
            widen_operation,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(add_operation, widen_operation);
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            add_definition_site,
            widen_definition_site,
            ..
        } = value
        else {
            unreachable!()
        };
        std::mem::swap(add_definition_site, widen_definition_site);
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { add_fuel, .. } = value else {
            unreachable!()
        };
        add_fuel[0].units += 1;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { widen_fuel, .. } = value else {
            unreachable!()
        };
        widen_fuel[0].units += 1;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd {
            left_temporary,
            right_temporary,
            ..
        } = value
        else {
            unreachable!()
        };
        *left_temporary = *right_temporary;
    });
    corrupt_true_leaf!(|value| {
        let LegalizedLeafValue::WidenedExactAdd { left, right, .. } = value else {
            unreachable!()
        };
        std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
    });
}
