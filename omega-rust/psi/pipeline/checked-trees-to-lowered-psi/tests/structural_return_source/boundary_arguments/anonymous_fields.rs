use super::*;
use semantic_vocabulary::IntegerValue;

const EXACT_FIELDS: [&str; 4] = [
    "7 / 2 * 2",
    "7 / 2.0 * 2",
    "0.1 * 70",
    "18446744073709551616 / 3 * 3 - 18446744073709551609",
];

#[test]
fn constructed_anonymous_fields_publish_exact_integer_values() {
    for expression in EXACT_FIELDS {
        let source = constructed_wrapper_source("value: i64;", &format!("value: {expression}"));
        let checked = checked(&source);
        let artifact = unit_wrapper_artifact(&checked);
        let module = decode_module(&artifact.0).expect("canonical Terminal module");
        let values = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::EstablishAffineScalarRecord { value, .. } => {
                    Some(value)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![&IntegerValue::Signed(7)], "{source}");
        // Exercise exact-fuel pauses, effect rejection/resume and transfer of
        // the new record owner through the existing wrapper execution harness.
        assert_constructed_wrapper_execution(&source);
    }
}

#[test]
fn constructed_anonymous_field_plans_cannot_change_the_authored_value() {
    for expression in EXACT_FIELDS {
        let source = constructed_wrapper_source("value: i64;", &format!("value: {expression}"));
        let mut changed = checked(&source);
        let retained = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                    value: checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                    ..
                } => Some(literal),
                _ => None,
            })
            .expect("retained exact field value");
        *retained = numerics::literals::IntegerLiteral::from_value(6).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::I64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "{source}"
        );
    }
}
