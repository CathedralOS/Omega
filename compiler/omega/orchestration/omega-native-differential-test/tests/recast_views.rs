//! Focused interpreter parity for programmable-layout recast views.

use omega_compiler::{CheckedCompilation, compile_to_checked};
use psi_checked_interpreter::{InterpretOutcome, interpret_entry};
use std::path::{Path, PathBuf};

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(checked, "Main::main", stdin)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-native-differential-test lives under compiler/omega/orchestration")
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

#[test]
fn bool_representation_recasts_preserve_aliasing_and_facts() {
    let main =
        repo_root().join("canaries/pass/recast/runtime_bool_representation_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "bool representation recasts should compile:\n{}",
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
        "interpreter declined bool representation recasts: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn shared_domain_weakening_preserves_the_source_value() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_shared_domain_weakening_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "shared domain weakening should compile:\n{}",
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
        "interpreter declined shared domain weakening: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn float_range_recasts_preserve_aliasing_and_interval_facts() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_float_range_representation_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "same-carrier float range recasts should compile:\n{}",
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
        "interpreter declined float-range recasts: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn shared_record_float_range_weakening_preserves_the_leaf_value() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_shared_record_float_range_weakening_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "shared record float-range weakening should compile:\n{}",
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
        "interpreter declined shared record float-range weakening: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn mutable_equivalent_record_recast_preserves_aliasing_and_facts() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_mutable_equivalent_record_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "equivalent-record recast should compile:\n{}",
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
        "interpreter declined equivalent-record recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn aggregate_slice_recasts_preserve_repeated_leaf_facts_and_aliasing() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_aggregate_slice_representation_recast_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "aggregate slice representation recast should compile:\n{}",
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
        "interpreter declined aggregate slice representation recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interior_slice_recasts_preserve_dynamic_tail_length_and_aliasing() {
    let main = repo_root()
        .join("canaries/pass/recast/runtime_interior_slice_view_mutable_write_exit/main.omg");
    let checked = compile_to_checked(&main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "interior slice recast should compile:\n{}",
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
        "interpreter declined interior slice recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}
