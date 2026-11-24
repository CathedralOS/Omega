//! Laws the mathematical kernel judges, through the `omega` command.
//!
//! Stage 06's own tests read the kernel's judgment and re-verify its
//! certificate. These run the same fixtures through every later stage, so an
//! accepted law also survives publication and a refused one reaches the user
//! as a diagnostic at its source position.
//!
//! The generic semiring machine once passed the checker and then failed
//! publication: the trait's type argument was recorded under the requirement
//! it instantiates, not under the machine that declares the record binder.

mod support;

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const CHECKER_FIXTURES: &str = "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures";
const UNPROVED: &str = "no available equality proves the stated Identity";

fn check(fixture: &str) -> (Output, String) {
    let directory: PathBuf = support::repository_root()
        .join(CHECKER_FIXTURES)
        .join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "main.omg"])
        .current_dir(directory)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn a_generic_machine_that_cites_its_semiring_law_compiles() {
    let (output, report) = check("semiring_identity/pass/cited");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_same_machine_without_the_citation_is_refused_at_its_ensures() {
    let (output, report) = check("semiring_identity/fail/uncited");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("core\\\\Identity::<T>(Ring::add(b, a), Ring::add(a, b))")
            && report.contains(UNPROVED)
            && report.contains("main.omg:4:9"),
        "{report}"
    );
}

#[test]
fn peano_addition_associativity_compiles_and_its_saved_program_names_the_kernel_type() {
    // One test for this fixture: two `omega` runs in one project directory
    // at the same time contend for its package lock, and one is refused.
    let (output, report) = check("ring_law/pass/ring_law_conformance");
    assert_eq!(output.status.code(), Some(0), "{report}");

    // Peano has no storage, so the saved program carries it only as a
    // mathematical carrier: its source type identity and its kernel type.
    let directory: PathBuf = support::repository_root()
        .join(CHECKER_FIXTURES)
        .join("ring_law/pass/ring_law_conformance");
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "kernel-judged-law-psi",
        false,
    )
    .unwrap();
    let artifact = scratch.path().join("ring_law_conformance.psi");
    let omega = |arguments: &[&std::ffi::OsStr]| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(arguments)
            .current_dir(&directory)
            .stdin(Stdio::null())
            .output()
            .expect("omega runs");
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(output.status.code(), Some(0), "{arguments:?}: {report}");
    };
    omega(&[
        "--emit-psi".as_ref(),
        artifact.as_os_str(),
        "main.omg".as_ref(),
    ]);
    omega(&[
        "psi".as_ref(),
        "inspect".as_ref(),
        "--artifact".as_ref(),
        artifact.as_os_str(),
    ]);

    let bytes = std::fs::read(&artifact).expect("the compiler saved the program");
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the saved program reopens");
    let module = saved.semantic_module();
    let [carrier] = module.types.mathematical_carriers.as_slice() else {
        panic!("one carrier: {:?}", module.types.mathematical_carriers)
    };
    let identity = module.identities.text(carrier.type_identity).unwrap();
    assert!(identity.contains("Peano"), "{identity}");
    let key = terminal_codec::decode_mathematical_certificate(&carrier.carrier_key).unwrap();
    assert!(
        key.arena
            .structurally_equal(key.certificate.term, key.certificate.expected)
    );
}

#[test]
fn a_satisfier_that_proves_nothing_is_refused_for_the_law_it_inherits() {
    let (output, report) = check("ring_law/fail/ring_law_unproven_rejected");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove the law `lawless` inherits from the requirement it satisfies: \
             core\\\\Identity::<Peano>(plus(plus(a, b), c), plus(a, plus(b, c)))"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:18:9"),
        "{report}"
    );
}
