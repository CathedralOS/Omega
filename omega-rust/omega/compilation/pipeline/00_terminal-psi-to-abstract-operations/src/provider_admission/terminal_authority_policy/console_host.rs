//! Optimizer module role: policy table. Canonical settled `Console` and
//! `ProcessExit` cohort dispositions.
//!
//! The canonical `Console` boundary declared in `source/library/std/console.omg`
//! and the toolchain-owned `ProcessExit` requirement close over three terminal
//! authority classes: `exit_process` terminates the process, the byte and
//! line writers produce process output, and the byte and line readers consume
//! process input. On a target whose kernel entry is a compiler-known hosted
//! intrinsic the intrinsic classification in `classification.rs` names the same
//! classes; on a target whose kernel entry is DLL linkage (Windows: kernel32
//! `ExitProcess`) the leaf is a normalized foreign import, and this cohort
//! gives that mechanism the same disposition the intrinsic would carry.
//!
//! Requirement names only locate rows inside the canonical checked schema; the
//! emitted classification row is still keyed by the exact role-tagged
//! mechanism identity. A name outside the canonical cohort is not a
//! disposition: lookup fails closed.

use effects::{
    TerminalAuthorityClass, TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::ServiceMethod,
};

use super::TerminalAuthorityPolicyRow;

/// The single terminal authority class one canonical console requirement
/// exercises, or `None` for a name outside the cohort.
pub(crate) fn settled_console_cohort(name: &str) -> Option<TerminalAuthorityClass> {
    match name {
        "exit_process" => Some(TerminalAuthorityClass::ProcessTermination),
        "write" | "write_byte" | "write_line" => Some(TerminalAuthorityClass::ProcessOutput),
        "read_byte" | "read_line" => Some(TerminalAuthorityClass::ProcessInput),
        _ => None,
    }
}

/// Emit one exact receiving-policy row classifying the mechanism realization
/// admitted for the named `Console` or `ProcessExit` requirement. The row is
/// keyed by the exact mechanism identity; `method.name` only selects the
/// settled cohort's class.
pub(crate) fn console_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    let class = settled_console_cohort(&method.name)?;
    Some(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes([class]),
    ))
}
