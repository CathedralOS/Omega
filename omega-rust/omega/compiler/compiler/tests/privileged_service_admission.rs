//! `Build.privileged_services` grants end to end: an authored
//! `b.privileged_services.port_io = true` on an otherwise hosted build widens
//! `AsmAuthorityAdmission` so port-I/O asm instructions admit, the grant stays
//! exact (it does not infer machine-owner or interrupt-table authority), and
//! an ungranted hosted build still rejects port-I/O asm while naming the
//! grant spelling.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};

fn write_project(label: &str, build: &str, main: &str) -> std::path::PathBuf {
    let project = std::env::temp_dir().join(format!(
        "omega-privileged-service-admission-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project");
    std::fs::write(project.join("build.omg"), build).expect("write build declaration");
    std::fs::write(project.join("main.omg"), main).expect("write source");
    project
}

fn check(project: &std::path::Path) -> Result<(), String> {
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: project.join("main.omg"),
            build_dir: Some(project.join("build")),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::Check),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .map(|_| ())
    .map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

const PORT_IO_PROGRAM: &str = r#"use omega::language::core::assembly;

data Main {
    port: u16;
    value: u8;
}

machine Main::main(&mut self) reaches PortIo {
    asm { out self.port, self.value }
    asm { in self.value, self.port }
}
"#;

const MACHINE_OWNER_PROGRAM: &str = r#"use omega::language::core::assembly;

data Main {}

machine Main::main(&mut self) reaches MachineControl {
    asm { hlt }
}
"#;

const GRANTED_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("privileged-service-admission");
    builder.privileged_services.port_io = true;
    builder.privileged_services.interrupt_table = true;
}
"#;

const PLAIN_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("privileged-service-admission");
}
"#;

#[test]
fn privileged_services_grant_admits_port_io_asm_on_hosted_build() {
    let project = write_project("granted", GRANTED_BUILD, PORT_IO_PROGRAM);

    check(&project).unwrap_or_else(|diagnostics| {
        panic!("a hosted build granting privileged_services.port_io must admit port-I/O asm: {diagnostics}")
    });
}

#[test]
fn ungranted_hosted_build_rejects_port_io_asm_and_names_the_grant() {
    let project = write_project("ungranted", PLAIN_BUILD, PORT_IO_PROGRAM);

    let diagnostics = check(&project).expect_err("hosted builds supply no port-I/O admission");
    assert!(
        diagnostics.contains("port-I/O authority"),
        "expected the port-I/O authority rejection, got: {diagnostics}"
    );
    assert!(
        diagnostics.contains("b.privileged_services.port_io = true"),
        "expected the diagnostic to name the granular grant spelling, got: {diagnostics}"
    );
}

#[test]
fn port_io_grant_does_not_admit_machine_owner_asm() {
    let project = write_project("grant-exact", GRANTED_BUILD, MACHINE_OWNER_PROGRAM);

    let diagnostics = check(&project)
        .expect_err("port-I/O and interrupt-table grants must not infer machine-owner authority");
    assert!(
        diagnostics.contains("machine-owner authority"),
        "expected the machine-owner authority rejection, got: {diagnostics}"
    );
}
