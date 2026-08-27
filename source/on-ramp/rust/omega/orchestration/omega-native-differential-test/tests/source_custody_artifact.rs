//! Product-owned semantic comparator for the private bootstrap artifact fixture.
//!
//! The bootstrap gate supplies the exact source path. This test deliberately
//! knows nothing about CKIR1: it checks the same ordinary Omega source through
//! the current product frontend and checked interpreter. Agreement is
//! differential evidence, not bootstrap authority.

use omega_compiler::compile_to_checked;
use psi_checked_interpreter::interpret_entry;
use std::path::Path;

#[test]
#[ignore = "run by the source-custody artifact gate with its exact fixture"]
fn product_semantics_observe_source_custody_fixture() {
    let source = std::env::var_os("OMEGA_SOURCE_CUSTODY_ARTIFACT")
        .expect("artifact gate must supply OMEGA_SOURCE_CUSTODY_ARTIFACT");
    let source = Path::new(&source);
    let checked = compile_to_checked(source, None).unwrap_or_else(|diagnostics| {
        panic!("product frontend rejected fixture: {diagnostics:#?}")
    });

    let outcome = interpret_entry(&checked, "Probe::run", &[]);
    assert_eq!(outcome.error, None, "product interpreter rejected fixture");
    assert_eq!(outcome.exit_code, 70, "fixture result changed");
    assert!(outcome.stdout.is_empty(), "fixture wrote unexpected stdout");
    assert!(outcome.stderr.is_empty(), "fixture wrote unexpected stderr");
}
