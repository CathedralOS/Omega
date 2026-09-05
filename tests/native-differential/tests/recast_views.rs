//! Focused interpreter parity for programmable-layout recast views.

#[path = "../fixture_rosters/recast_views.rs"]
mod fixture_roster;

use checked_interpreter::{InterpretOutcome, interpret_entry};
use compiler::{CheckedCompilation, compile_to_checked};
use std::path::{Path, PathBuf};

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(checked, "Main::main", stdin)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .to_path_buf()
}

#[test]
fn mutable_equivalent_domain_recast_preserves_the_established_fact() {
    let main = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_RANGE_RECAST_EXIT)
        .join("main.omg");
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
    let main = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_BOOL_REPRESENTATION_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_SHARED_DOMAIN_WEAKENING_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_FLOAT_RANGE_REPRESENTATION_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_SHARED_RECORD_FLOAT_RANGE_WEAKENING_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_RECORD_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_AGGREGATE_SLICE_REPRESENTATION_RECAST_EXIT)
        .join("main.omg");
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
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_INTERIOR_SLICE_VIEW_MUTABLE_WRITE_EXIT)
        .join("main.omg");
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
