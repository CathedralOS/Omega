//! Hosted console byte input publishes its honest envelope — `blocks;` plus an
//! unconditional `crashes Trap` route — on the requirement and on each target
//! realization, and the leaf keeps its selected compiler-intrinsic row under
//! that envelope. The bounded-line fixture also exercises the shared checked
//! `read_line` adapter, which inherits the same envelope transitively through
//! its `read_byte` calls.
//!
//! Every request below pins the product target as the build execution profile
//! explicitly, so this coverage runs on development hosts that have no native
//! target profile of their own.

use build_declarations::{BuildDeclaration, extract_build_declaration};
use checked_interpreter::{InterpretOptions, interpret_entry};
use compiler::{CheckedCompilation, CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use target::TargetProfile;

#[path = "support/console_acceptance.rs"]
mod console_acceptance;

const TARGET: &str = "linux_x86_64";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("fixture package identity is nonzero")
}

/// The product target doubles as the build execution profile so the check
/// never consults the compiler host's own profile.
fn checked_request(
    root_path: &Path,
    package_inputs: PackageCompilationInputs,
) -> CheckedCompileRequest<'static> {
    let mut request = CheckedCompileRequest::new(root_path, Some(TARGET));
    request.build_execution_profile = Some(TargetProfile::LinuxX64);
    request.package_inputs = Some(package_inputs);
    request
}

/// The hosted ProgramEntry contract is checked as dependency source before the
/// application's entry is selected; the candidate derives from a real checked
/// compile of the std package's entry source.
fn linux_x86_64_entry_binding(
    standard_library_root: &Path,
    package: PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "omega-language-std",
            standard_library_root.to_path_buf(),
        )],
        Vec::new(),
    )
    .map_err(|errors| {
        vec![Diagnostic::error(format!(
            "entry fixture inputs: {errors:?}"
        ))]
    })?;
    let checked = compile_to_checked(checked_request(
        &standard_library_root.join("targets/linux_x86_64/entry.omg"),
        inputs,
    ))?;
    checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
            package,
            "LinuxX86_64Application",
        )
        .map_err(|diagnostic| vec![diagnostic])
}

/// Root + bundled-std package routing for one fixture whose `build.omg`
/// declares the ordinary `Source::Path` std dependency. `standard_library_root`
/// is a parameter so negative fixtures can point at a mutated copy.
fn fixture_package_inputs(
    project_root: &Path,
    standard_library_root: &Path,
) -> PackageCompilationInputs {
    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let BuildDeclaration::Application(application) = declaration else {
        panic!(
            "fixture {} is not an application root",
            project_root.display()
        );
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let packages = vec![
        PackageSourceBinding::new(
            root_identity,
            application.name.into_string(),
            project_root.to_path_buf(),
        ),
        PackageSourceBinding::new(
            standard_library_identity,
            "omega-language-std",
            standard_library_root.to_path_buf(),
        ),
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        root_identity,
        "omega_language_std",
        standard_library_identity,
    )];
    PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
        .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", project_root.display()))
}

/// Compile the bounded-line fixture to checked semantics with the entry and
/// console service bindings the test policy accepts.
fn compile_bounded_line_fixture(
    project_root: &Path,
    standard_library_root: &Path,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let root_path = project_root.join("main.omg");
    let package_inputs = fixture_package_inputs(project_root, standard_library_root);
    let standard_library_identity = fixture_package_identity(2);
    let entry_binding =
        linux_x86_64_entry_binding(standard_library_root, standard_library_identity)?;
    // The package-owned entry contract must be accepted before the preliminary
    // compile: without it, entry selection rejects the package-owned source.
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![entry_binding.clone()])
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit fixture entry binding: {errors:?}"
            ))]
        })?;
    let preliminary = compile_to_checked(checked_request(&root_path, package_inputs.clone()))?;
    let console_binding = console_acceptance::candidate_console_exit_binding(
        &preliminary,
        standard_library_identity,
        true,
        true,
    )?;
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(vec![entry_binding, console_binding])
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit fixture semantic bindings: {errors:?}"
            ))]
        })?;
    compile_to_checked(checked_request(&root_path, package_inputs))
}

fn console_plan(checked: &CheckedCompilation) -> &ProviderPlan {
    checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Console")
        .expect("the bounded-line fixture selects the std Console provider")
}

#[test]
fn byte_input_keeps_its_compiler_intrinsic_row_under_the_honest_envelope() {
    let project_root = repo_root().join("tests/omega/pass/host/runtime_console_bounded_line_exit");
    let checked =
        compile_bounded_line_fixture(&project_root, &repo_root().join("source/library/std"))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "the bounded-line fixture must still reach checked semantics under the \
                 honest blocking/crash envelope: {diagnostics:#?}"
                )
            });
    let plan = console_plan(&checked);
    let read_byte = plan
        .rows
        .iter()
        .find(|row| row.method == "read_byte")
        .expect("the selected Console plan carries the read_byte row");
    assert!(
        matches!(read_byte.binding, ProviderBinding::CompilerIntrinsic { .. }),
        "the hosted byte-input leaf keeps its compiler-intrinsic row: {read_byte:?}"
    );
    let read_line = plan
        .rows
        .iter()
        .find(|row| row.method == "read_line")
        .expect("the selected Console plan carries the read_line row");
    assert!(
        !matches!(read_line.binding, ProviderBinding::CompilerIntrinsic { .. }),
        "the shared checked line adapter is never claimed as a native intrinsic: {read_line:?}"
    );

    // The honest envelope must not change interpreted behavior: this is the
    // same bounded-input matrix the canary suite drives on supported hosts.
    // Zero-capacity returns Full(0) without consuming input; LF in the last
    // destination slot still completes; EOF yields the consumed prefix; CR,
    // NUL, and multibyte bytes stay exact; untouched tails stay 0xa5.
    let cases: &[(&[u8], i32, &[u8])] = &[
        (b"", 20, b"\xa5\xa5\xa5\xa5"),
        (b"\nX", 11, b"\n\xa5\xa5\xa5X"),
        (b"ab", 22, b"ab\xa5\xa5"),
        (b"abc\nX", 14, b"abc\nX"),
        (b"abcd\n", 34, b"abcd\n"),
        (b"abcd", 34, b"abcd"),
        (b"\0\r\n\xff", 13, b"\0\r\n\xa5\xff"),
        (b"\xc3\xa9\xff\0X", 34, b"\xc3\xa9\xff\0X"),
    ];
    for &(input, expected_exit, expected_output) in cases {
        let outcome = interpret_entry(&checked, "Main::main", input, InterpretOptions::default());
        assert_eq!(outcome.error, None, "input {input:?}");
        assert_eq!(outcome.exit_code, expected_exit, "input {input:?}");
        assert_eq!(outcome.stdout, expected_output, "input {input:?}");
    }
    let repeated = interpret_entry(
        &checked,
        "Main::repeat",
        b"ab\nc",
        InterpretOptions::default(),
    );
    assert_eq!(repeated.error, None, "repeated middle window");
    assert_eq!(repeated.exit_code, 70);
    assert_eq!(repeated.stdout, b"\xa5ab\xa5\xa5\nb\xa5");
}

/// A `pub machine` that calls the blocking leaf must publish `blocks;` itself:
/// an authored `block` acknowledgement on the call is not a substitute for the
/// caller's own operational ceiling.
#[test]
fn a_published_caller_carries_the_envelope_it_invokes() {
    let scratch = ScratchTree::new();
    let project = scratch.0.join("project");
    fs::create_dir_all(&project).expect("create synthetic project");
    let standard_library = repo_root().join("source/library/std");
    let main_source = |signature_tail: &str| {
        format!(
            "use omega_language_std::console;\n\npub data Main {{\n    console: Console;\n}}\n\npub machine Main::main(&mut self)\nreaches Console\ninvokes Console;{signature_tail}\n{{\n    let observed: ByteRead = block self.console.read_byte();\n}}\n\nmachine build(builder: &mut Build) {{\n    builder.application(\"console-input-envelope-probe\");\n    builder.depend(Source::Path {{ location: \"{}\" }});\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n}}\n",
            standard_library.to_string_lossy().replace('\\', "/")
        )
    };
    let write_fixture = |signature_tail: &str| {
        let source = main_source(signature_tail);
        let (main_part, build_part) = source.split_once("machine build").expect("split");
        fs::write(project.join("main.omg"), main_part).expect("write caller");
        fs::write(
            project.join("build.omg"),
            format!("machine build{build_part}"),
        )
        .expect("write build");
    };

    // Without its own `blocks;`, the published caller must be rejected.
    write_fixture("");
    let bare = compile_bounded_line_fixture(&project, &standard_library);
    let bare_diagnostics = bare.expect_err(
        "a published machine calling a blocking leaf must not compile without `blocks;`",
    );
    assert!(
        bare_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("blocks")),
        "the rejection names the missing operational clause: {bare_diagnostics:#?}"
    );

    // Publishing `blocks;` and covering the leaf's trap route restores the
    // honest envelope and the intrinsic row. `crashes` contracts precede the
    // operational clause in signature order.
    write_fixture("\ncrashes Trap\nblocks;");
    let checked =
        compile_bounded_line_fixture(&project, &standard_library).unwrap_or_else(|diagnostics| {
            panic!("the honest published caller must reach checked semantics: {diagnostics:#?}")
        });
    let read_byte = console_plan(&checked)
        .rows
        .iter()
        .find(|row| row.method == "read_byte")
        .expect("the selected Console plan carries the read_byte row");
    assert!(
        matches!(read_byte.binding, ProviderBinding::CompilerIntrinsic { .. }),
        "the hosted byte-input leaf keeps its compiler-intrinsic row: {read_byte:?}"
    );
}

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct ScratchTree(PathBuf);

impl ScratchTree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-console-input-envelope-{}-{}",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed),
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create scratch tree");
        Self(root)
    }

    /// Copy one directory tree of `.omg` sources, preserving structure.
    fn copy_sources(&self, source: &Path, name: &str) -> PathBuf {
        let destination = self.0.join(name);
        let mut pending = vec![(source.to_path_buf(), destination.clone())];
        while let Some((from, to)) = pending.pop() {
            fs::create_dir_all(&to).expect("create copied source directory");
            for entry in fs::read_dir(&from).expect("list copied source directory") {
                let entry = entry.expect("read copied source entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push((path, to.join(entry.file_name())));
                } else if path.extension().is_some_and(|ext| ext == "omg") {
                    fs::copy(&path, to.join(entry.file_name())).expect("copy source file");
                }
            }
        }
        destination
    }
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A realization that drops the published `blocks;` clause no longer spells
/// the selected compiler-intrinsic identity: the row must stop binding
/// `Binding::CompilerIntrinsic`, or checked semantics must reject the
/// satisfies edge outright.
#[test]
fn a_realization_dropping_blocks_loses_its_intrinsic_row() {
    let scratch = ScratchTree::new();
    let standard_library = scratch.copy_sources(&repo_root().join("source/library/std"), "std");
    let project = scratch.copy_sources(
        &repo_root().join("tests/omega/pass/host/runtime_console_bounded_line_exit"),
        "project",
    );

    // Point the copied build at the copied std and strip the realization's
    // published blocking clause.
    let build_path = project.join("build.omg");
    let build = fs::read_to_string(&build_path).expect("read copied build");
    fs::write(
        &build_path,
        build.replace(
            "../../../../../source/library/std",
            &standard_library.to_string_lossy().replace('\\', "/"),
        ),
    )
    .expect("write copied build");
    let provider_path = standard_library.join("targets/linux_x86_64/console_impl.omg");
    let provider = fs::read_to_string(&provider_path).expect("read copied provider");
    let honest = "via Binding::CompilerIntrinsic\n    crashes Trap\n    blocks;";
    assert!(
        provider.contains(honest),
        "the linux x86-64 provider publishes the honest envelope"
    );
    fs::write(
        &provider_path,
        provider.replacen(
            honest,
            "via Binding::CompilerIntrinsic\n    crashes Trap;",
            1,
        ),
    )
    .expect("weaken copied provider");

    match compile_bounded_line_fixture(&project, &standard_library) {
        Err(_) => {
            // The satisfies edge or the plan admission rejected the doctored
            // provider; either is an honest refusal.
        }
        Ok(checked) => {
            let read_byte = console_plan(&checked)
                .rows
                .iter()
                .find(|row| row.method == "read_byte")
                .expect("the selected Console plan carries the read_byte row");
            assert!(
                !matches!(read_byte.binding, ProviderBinding::CompilerIntrinsic { .. }),
                "a nonblocking realization must not keep the hosted byte-input \
                 intrinsic row: {read_byte:?}"
            );
        }
    }
}
