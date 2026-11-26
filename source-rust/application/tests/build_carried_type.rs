//! Product publication distinguishes a carried result from selecting its field.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn passing_a_transitive_result_keeps_its_type_without_granting_member_access() {
    let project = TemporaryDirectory::create("carried-result", false).unwrap();
    let root = project.path().join("consumer");
    let supplier = project.path().join("supplier");
    let api = project.path().join("api");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&supplier).unwrap();
    fs::create_dir(&api).unwrap();
    fs::write(
        supplier.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "packet_library"); }"#,
    )
    .unwrap();
    fs::write(
        supplier.join("main.omg"),
        "public data Packet [copy] { count: u32; }
         public machine packet() -> Packet reaches none crashes none { Packet { count: 41u32 } }",
    )
    .unwrap();
    fs::write(
        api.join("build.omg"),
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
            Build::package(builder, "packet_api");
            let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../supplier" }));
        }"#,
    )
    .unwrap();
    fs::write(
        api.join("main.omg"),
        r#"use {packet_library}/main.omg as packet_library;
           public machine exported_packet() -> |packet_library|::Packet reaches none crashes none { |packet_library|::packet() }
           public machine read_packet(value: |packet_library|::Packet) -> u32 reaches none crashes none { value.count }"#,
    )
    .unwrap();
    fs::write(
        root.join("build.omg"),
        r#"use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
            Build::application(builder, "packet_app");
            let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../api" }));
            Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
        }"#,
    )
    .unwrap();
    let compile = |arguments: &[&str], observation| {
        fs::write(
            root.join("main.omg"),
            format!(
                r#"use {{packet_api}}/main.omg as packet_api;
                   machine main() crashes Trap {{
                       transition {observation} == 41u32 {{ true -> done(), _ -> wrong() }}
                       machine done() {{}}
                       machine wrong() {{ crash Trap; }}
                   }}"#
            ),
        )
        .unwrap();
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&root)
            .args(arguments)
            .arg("build.omg")
            .output()
            .unwrap()
    };
    let publish = ["--emit-psi", "app.psi", "--target", "linux_x86_64"];
    let check = ["--check", "--target", "linux_x86_64"];
    for arguments in [check.as_slice(), publish.as_slice()] {
        let carried = compile(arguments, "|packet_api|::read_packet(|packet_api|::exported_packet())");
        assert!(carried.status.success(), "{carried:?}");
        let selected = compile(arguments, "|packet_api|::exported_packet().count");
        let diagnostic = String::from_utf8_lossy(&selected.stderr);
        assert!(!selected.status.success());
        assert!(
            diagnostic.contains("Packet::count")
                && diagnostic.contains("without direct dependency authority"),
            "{diagnostic}"
        );
    }
}
