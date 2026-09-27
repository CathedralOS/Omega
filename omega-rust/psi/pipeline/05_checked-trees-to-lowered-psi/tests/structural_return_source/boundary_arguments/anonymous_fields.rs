use super::{constructed_wrapper_source, record_field_computation};
use checked_trees::CheckedScalarComputationKind;
use checked_trees_to_lowered_psi::TerminalMachineSelection;

const EXACT_FIELDS: [&str; 4] = [
    "7 / 2 * 2",
    "7 / 2.0 * 2",
    "0.1 * 70",
    "18446744073709551616 / 3 * 3 - 18446744073709551609",
];

#[test]
fn constructed_anonymous_field_plans_cannot_change_the_authored_value() {
    for expression in EXACT_FIELDS {
        let source = constructed_wrapper_source("value: i64;", &format!("value: {expression}"));
        let mut changed = crate::front_end::checked_program(&source);
        let handle = record_field_computation(&changed);
        let retained = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle)
            .kind;
        *retained = CheckedScalarComputationKind::Value(
            checked_trees::CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(6).with_landing(
                    numerics::literals::IntegerLanding {
                        landed_type: numerics::literals::LandedIntegerType::I64,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                ),
            },
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "{source}"
        );
    }
}
