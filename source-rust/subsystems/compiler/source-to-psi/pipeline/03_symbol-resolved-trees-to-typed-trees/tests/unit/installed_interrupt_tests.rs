//! The bundled installer and its registration operands must keep their source custody.

use crate::qualified_operator_home_tests::{bundled_core_sources, packages};

#[test]
fn only_the_bundled_installer_requires_interrupt_registration_operands() {
    let edited = "public data X86StaticInterruptScope {} public machine X86StaticInterruptScope::run() -> bool { true }";
    let call = "use {omega}/language/core/static_interrupt.omg; machine main() -> bool { |static_interrupt|::X86StaticInterruptScope::run() }";
    let ordinary = packages(
        &[(0, "static_interrupt.omg", edited), (31, "main.omg", call)],
        &[],
    )
    .expect("an edited toolchain file can declare an ordinary machine under that name");
    assert!(
        ordinary
            .expression_table
            .iter_backing_expressions()
            .any(|(_, expression)| {
                matches!(expression, crate::source_state::expression::ExpressionNode::Call(call)
                    if call.installed_interrupt_operation.is_none())
            })
    );

    let files = bundled_core_sources();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((31, "main.omg", call));
    let error =
        packages(&units, &[]).expect_err("the exact installer requires its registration operands");
    assert!(
        error.contains(
            "interrupt installation needs its exact registrar application and timer slot"
        ),
        "{error}"
    );
}
