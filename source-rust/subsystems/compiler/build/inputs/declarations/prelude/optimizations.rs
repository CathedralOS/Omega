// Splices the shared `Optimization` catalog into the toolchain Build prelude.
//
// Optimization counters use `u8 in Saturating` so synthesized increments are
// obligation-free, clamp at `u8::MAX` instead of wrapping a report count back
// to zero, and emit no `crashes Trap` bucket. Storing `selected` as a single
// array indexed in `Optimization::ALL` order avoids checking one field and one
// nested writer machine per optimization on every compile.

use optimization_selections::Optimization;

pub(super) fn module_source() -> String {
    let mut text = enable_machine();
    text.push_str(include_str!("modules/optimizations.omg"));
    text
}

pub(super) fn declarations() -> String {
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
            "        Optimization::{} -> {slot}u64,\n",
            optimization.build_case_name()
        ));
    }
    text.push_str(
        "    }\n}\npublic machine enable(optimizations: @mutable Optimizations, optimization: Optimization) {\n    let slot: u64 = optimization_selection_slot(optimization);\n    optimizations.selected[slot] = optimizations.selected[slot] + 1;\n}\n",
    );
    text
}
