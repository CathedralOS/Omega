//! Build-prelude text for the closed optimization vocabulary.
//!
//! `optimization-core`'s `optimization_vocabulary!` declaration is the only
//! enumeration of case names and counter fields; this module owns the
//! Omega-source shape of the injected `Optimization`/`Optimizations` data and
//! the `enable`/`emit_report` machines, projected from `Optimization::ALL` so
//! a new variant cannot leave the injected vocabulary a row short.

use optimization_core::Optimization;

pub(super) fn declarations() -> String {
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

pub(super) fn enable_machine() -> String {
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

pub(super) const REPORT_MACHINE: &str = r#"pub machine Optimizations::emit_report(&mut self) {
    self.human_report = self.human_report + 1;
}
"#;
