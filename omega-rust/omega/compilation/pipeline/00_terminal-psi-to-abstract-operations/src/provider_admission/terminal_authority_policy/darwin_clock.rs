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

/// The named requirement has no settled canonical `DarwinClock` disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledDarwinClockRequirement {
    /// The schema is not the `DarwinClock` boundary, or the method is not one
    /// of its canonical requirements. No disposition is inferred from a
    /// readable name alone.
    UnknownRequirement,
    /// The member is canonical, but the mechanism is not the exact libSystem
    /// import it was reviewed against.
    UnreviewedMechanism,
}

/// The exact libSystem symbol a canonical `DarwinClock` member was reviewed
/// against, or `None` outside the cohort.
fn reviewed_symbol(schema: &ServiceSchema, method: &ServiceMethod) -> Option<&'static [u8]> {
    // The requirement owner is the trait's declaration path; an inherited
    // requirement of another boundary is not a cohort member.
    if schema.trait_name != DARWIN_CLOCK_TRAIT
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
pub fn darwin_clock_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Result<TerminalAuthorityPolicyRow, UnsettledDarwinClockRequirement> {
    let symbol = reviewed_symbol(schema, method)
        .ok_or(UnsettledDarwinClockRequirement::UnknownRequirement)?;
    if !is_reviewed_import(mechanism, symbol) {
        return Err(UnsettledDarwinClockRequirement::UnreviewedMechanism);
    }
    Ok(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes([]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn libsystem_import(symbol: &[u8]) -> TerminalMechanismIdentity {
        let locator = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: LIBSYSTEM_INSTALL_NAME.to_vec(),
                symbol: symbol.to_vec(),
            },
            target::TargetProfile::MacosArm64,
        )
        .expect("applicable Mach-O locator");
        effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            &locator,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([7; 32]),
        )
        .into()
    }

    fn schema(trait_name: &str, requirement_owner: &str, names: &[&str]) -> ServiceSchema {
        ServiceSchema {
            trait_name: trait_name.to_owned(),
            trait_package_identity: None,
            methods: names
                .iter()
                .map(|name| ServiceMethod {
                    completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                    name: (*name).to_owned(),
                    requirement_owner: requirement_owner.to_owned(),
                    requirement_identity: format!("{requirement_owner}::{name}"),
                    ..ServiceMethod::default()
                })
                .collect(),
        }
    }

    #[test]
    fn reviewed_darwin_clock_imports_classify_with_the_explicit_empty_disposition() {
        let schema = schema(
            "DarwinClock",
            "macos_gui::DarwinClock",
            &["clock_ns", "mach_absolute_time", "poll"],
        );
        let symbols: [&[u8]; 3] = [b"_clock_gettime_nsec_np", b"_mach_absolute_time", b"_poll"];
        for (method, symbol) in schema.methods.iter().zip(symbols) {
            let mechanism = libsystem_import(symbol);
            let row = darwin_clock_mechanism_row(mechanism, &schema, method)
                .expect("canonical DarwinClock import settles");
            assert_eq!(row.mechanism(), mechanism);
            assert!(row.disposition().is_authority_class_empty());
            // The row keys the exact mechanism in an ordinary receiving
            // policy; without it the same mechanism stays unclassified.
            let policy = super::super::terminal_authority_policy_with_rows(vec![row])
                .expect("one exact normalized-foreign row");
            assert!(
                policy
                    .classify(mechanism)
                    .expect("classified")
                    .is_authority_class_empty()
            );
            assert!(
                super::super::current_terminal_authority_policy()
                    .classify(mechanism)
                    .is_err()
            );
        }
    }

    #[test]
    fn names_alone_settle_nothing() {
        let canonical = schema("DarwinClock", "DarwinClock", &["poll"]);
        let poll = &canonical.methods[0];
        // The canonical member bound to another libSystem function.
        assert_eq!(
            darwin_clock_mechanism_row(libsystem_import(b"_unlink"), &canonical, poll),
            Err(UnsettledDarwinClockRequirement::UnreviewedMechanism)
        );
        // The reviewed symbol under a different install name.
        let other_library = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/local/lib/libpoll.dylib".to_vec(),
                symbol: b"_poll".to_vec(),
            },
            target::TargetProfile::MacosArm64,
        )
        .expect("applicable Mach-O locator");
        let other_library =
            effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
                &other_library,
                effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([7; 32]),
            )
            .into();
        assert_eq!(
            darwin_clock_mechanism_row(other_library, &canonical, poll),
            Err(UnsettledDarwinClockRequirement::UnreviewedMechanism)
        );
        // The reviewed import under another boundary, an inherited
        // requirement, or an unknown member.
        let other_boundary = schema("TimeHost", "TimeHost", &["poll"]);
        let inherited = schema("DarwinClock", "PosixWait", &["poll"]);
        let unknown = schema("DarwinClock", "DarwinClock", &["read_byte"]);
        for schema in [&other_boundary, &inherited, &unknown] {
            assert_eq!(
                darwin_clock_mechanism_row(libsystem_import(b"_poll"), schema, &schema.methods[0]),
                Err(UnsettledDarwinClockRequirement::UnknownRequirement),
                "{}::{} is not a DarwinClock cohort member",
                schema.trait_name,
                schema.methods[0].name
            );
        }
    }
}
