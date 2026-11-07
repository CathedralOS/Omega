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
    let mut text = String::from("public data Optimization {\n");
    for optimization in Optimization::ALL {
        text.push_str(&format!("    case {};\n", optimization.build_case_name()));
    }
    // Counters are build bookkeeping: a defined-overflow domain keeps the
    // synthesized increments obligation-free. Saturating clamps at the type
    // maximum rather than wrapping a report count back to zero, and the
    // machines emit no `crashes Trap` bucket the verifier would demand.
    // `selected` holds one counter per optimization in `Optimization::ALL`
    // order. One array keeps every compile from checking one counter field
    // and one nested writer machine per optimization.
    text.push_str(&format!(
        "}}\npublic data Optimizations {{\n    human_report: u8 in Saturating;\n    selected: [u8 in Saturating; {}];\n}}\n",
        Optimization::ALL.len()
    ));
    text
}

fn enable_machine() -> String {
    let count = Optimization::ALL.len();
    let mut text = format!(
        "machine optimization_selection_slot(optimization: Optimization) -> u64\nensures result < {count}u64\n{{\n    match optimization {{\n"
    );
    for (slot, optimization) in Optimization::ALL.into_iter().enumerate() {
        text.push_str(&format!(
            "        Optimization::{} -> {slot}u64\n",
            optimization.build_case_name()
        ));
    }
    text.push_str(
        "    }\n}\npublic machine Optimizations::enable(optimizations: &mut Optimizations, optimization: Optimization) {\n    let slot: u64 = optimization_selection_slot(optimization);\n    optimizations.selected[slot] = optimizations.selected[slot] + 1;\n}\n",
    );
    text
}

const REPORT_MACHINE: &str = r#"public machine Optimizations::emit_report(optimizations: &mut Optimizations) {
    optimizations.human_report = optimizations.human_report + 1;
}
"#;
