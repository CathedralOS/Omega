//! Optimizer module role: policy table. Canonical settled `DarwinClock`
//! cohort dispositions.
//!
//! `source/library/std/macos_gui.omg` declares `DarwinClock` as the darwin
//! clock seam: `clock_ns` and `mach_absolute_time` are raw clock reads and
//! `poll` is the millisecond timeout wait (`poll(NULL, 0, ms)`). The
//! `macos_arm64` leaves bind each one to a `MachODylibSymbol` import from
//! libSystem, so the realized mechanism is a normalized foreign call and the
//! receiving policy must classify it before the artifact exists. This is the
//! darwin twin of the `TimeHost` cohort in `time_host.rs`, whose `sleep` is
//! documented as bound by darwin `poll`: no closed `TerminalAuthorityClass`
//! covers a clock read or a timed wait, so every member settles to the
//! explicit empty classification.
//!
//! `poll` classifies the union over every permitted argument value, including
//! a nonzero `fds` word naming caller `pollfd` records. The call observes
//! descriptor readiness and writes the records' `revents`; it reads or writes
//! no file content and touches no other closed class. An empty row does not
//! claim custody safety for that caller-supplied address: the permission spec
//! keeps custody separate from authority.
//!
//! The reviewed disposition belongs to the physical function, not to a
//! spelling. The schema's boundary-trait name and the requirement name only
//! locate the cohort member; a row is emitted only when the mechanism is the
//! exact normalized libSystem import that member was reviewed against (same
//! target, locator identity and unconstrained argument contract), and the
//! row is keyed by that complete mechanism, calling-plan commitment
//! included. A same-named boundary in another package that binds some other
//! symbol, or the right symbol under another requirement, stays unclassified
//! and fails closed.

use effects::{
    NormalizedForeignArgumentContract, TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema},
};

use super::TerminalAuthorityPolicyRow;

/// The canonical boundary trait that owns this cohort.
const DARWIN_CLOCK_TRAIT: &str = "DarwinClock";
/// The install name every reviewed `DarwinClock` leaf imports from.
const LIBSYSTEM_INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";

/// The exact libSystem symbol a canonical `DarwinClock` member was reviewed
/// against, or `None` outside the cohort.
fn reviewed_symbol(schema: &ServiceSchema, method: &ServiceMethod) -> Option<&'static [u8]> {
    // The requirement owner is the trait's declaration path; an inherited
    // requirement of another boundary is not a cohort member.
    if schema.identity_text(schema.trait_name) != DARWIN_CLOCK_TRAIT
        || method.requirement_owner.rsplit("::").next() != Some(DARWIN_CLOCK_TRAIT)
    {
        return None;
    }
    match method.name.as_str() {
        "clock_ns" => Some(b"_clock_gettime_nsec_np"),
        "mach_absolute_time" => Some(b"_mach_absolute_time"),
        "poll" => Some(b"_poll"),
        _ => None,
    }
}

/// Whether `mechanism` is the unconstrained normalized `macos_arm64` import of
/// exactly `symbol` from libSystem.
fn is_reviewed_import(mechanism: TerminalMechanismIdentity, symbol: &[u8]) -> bool {
    let TerminalMechanismIdentity::NormalizedForeign(foreign) = mechanism else {
        return false;
    };
    let Ok(reviewed) = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: LIBSYSTEM_INSTALL_NAME.to_vec(),
            symbol: symbol.to_vec(),
        },
        target::TargetProfile::MacosArm64,
    ) else {
        return false;
    };
    foreign.target() == reviewed.target()
        && foreign.locator_identity() == reviewed.identity_digest()
        && foreign.argument_contract() == NormalizedForeignArgumentContract::AdmittedPlan
}

/// Emit one exact receiving-policy row classifying the mechanism admitted for
/// a canonical `DarwinClock` requirement with the explicit empty disposition.
pub(crate) fn darwin_clock_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    let symbol = reviewed_symbol(schema, method)?;
    is_reviewed_import(mechanism, symbol).then(|| {
        TerminalAuthorityPolicyRow::new(mechanism, TerminalAuthorityDisposition::from_classes([]))
    })
}
