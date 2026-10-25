//! Default-build provenance through publication, relocation and a real consumer.
//! Fixture harness acceptance is receiving test policy; it grants no authority
//! merely because an anonymous identity or a matching display name exists.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CheckedCompileRequest, CompilationOutput, CompilationProduct, CompilationSession};
use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
};
use terminal_codec::CanonicalTerminalArtifact;

fn checked_owner(root: &Path) -> PackageKeyIdentity {
    checked_owner_for(root, "library.omg")
}

fn checked_owner_for(root: &Path, file: &str) -> PackageKeyIdentity {
    let checked = CompilationSession::default()
        .check(CheckedCompileRequest::new(&root.join(file), None))
        .result
        .unwrap_or_else(|failure| panic!("anonymous check: {:?}", failure.into_diagnostics()));
    let owners = checked
        .execution()
        .program
        .symbols
        .source_files()
        .filter(|file| file.origin == source::SourceOrigin::User)
        .map(|file| {
            file.package_identity
                .expect("default root must carry its final owner")
        })
        .collect::<Vec<_>>();
    assert!(!owners.is_empty());
    assert!(owners.iter().all(|owner| *owner == owners[0]));
    owners[0]
}

fn artifact(root: &Path) -> CanonicalTerminalArtifact {
    CompilationSession::default()
        .produce(
            CheckedCompileRequest::new(&root.join("library.omg"), None),
            CompilationProduct::Inspection("library::read".to_owned()),
        )
        .and_then(CompilationOutput::into_inspection)
        .unwrap_or_else(|error| {
            panic!(
                "anonymous Terminal publication: {:?}",
                error.into_diagnostics()
            )
        })
}

fn declaration(artifact: &CanonicalTerminalArtifact) -> &terminal_psi::CheckedMachineDeclaration {
    let module = artifact.semantic_module();
    module
        .machines
        .iter()
        .find(|machine| terminal_psi::TargetMachineCallable::Machine(machine.id) == module.entry)
        .and_then(|machine| machine.declaration.as_ref())
        .expect("published source declaration")
}

fn inputs(
    consumer: &Path,
    library: &Path,
    owner: PackageKeyIdentity,
    copied: Option<(&Path, PackageKeyIdentity)>,
) -> PackageCompilationInputs {
    let root_owner = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let standard = PackageKeyIdentity::from_digest([2; 32]).unwrap();
    let standard_root = fixture_package_inputs::bundled_standard_library_root();
    let mut packages = vec![
        PackageSourceBinding::new(root_owner, "consumer", consumer.to_path_buf())
            .with_canonical_source_metadata()
            .expect("sealed consumer Source metadata"),
        PackageSourceBinding::new(standard, "omega_language_std", standard_root.clone()),
        PackageSourceBinding::new(owner, "owner_identity_library", library.to_path_buf()),
    ];
    let mut edges = vec![
        PackageDependencyBinding::new(root_owner, "omega_language_std", standard),
        PackageDependencyBinding::new(root_owner, "shared", owner),
    ];
    if let Some((path, identity)) = copied {
        packages.push(PackageSourceBinding::new(
            identity,
            "copied_library",
            path.to_path_buf(),
        ));
        edges.push(PackageDependencyBinding::new(
            root_owner,
            "foreign_copy",
            identity,
        ));
    }
    let binding = fixture_package_inputs::candidate_program_entry_binding(
        "linux_x86_64",
        &standard_root,
        standard,
    )
    .unwrap_or_else(|errors| panic!("entry candidate: {errors:?}"))
    .unwrap();
    PackageCompilationInputs::new(
        root_owner,
        BuildDeclarationKind::Application,
        packages,
        edges,
    )
    .expect("exact supplied source graph")
    .with_accepted_semantic_bindings(vec![binding])
    .expect("accepted entry binding")
}

fn scratch_library(name: &str, extra: bool) -> PathBuf {
    let root = fixture_package_inputs::sealed_fixture_staging::temporary_session_root().join(name);
    fs::create_dir(&root).unwrap();
    let source = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/nominal_declaration_owner/library");
    for file in ["library.omg", "copied.omg"] {
        fs::copy(source.join(file), root.join(file)).unwrap();
    }
    if extra {
        fs::write(root.join("semantic-input.omg"), "const CHANGED: u8 = 1;\n").unwrap();
    }
    fixture_package_inputs::sealed_fixture_staging::stage_sealed_dependency_snapshot(
        fixture_package_inputs::sealed_fixture_staging::temporary_session_root(),
        &root,
    )
}

#[test]
fn anonymous_publication_reloads_without_rebasing_and_copied_owner_differs() {
    let first = scratch_library("anonymous-original", false);
    let relocated = scratch_library("anonymous-relocated", false);
    let copied = scratch_library("anonymous-changed", true);
    let owner = checked_owner(&first);
    assert_eq!(
        owner,
        checked_owner_for(&first, "copied.omg"),
        "source entry selection is separate from declaration owner"
    );
    assert_eq!(
        owner,
        checked_owner(&relocated),
        "host paths are not owners"
    );
    assert_ne!(
        owner,
        checked_owner(&copied),
        "the complete captured source is an input"
    );
    let first = artifact(&first);
    let relocated = artifact(&relocated);
    let copied = artifact(&copied);
    let reloaded = CanonicalTerminalArtifact::from_bytes(&first.to_bytes())
        .expect("canonical artifact reload");
    for same in [&relocated, &reloaded] {
        assert!(declaration(&first).same_identity_as(
            &first.semantic_module().identities,
            declaration(same),
            &same.semantic_module().identities
        ));
    }
    assert!(!declaration(&first).same_identity_as(
        &first.semantic_module().identities,
        declaration(&copied),
        &copied.semantic_module().identities
    ));
    let absent =
        terminal_psi::NominalDeclarationIdentity::new(terminal_psi::TerminalIdentityIndex::ABSENT);
    assert!(!absent.same_identity_as(
        &first.semantic_module().identities,
        &declaration(&first).machine,
        &first.semantic_module().identities
    ));
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn anonymous_owner_consumer_runs_nine_places_and_rejects_namespace_substitution() {
    let library = scratch_library("anonymous-consumer-library", false);
    let owner = checked_owner(&library);
    let repository = fixture_package_inputs::repo_root();
    for place in [
        "local",
        "field",
        "parameter",
        "call_result",
        "match_join",
        "const_name",
        "other_state",
        "other_machine",
        "target_body",
    ] {
        let authored = repository
            .join("samples/cli/facts/nominal_declaration_owner")
            .join(place);
        let staged = fixture_package_inputs::sealed_fixture_staging::stage_sealed_snapshot(
            &repository,
            &authored,
        );
        let supplied = inputs(&staged, &library, owner, None);
        let checked = CompilationSession::default()
            .check(CheckedCompileRequest {
                package_inputs: Some(supplied.clone()),
                ..CheckedCompileRequest::new(&staged.join("main.omg"), Some("linux_x86_64"))
            })
            .result
            .unwrap_or_else(|failure| panic!("{place} source: {:?}", failure.into_diagnostics()));
        assert!(
            checked
                .execution()
                .program
                .symbols
                .source_files()
                .filter(|file| file.path.starts_with(&library))
                .all(|file| file.package_identity == Some(owner)),
            "consumer must preserve its supplier owner"
        );
        let output = fixture_package_inputs::sealed_fixture_staging::temporary_session_root()
            .join(format!("native-{place}"));
        let report = compiler::compile(
            compiler::CompileRequest::new(compiler::CompileOptions {
                root_path: staged.join("main.omg"),
                build_dir: Some(output.clone()),
                target_name: Some("linux_x86_64".to_owned()),
            })
            .with_package_inputs(supplied)
            .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|errors| panic!("{place} native production: {errors:?}"));
        let artifact = report
            .retained_native_artifact()
            .expect("direct native product")
            .psi_artifact();
        let reloaded = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
            .expect("consumer artifact reload");
        let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
            x86_flags: None,
            provider_proposal: None,
            artifact: &reloaded,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 100_000,
            domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: None,
            clock: None,
        })
        .unwrap_or_else(|error| panic!("{place} Terminal execution: {error:?}"));
        assert!(
            matches!(
                observation.status,
                terminal_interpreter::TerminalExecutionStatus::Complete(
                    terminal_interpreter::TerminalExecutionResult::Unit
                )
            ),
            "{place}: {:?}",
            observation.status
        );
        let published = compilation_products::publication::publish_compilation(report, &output)
            .expect("native publication");
        let executable = published
            .checked_native_executable_path()
            .expect("native executable");
        let result = Command::new(executable)
            .output()
            .expect("host native execution");
        assert_eq!(
            result.status.code(),
            Some(0),
            "{place} native oracle: {result:?}"
        );
        let invalid = fixture_package_inputs::sealed_fixture_staging::stage_sealed_snapshot(
            &repository,
            &authored.join("invalid"),
        );
        let failure = CompilationSession::default()
            .check(CheckedCompileRequest {
                package_inputs: Some(inputs(&invalid, &library, owner, None)),
                ..CheckedCompileRequest::new(&invalid.join("main.omg"), Some("linux_x86_64"))
            })
            .result
            .err()
            .unwrap_or_else(|| panic!("{place}: copied namespace must reject"));
        let diagnostic = format!("{:?}", failure.into_diagnostics());
        assert!(
            diagnostic.contains("expects the `library::Token` data type but got `copied::Token`"),
            "{place}: {diagnostic}"
        );
    }
}

#[test]
fn equal_shaped_types_from_a_changed_anonymous_owner_cannot_substitute() {
    let library = scratch_library("anonymous-owner-control-original", false);
    let copied = scratch_library("anonymous-owner-control-copy", true);
    let owner = checked_owner(&library);
    let copied_owner = checked_owner(&copied);
    assert_ne!(owner, copied_owner);
    let temporary = fixture_package_inputs::sealed_fixture_staging::temporary_session_root();
    let source = "use shared::library;\nuse foreign_copy::library;\nmachine main() { let result: u8 = shared::library::read(foreign_copy::library::Token { value: 7 }); }\n";
    let build = "machine build(builder: &mut Build) { Build::application(builder, \"copied_owner_consumer\"); Build::bind(builder, linux_x86_64::ProgramEntry, main); }\n";
    for (name, source, rejects) in [
        (
            "owner-positive",
            source.replace("foreign_copy::library::Token", "shared::library::Token"),
            false,
        ),
        ("owner-rejected", source.to_owned(), true),
    ] {
        let authored = temporary.join(name);
        fs::create_dir(&authored).unwrap();
        fs::write(authored.join("main.omg"), source).unwrap();
        fs::write(authored.join("build.omg"), build).unwrap();
        let staged = fixture_package_inputs::sealed_fixture_staging::stage_sealed_snapshot(
            temporary, &authored,
        );
        let result = CompilationSession::default()
            .check(CheckedCompileRequest {
                package_inputs: Some(inputs(
                    &staged,
                    &library,
                    owner,
                    Some((&copied, copied_owner)),
                )),
                ..CheckedCompileRequest::new(&staged.join("main.omg"), Some("linux_x86_64"))
            })
            .result;
        if rejects {
            let failure = result
                .err()
                .expect("a different anonymous owner cannot supply the original nominal type");
            let diagnostic = format!("{:?}", failure.into_diagnostics());
            eprintln!("copied-owner rejection: {diagnostic}");
            assert!(
                diagnostic
                    .contains("expects the `library::Token` data type but got `library::Token`")
                    && diagnostic.contains("these are incompatible data types"),
                "{diagnostic}"
            );
        } else {
            result.unwrap_or_else(|failure| {
                panic!(
                    "matching anonymous owner must remain valid: {:?}",
                    failure.into_diagnostics()
                )
            });
        }
    }
}
