//! Focused interpreter parity for programmable-layout recast views.

use build_declarations::{BuildDeclaration, extract_build_declaration};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
#[path = "../fixture_rosters/recast_views.rs"]
mod fixture_roster;
// The compiler test target owns the exact Console provider acceptance this
// harness must replay; including it keeps both targets on one derivation.
#[path = "../../../omega-rust/omega/compiler/compiler/tests/support/console_acceptance.rs"]
mod console_acceptance;

use checked_interpreter::{InterpretOutcome, interpret_entry};
use compiler::CheckedCompileRequest;
use compiler::{CheckedCompilation, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(
        checked,
        BuildMachineEntry::Name("Main::main"),
        stdin,
        InterpretOptions::default(),
    )
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("recast fixture package identity is nonzero")
}

/// Package inputs for a recast fixture that declares the ordinary std
/// dependency in its `build.omg`; `None` for self-contained fixtures. The
/// reconciled graph binds the root package to the fixture directory and std
/// to the repository path, which is what `use omega_language_std::console`
/// resolves against — the unmanaged route would instead look for a
/// `omega_language_std/` module below the fixture root.
fn fixture_package_inputs(root_path: &Path) -> Option<PackageCompilationInputs> {
    let project_root = root_path
        .parent()
        .expect("recast fixture source has a project root");
    let declares_std = fs::read_to_string(project_root.join("build.omg")).is_ok_and(|build| {
        build.contains("builder.depend(Source::Path") && build.contains("source/library/std")
    });
    if !declares_std {
        return None;
    }

    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("recast fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "recast fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let packages = vec![
        PackageSourceBinding::new(
            root_identity,
            root_name.into_string(),
            project_root.to_path_buf(),
        ),
        PackageSourceBinding::new(
            standard_library_identity,
            "omega-language-std",
            repo_root().join("source/library/std"),
        ),
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        root_identity,
        "omega_language_std",
        standard_library_identity,
    )];

    Some(
        PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
            .unwrap_or_else(|errors| {
                panic!("recast fixture {}: {errors:#?}", project_root.display())
            }),
    )
}

/// The package graph plus this harness's test acceptance of the exact std
/// Console provider the fixture selects. The admitted row is derived from and
/// replayed against the preliminary checked graph; this is test-owned
/// acceptance, not evidence that an audit occurred.
fn reviewed_fixture_package_inputs(
    root_path: &Path,
) -> Result<Option<PackageCompilationInputs>, Vec<Diagnostic>> {
    let Some(package_inputs) = fixture_package_inputs(root_path) else {
        return Ok(None);
    };
    let accepts_console_exit = fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::console") && source.contains(".exit_process(")
    });
    if !accepts_console_exit {
        return Ok(Some(package_inputs));
    }
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone()),
        ..CheckedCompileRequest::new(root_path, None)
    })?;
    let binding = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        fixture_package_identity(2),
        false,
        false,
    )?;
    package_inputs
        .with_accepted_semantic_bindings(vec![binding])
        .map(Some)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit recast fixture semantic binding: {errors:?}"
            ))]
        })
}

fn compile_fixture(main: &Path) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let mut request = CheckedCompileRequest::new(main, None);
    request.package_inputs = reviewed_fixture_package_inputs(main)?;
    compile_to_checked(request)
}

#[test]
fn mutable_equivalent_domain_recast_preserves_the_established_fact() {
    let main = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_MUTABLE_EQUIVALENT_DOMAIN_RECAST_EXIT)
        .join("main.omg");
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
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

#[test]
fn interior_slice_congruent_runtime_offset_preserves_length_and_writes() {
    let main = repo_root()
        .join("tests/omega/pass")
        .join(fixture_roster::RUNTIME_INTERIOR_SLICE_CONGRUENT_OFFSET_EXIT)
        .join("main.omg");
    let checked = compile_fixture(&main).unwrap_or_else(|diagnostics| {
        panic!(
            "congruent-offset interior slice recast should compile:\n{}",
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
        "interpreter declined congruent-offset interior slice recast: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}
