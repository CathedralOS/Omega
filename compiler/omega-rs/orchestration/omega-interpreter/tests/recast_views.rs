//! Focused interpreter parity for programmable-layout recast views.

use omega_compiler::compile_to_checked;
use omega_interpreter::interpret;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-interpreter lives under compiler/orchestration/omega-interpreter")
        .to_path_buf()
}

#[test]
fn mutable_equivalent_domain_recast_preserves_the_established_fact() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_mutable_equivalent_domain_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "equivalent-domain recast should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter declined equivalent-domain recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn mutable_equivalent_range_recast_preserves_the_established_fact() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_mutable_equivalent_range_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "equivalent-range recast should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter declined equivalent-range recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}
