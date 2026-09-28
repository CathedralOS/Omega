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
    text.push_str("}\npub data Optimizations {\n    human_report: u8 in Trapping;\n");
    for optimization in Optimization::ALL {
        text.push_str(&format!(
            "    {}: u8 in Trapping;\n",
            optimization.build_counter_field()
        ));
    }
    text.push_str("}\n");
    text
}

fn enable_machine() -> String {
    let mut text = String::from(
        "pub machine Optimizations::enable(&mut self, optimization: Optimization) {\n    transition optimization {\n",
    );
    for optimization in Optimization::ALL {
        text.push_str(&format!(
            "        Optimization::{} -> {}()\n",
            optimization.build_case_name(),
            optimization.build_counter_field()
        ));
    }
    text.push_str("    }\n");
    for optimization in Optimization::ALL {
        let counter = optimization.build_counter_field();
        text.push_str(&format!(
            "\n    state {counter}(&mut self) {{\n        self.{counter} = self.{counter} + 1;\n    }}\n"
        ));
    }
    text.push_str("}\n");
    text
}

const REPORT_MACHINE: &str = r#"pub machine Optimizations::emit_report(&mut self) {
    self.human_report = self.human_report + 1;
}
"#;
