//! Project the shared optimization catalog into the toolchain Build API.

use optimization_core::Optimization;

const DECLARATIONS_SLOT: &str = "// compiler-owned optimization declarations\n";
const ENABLE_SLOT: &str = "// compiler-owned optimization enable machine\n";
const REPORT_SLOT: &str = "// compiler-owned optimization report machine\n";

pub(super) fn install(base: &str) -> String {
    for slot in [DECLARATIONS_SLOT, ENABLE_SLOT, REPORT_SLOT] {
        assert_eq!(
            base.matches(slot).count(),
            1,
            "build prelude must contain exactly one `{}` slot",
            slot.trim()
        );
    }
    let declarations = declarations();
    let enable_machine = enable_machine();
    base.replacen(DECLARATIONS_SLOT, &declarations, 1)
        .replacen(ENABLE_SLOT, &enable_machine, 1)
        .replacen(REPORT_SLOT, REPORT_MACHINE, 1)
}

fn declarations() -> String {
    let mut text = String::from("pub data Optimization {\n");
    for optimization in Optimization::ALL {
        text.push_str(&format!("    case {};\n", optimization.build_case_name()));
    }
    // Counters are build bookkeeping: a defined-overflow domain keeps the
    // synthesized increments obligation-free. Saturating clamps at the type
    // maximum rather than wrapping a report count back to zero, and the
    // machines emit no `crashes Trap` bucket the verifier would demand.
    text.push_str("}\npub data Optimizations {\n    human_report: u8 in Saturating;\n");
    for optimization in Optimization::ALL {
        text.push_str(&format!(
            "    {}: u8 in Saturating;\n",
            optimization.build_counter_field()
        ));
    }
    text.push_str("}\n");
    text
}

fn enable_machine() -> String {
    let mut text = String::from(
        "pub machine Optimizations::enable(optimizations: &mut Optimizations, optimization: Optimization) {\n    transition optimization {\n",
    );
    for optimization in Optimization::ALL {
        text.push_str(&format!(
            "        Optimization::{} -> {}(optimizations)\n",
            optimization.build_case_name(),
            optimization.build_counter_field()
        ));
    }
    text.push_str("    }\n");
    for optimization in Optimization::ALL {
        let counter = optimization.build_counter_field();
        text.push_str(&format!(
            "\n    state {counter}(optimizations: &mut Optimizations) {{\n        optimizations.{counter} = optimizations.{counter} + 1;\n    }}\n"
        ));
    }
    text.push_str("}\n");
    text
}

const REPORT_MACHINE: &str = r#"pub machine Optimizations::emit_report(optimizations: &mut Optimizations) {
    optimizations.human_report = optimizations.human_report + 1;
}
"#;
