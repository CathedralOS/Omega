//! Build-owned ELF loader selection through accepted package production
//! (wiki/spec/build/configuration.md#elf-executable-loader-selection).
//!
//! One accepted Linux customer calls a glibc export, so its object retains a
//! normalized ELF import and links dynamically. The retained proposal carries
//! the effective loader into emission: the Linux target environment's
//! declared default when the Build selects none, the authored row otherwise.
//! Publication writes a flat file whose `PT_INTERP` the test parses from the
//! installed bytes. Cross-target emission never consults the build host's
//! loader, so this runs on every development host.

use super::super::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use super::{TemporaryProject, accepted_lock::accept_project};
use crate::operations::{LocalProjectPreparationOptions, prepare_local_project};

const DYNAMIC_EXIT_CUSTOMER: &str = r#"use omega::language::core::external_binding;

pub data ForeignProcess {}

pub boundary requirement ForeignProcess::exit_with(code: i32);

data ForeignProcessProvider {}

linux_x86_64 machine foreign_exit_binding() -> ForeignBinding<9, 4, 11> {
    ForeignBinding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libc.so.6",
            symbol: "exit",
            version: "GLIBC_2.2.5",
        },
    }
}

linux_x86_64 machine ForeignProcessProvider::exit_leaf(code: i32)
satisfies ForeignProcess::exit_with via foreign_exit_binding();

data Main {}

machine Main::main(&mut self)
{
    ForeignProcess::exit_with(21);
}
"#;

/// Accept and realize the dynamic customer under `build_statements` (extra
/// authored Build statements).
fn compile_dynamic_customer(
    project: &TemporaryProject,
    build_statements: &str,
) -> Result<compiler::CompileReport, super::super::CompilePreparedLocalProjectNativeError> {
    std::fs::write(
        project.source.join("build.omg"),
        format!(
            r#"
machine build(builder: &mut Build) {{
    Build::application(builder, "elf_loader_customer");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
{build_statements}
}}
"#
        ),
    )
    .expect("write the loader customer's build declaration");
    std::fs::write(project.entry(), DYNAMIC_EXIT_CUSTOMER).expect("write the loader customer");
    let target = target::TargetProfile::LinuxX64;
    accept_project(project, target);
    let prepared = prepare_local_project(
        &project.entry(),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .expect("prepare the accepted loader customer")
    .expect("application project");
    compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, project.workspace.join("native"), target),
        |_| (),
    )
    .map(|(report, ())| report)
}

/// Compile, accept and publish the dynamic customer under `loader_row`,
/// returning the installed file's `PT_INTERP`.
fn published_interpreter(loader_row: &str) -> Vec<u8> {
    let project = TemporaryProject::new();
    let report = compile_dynamic_customer(&project, loader_row)
        .expect("an accepted import-bearing Linux customer realizes");
    assert_eq!(
        report.output_kind(),
        compiler::CompileOutputKind::RetainedNativeArtifact
    );
    assert!(
        report.retained_native_artifact().is_none(),
        "dynamic output never enters direct-image custody"
    );
    report
        .retained_dynamic_elf_artifact()
        .expect("the import-bearing object selected the dynamic ELF writer")
        .validate()
        .expect("dynamic ELF custody replays");
    let published = compilation_products::publication::publish_compilation(
        report,
        &project.workspace.join("published"),
    )
    .expect("publish the dynamic ELF output");
    let receipt = published
        .dynamic_elf_publication()
        .expect("publication recorded dynamic ELF custody");
    receipt
        .validate_published_file()
        .expect("the installed file matches its receipt");
    let executable = published
        .checked_native_executable_path()
        .expect("the published report names its executable");
    pt_interp(&std::fs::read(executable).expect("read the published executable"))
        .expect("dynamic output carries PT_INTERP")
}

/// `PT_INTERP` contents of one little-endian ELF64 image.
fn pt_interp(bytes: &[u8]) -> Option<Vec<u8>> {
    const PT_INTERP: u32 = 3;
    let u16_at = |offset: usize| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    let u32_at = |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    assert!(bytes.starts_with(b"\x7fELF\x02\x01"));
    let program_headers = u64_at(0x20) as usize;
    let entry_size = usize::from(u16_at(0x36));
    (0..usize::from(u16_at(0x38))).find_map(|index| {
        let header = program_headers + index * entry_size;
        (u32_at(header) == PT_INTERP).then(|| {
            let offset = u64_at(header + 8) as usize;
            bytes[offset..offset + u64_at(header + 32) as usize].to_vec()
        })
    })
}

#[test]
fn accepted_dynamic_linux_output_uses_the_target_environment_default_loader() {
    assert_eq!(published_interpreter(""), b"/lib64/ld-linux-x86-64.so.2\0");
}

#[test]
fn accepted_dynamic_linux_output_uses_the_authored_loader_row() {
    assert_eq!(
        published_interpreter(
            r#"    builder.elf_interpreter.linux_x86_64 = "/lib/ld-musl-x86_64.so.1";"#
        ),
        b"/lib/ld-musl-x86_64.so.1\0",
    );
}

#[test]
fn accepted_dynamic_linux_output_refuses_a_native_proof_product() {
    // The native `.proof` sidecar certifies direct-image evidence only; a
    // dynamically linked output must refuse the request rather than publish
    // an uncertified pair.
    let project = TemporaryProject::new();
    let error = compile_dynamic_customer(&project, "    builder.pcc.native = true;")
        .expect_err("dynamic ELF output cannot satisfy a native proof request");
    assert!(
        format!("{error:?}").contains("cannot carry a native proof product"),
        "{error:?}"
    );
}
