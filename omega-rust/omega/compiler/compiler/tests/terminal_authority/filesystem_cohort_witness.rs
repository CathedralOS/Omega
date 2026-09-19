//! The canonical `FilesystemHost` cohort table is toolchain-settled policy:
//! an ordinary program that demands a settled filesystem leaf emits its
//! native artifact with no receiving-policy input, the leaf lands in the
//! closure review with its exact exercised classes, and explicit receiver
//! admission replays admit only under a sufficient accepted policy.
//!
//! The emitters in
//! `native-realization/terminal_authority_policy/filesystem.rs` hold the
//! settled table; this target drives the same production join the operations
//! compile route reaches, so the board can see the leaf classifying without a
//! receiver-supplied row and the admitted artifact replaying the two-axis
//! claim exactly.

use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    RetainedNativeRealizationRequest, compile, compile_to_checked,
    realize_retained_native_artifact,
};
use diagnostics::Diagnostic;
use native_realization as native;
use package_compilation::{
    AcceptedSemanticBindingRole, BuildDeclarationKind, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

#[path = "../support/linux_entry_acceptance.rs"]
mod linux_entry_acceptance;

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate lives under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-filesystem-cohort-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).expect("create filesystem cohort fixture");
        let standard_library = repo_root()
            .join("source/library/std")
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(
            root.join("main.omg"),
            r#"use omega_language_std::filesystem_host;

data Main {
    files: FilesystemHost;
    fd: i32;
    rc: i32;
}

machine Main::main(&mut self)
reaches FilesystemHost
invokes FilesystemHost;
{
    self.fd = 3;
    let n: i32 = self.files.set_len(self.fd, 0);
    self.rc = n;
}
"#,
        )
        .expect("write filesystem cohort source");
        std::fs::write(
            root.join("build.omg"),
            format!(
                r#"machine build(builder: &mut Build) {{
    builder.application("filesystem-cohort-witness-{label}");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}}
"#
            ),
        )
        .expect("write filesystem cohort build declaration");
        Self { root }
    }

    fn main(&self) -> PathBuf {
        self.root.join("main.omg")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Package-level custody for the fixture: the application depends on the
/// exact repository standard library, accepts the checked
/// `LinuxX86_64Application` entry schema, and accepts the `FilesystemHost`
/// service binding carrying the settled cohort permission table the ordinary
/// review proposes for the root consumer.
fn filesystem_package_inputs(fixture: &Fixture) -> PackageCompilationInputs {
    let standard_library_root = repo_root().join("source/library/std");
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let inputs = PackageCompilationInputs::new(
        root_identity,
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                root_identity,
                "filesystem-cohort-witness",
                fixture.root.clone(),
            ),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega-language-std",
                standard_library_root.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .unwrap_or_else(|errors| panic!("fixture package inputs: {errors:#?}"));
    let mut bindings = vec![
        linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
            &standard_library_root,
            standard_library_identity,
        )
        .unwrap_or_else(|diagnostics| panic!("Linux entry fixture acceptance: {diagnostics:#?}")),
    ];
    let inputs = inputs
        .with_accepted_semantic_bindings(bindings.clone())
        .unwrap_or_else(|errors| panic!("fixture entry acceptance: {errors:#?}"));
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&fixture.main(), Some("linux_x86_64"))
    })
    .unwrap_or_else(|diagnostics| {
        panic!("preliminary checked fixture compilation: {diagnostics:#?}")
    });
    let definitions = preliminary
        .typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && preliminary
                    .typed
                    .symbols
                    .symbol_package_identity(definition.symbol)
                    == Some(standard_library_identity)
                && preliminary
                    .typed
                    .symbols
                    .display_path(definition.symbol, "::")
                    == "FilesystemHost"
        })
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!("the fixture resolves exactly one std FilesystemHost boundary");
    };
    let service_schema =
        provider_planning::service_schema::from_typed(&preliminary.typed, definition)
            .expect("the canonical FilesystemHost boundary yields a service schema");
    bindings.push(
        preliminary
            .candidate_service_binding(
                AcceptedSemanticBindingRole::FilesystemHostService,
                standard_library_identity,
                "FilesystemHost",
            )
            .expect("exact std FilesystemHost service candidate")
            .with_terminal_authority_permissions(
                native::filesystem_host_permission_rows(&service_schema)
                    .expect("the canonical schema resolves the settled cohort table"),
            )
            .expect("cohort permission rows attach to the accepted binding"),
    );
    inputs
        .with_accepted_semantic_bindings(bindings)
        .unwrap_or_else(|errors| panic!("fixture acceptance: {errors:#?}"))
}

fn compile_fixture(fixture: &Fixture) -> compilation_report::RetainedTerminalArtifact {
    compile(
        CompileRequest::new(CompileOptions {
            root_path: fixture.main(),
            build_dir: Some(fixture.root.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_package_inputs(filesystem_package_inputs(fixture)),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!(
            "filesystem cohort fixture should reach retained Terminal custody:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
    .into_retained_terminal_artifact()
    .expect("Terminal compilation retains its native proposal")
}

/// Realize the retained artifact under one supplied mechanism policy and one
/// optional receiving permission policy. The accepted-package rejoin keeps
/// the exact rows the compile retained.
fn realize(
    fixture: &Fixture,
    mechanism_policy: native::TerminalAuthorityPolicy,
    permission_policy: Option<native::TerminalAuthorityPermissionPolicy>,
) -> Result<native::RequestedNativeArtifact, Vec<Diagnostic>> {
    let retained = compile_fixture(fixture);
    let (subsystem, package_permissions) = {
        let proposal = retained
            .native_realization_proposal()
            .expect("native proposal");
        (
            proposal.subsystem(),
            proposal.package_terminal_authority_permissions().to_vec(),
        )
    };
    let accepted_package_policy =
        native::terminal_authority_permission_policy_with_rows(package_permissions)
            .expect("retained package permissions form the accepted policy");
    realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: mechanism_policy,
            accepted_package_terminal_authority_permission_policy: accepted_package_policy,
            terminal_authority_permission_policy: permission_policy,
            image_request: native::ExecutableImageEmissionRequest::direct(subsystem),
            imports: &[],
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)
}

/// Selected provider plans derive only from `satisfies` machine
/// conformances, and no `satisfies FilesystemHost` machine exists in the
/// standard library on this revision: the demanded `set_len` leaf reaches
/// `review_terminal_authority_closure` with zero selected provider rows, so
/// the closure review fails closed before any mechanism classification. The
/// named next blocker is selected-provider coverage for demanded canonical
/// `FilesystemHost` boundary requirements; the toolchain-settled cohort
/// minting this leg landed is witnessed one level down in
/// `native-realization`'s settlement tests.
#[test]
fn filesystem_cohort_emission_reports_the_selected_provider_blocker() {
    let fixture = Fixture::new("blocker");
    let diagnostics = realize(&fixture, native::current_terminal_authority_policy(), None)
        .expect_err("demanded FilesystemHost leaves have no selected provider coverage");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 0 selected provider rows")
                && diagnostic.message.contains("set_len")
        }),
        "the closure review names the demanded leaf and its missing coverage: {diagnostics:#?}"
    );
}
