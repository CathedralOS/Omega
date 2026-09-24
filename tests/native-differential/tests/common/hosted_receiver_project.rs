//! The authored hosted-receiver project both differential entry targets
//! compile.
//!
//! `hosted_receiver` and `hosted_receiver_checked_entry` each witness the same
//! contract — an authored `Binding<Console>`-carrying receiver reaches a
//! published process image under exact custody, and a bare interface spelling
//! of the same field refuses — so the project they compile, the package graph
//! it is bound into, and the two acceptance rows it needs are written once
//! here. Only the `#[test]` functions that witness the contract stay in each
//! target.
//!
//! The project is authored, not a repository fixture: its `build.omg` is
//! written here, so its standard-library dependency row comes from
//! [`crate::fixture_package_inputs::bundled_standard_library_dependency_declaration`]
//! rather than from a path typed into the string.

// Included by two test targets through `#[path]`.
#![allow(dead_code)]

use crate::fixture_package_inputs::{
    bundled_standard_library_dependency_declaration, fixture_package_identity,
    linux_x86_64_entry_binding, standard_library_package_inputs,
};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct,
    compile_to_checked,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The authored application's fixture marker, and the bundled standard
/// library's: the standard library's marker must be nameable by the entry and
/// Console acceptance rows below.
const ROOT_MARKER: u8 = 101;
const STANDARD_LIBRARY_MARKER: u8 = 102;

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The bundled standard library's fixture identity, which the Console
/// acceptance row below must name.
fn standard_library_identity() -> PackageKeyIdentity {
    fixture_package_identity(STANDARD_LIBRARY_MARKER)
}

/// The package graph the authored project compiles in: its own root bound to
/// the written directory, and the bundled standard library it declared.
pub fn hosted_receiver_package_inputs(project: &Path) -> PackageCompilationInputs {
    standard_library_package_inputs(project, ROOT_MARKER, STANDARD_LIBRARY_MARKER)
}

pub fn project_directory(name: &str) -> PathBuf {
    let sequence = PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-native-diff-hosted-receiver-{name}-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create exclusively owned hosted-entry project");
    directory
}

/// Author the hosted receiver program and its build declaration into a
/// fresh project directory. `bound_service` selects the intrinsic
/// `Binding<Console>` carrier or the bare `Console` interface that must
/// refuse; `explicit_exit` completes through `exit_process(37)` so the
/// provider's own status survives the hosted bridge.
pub fn author_hosted_receiver_project(
    directory: &Path,
    explicit_exit: bool,
    bound_service: bool,
) -> PathBuf {
    std::fs::write(
        directory.join("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("native_diff_hosted_receiver");
{}    builder.select_provider<omega_language_std::Console, omega_language_std::ConsoleNativeProvider>();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}}
"#,
            bundled_standard_library_dependency_declaration()
        ),
    )
    .expect("write authored target, entry, and provider selection");
    let completion = if explicit_exit {
        "self.console.exit_process(37);"
    } else {
        ""
    };
    let console_type = if bound_service {
        "Binding<Console>"
    } else {
        "Console"
    };
    let root = directory.join("main.omg");
    std::fs::write(
        &root,
        format!(
            r#"use omega_language_std::console;
use omega::language::core::binding;

data Main {{
    value: i32;
    bytes: [u8; 256];
    console: {console_type};
}}

machine Main::main(&mut self) reaches Console {{
    transition self.value == 0 {{
        true -> initialized()
        false -> failed()
    }}
    state initialized(&mut self) {{
        self.value = 65;
        self.bytes[255] = 64;
        transition self.value == 65 && self.bytes[255] == 64 && self.bytes[254] == 0 {{
            true -> observed()
            false -> failed()
        }}
    }}
    state observed(&mut self) {{
        self.console.write_byte(self.value);
        {completion}
    }}
    state failed(&mut self) {{
        self.console.write_byte(70);
    }}
}}
"#
        ),
    )
    .expect("write receiver storage and fused Console customer");
    root
}

/// Derive the Console service acceptance for this exact application from
/// its own preliminary checked compile: the bound provider plan must be
/// std's `Console` schema realized by `ConsoleNativeProvider`, and only the
/// output and termination authorities the fixture calls may be admitted.
pub fn console_binding(preliminary: &compiler::CheckedCompilation) -> AcceptedSemanticBinding {
    let candidates = preliminary
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(preliminary.selected_provider_provenance())
        .filter(|(plan, provenance)| {
            plan.schema.trait_name == "Console"
                && plan.rows.iter().any(|row| row.method == "write_byte")
                && preliminary
                    .typed
                    .symbols
                    .symbol_package_identity(provenance.provider.schema.symbol())
                    == Some(standard_library_identity())
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        panic!(
            "hosted receiver resolved {} exact std Console provider plans instead of one",
            candidates.len()
        )
    };
    let declaration_path = preliminary
        .typed
        .symbols
        .display_path(provenance.provider.schema.symbol(), "::");
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        standard_library_identity(),
        declaration_path,
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .expect("construct Console service binding");
    let mut permissions = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            matches!(
                method.name.as_str(),
                "exit_process" | "write" | "write_byte" | "write_line"
            )
        })
        .map(|method| {
            effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                method.requirement_identity.clone(),
                effects::TerminalAuthorityDisposition::from_classes(match method.name.as_str() {
                    "exit_process" => {
                        vec![effects::TerminalAuthorityClass::ProcessTermination]
                    }
                    _ => vec![effects::TerminalAuthorityClass::ProcessOutput],
                }),
            )
        })
        .collect::<Vec<_>>();
    permissions.sort_by(|left, right| {
        left.requirement_identity()
            .cmp(right.requirement_identity())
    });
    binding
        .with_terminal_authority_permissions(permissions)
        .expect("attach Console terminal authority permissions")
}

pub fn compile_hosted_receiver(root: &Path, project: &Path) -> compiler::CompileReport {
    let inputs = hosted_receiver_package_inputs(project);
    // The program-entry root must be accepted before the build machine's
    // `roots.bind` evaluates during checked compilation.
    let entry = linux_x86_64_entry_binding(STANDARD_LIBRARY_MARKER);
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry.clone()])
        .expect("hosted receiver entry binding admits");
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(root, Some("linux_x86_64"))
    })
    .expect("hosted receiver preliminary checked compilation");
    let inputs = inputs
        .with_accepted_semantic_bindings(vec![entry, console_binding(&preliminary)])
        .expect("hosted receiver semantic bindings admit");
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root.to_path_buf(),
            build_dir: Some(project.join("build")),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("authored hosted receiver must produce its executable")
}
