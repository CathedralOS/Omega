// Splices the shared `Optimization` catalog into the toolchain Build prelude.
//
// Optimization counters use `u8 in Saturating` so synthesized increments are
// obligation-free, clamp at `u8::MAX` instead of wrapping a report count back
// to zero, and emit no `crashes Trap` bucket. Storing `selected` as a single
// array indexed in `Optimization::ALL` order avoids checking one field and one
// nested writer machine per optimization on every compile.

use optimization_core::Optimization;

const DECLARATIONS_SLOT: &str = "// compiler-owned optimization declarations\n";

pub(super) fn install(base: &str) -> String {
    assert_eq!(
        base.matches(DECLARATIONS_SLOT).count(),
        1,
        "build prelude must contain exactly one optimization declaration slot"
    );
    let declarations = declarations();
    base.replacen(DECLARATIONS_SLOT, &declarations, 1)
}

pub(super) fn module_source() -> String {
    format!(
        "module Optimizations;\n{}{}",
        enable_machine(),
        REPORT_MACHINE
    )
}

fn declarations() -> String {
    let mut text = String::from("public data Optimization {\n");
    for optimization in Optimization::ALL {
        text.push_str(&format!("    case {};\n", optimization.build_case_name()));
    }
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
        "    }\n}\npublic machine enable(optimizations: &mut Optimizations, optimization: Optimization) {\n    let slot: u64 = optimization_selection_slot(optimization);\n    optimizations.selected[slot] = optimizations.selected[slot] + 1;\n}\n",
    );
    text
}

const REPORT_MACHINE: &str = r#"public machine emit_report(optimizations: &mut Optimizations) {
    optimizations.human_report = optimizations.human_report + 1;
}
"#;
