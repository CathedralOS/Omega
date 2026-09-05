//! Optimizer module role: executable entrance. Injected Omega optimization vocabulary.
//!
//! Both ordinary and filesystem-capable build preludes pass through this one
//! projection, which installs the closed case/counter schema and its exact
//! transition mapping without changing the surrounding build vocabulary.

mod fragments;

pub(super) const DECLARATIONS_SLOT: &str = "// compiler-owned optimization declarations\n";
pub(super) const ENABLE_SLOT: &str = "// compiler-owned optimization enable machine\n";
pub(super) const REPORT_SLOT: &str = "// compiler-owned optimization report machine\n";

pub(in crate::pipeline) fn install(base: &str) -> String {
    for slot in [DECLARATIONS_SLOT, ENABLE_SLOT, REPORT_SLOT] {
        assert_eq!(
            base.matches(slot).count(),
            1,
            "build prelude must contain exactly one `{}` slot",
            slot.trim()
        );
    }
    base.replacen(DECLARATIONS_SLOT, fragments::DECLARATIONS, 1)
        .replacen(ENABLE_SLOT, fragments::ENABLE_MACHINE, 1)
        .replacen(REPORT_SLOT, fragments::REPORT_MACHINE, 1)
}

#[cfg(test)]
mod tests {
    use super::{DECLARATIONS_SLOT, ENABLE_SLOT, REPORT_SLOT, install};

    #[test]
    fn exact_cases_map_to_their_canonical_counters() {
        let projected = install(&format!("{DECLARATIONS_SLOT}{ENABLE_SLOT}{REPORT_SLOT}"));
        for optimization in optimization_core::Optimization::ALL {
            let case = optimization.build_case_name();
            let counter = optimization.build_counter_field();
            assert!(projected.contains(&format!("case {case};")), "{case}");
            assert!(
                projected.contains(&format!("{counter}: u8 in Trapping;")),
                "{case} -> {counter}"
            );
            assert!(
                projected.contains(&format!("Optimization::{case} -> {counter}()")),
                "{case} -> {counter}"
            );
            assert!(
                projected.contains(&format!("self.{counter} = self.{counter} + 1;")),
                "{case} -> {counter}"
            );
        }
    }
}
