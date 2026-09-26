//! Optimizer module role: executable entrance. Injected Omega optimization vocabulary.
//!
//! Both ordinary and filesystem-capable build preludes pass through this one
//! projection, which installs the closed case/counter schema and its exact
//! transition mapping without changing the surrounding build vocabulary.
//! `optimization-core`'s `optimization_vocabulary!` declaration owns the case
//! and counter enumeration; this module only projects it into prelude text.

mod fragments;

pub(super) const DECLARATIONS_SLOT: &str = "// compiler-owned optimization declarations\n";
pub(super) const ENABLE_SLOT: &str = "// compiler-owned optimization enable machine\n";
pub(super) const REPORT_SLOT: &str = "// compiler-owned optimization report machine\n";

pub fn install(base: &str) -> String {
    for slot in [DECLARATIONS_SLOT, ENABLE_SLOT, REPORT_SLOT] {
        assert_eq!(
            base.matches(slot).count(),
            1,
            "build prelude must contain exactly one `{}` slot",
            slot.trim()
        );
    }
    let declarations = fragments::declarations();
    let enable_machine = fragments::enable_machine();
    base.replacen(DECLARATIONS_SLOT, &declarations, 1)
        .replacen(ENABLE_SLOT, &enable_machine, 1)
        .replacen(REPORT_SLOT, fragments::REPORT_MACHINE, 1)
}
