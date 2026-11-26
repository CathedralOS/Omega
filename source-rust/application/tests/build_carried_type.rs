//! Product publication distinguishes a carried result from selecting its field.
#[path = "support/project.rs"]
mod project;

use std::process::Command;

#[test]
fn passing_a_transitive_result_keeps_its_type_without_granting_member_access() {
    let scratch = project::stage(
        "carried-result",
        &[
            (
                "supplier/build.omg",
                r#"machine build(builder: @mutable Build) { Build::package(builder, "packet_library"); }"#,
            ),
            (
                "supplier/main.omg",
                "public data Packet [copy] { count: u32; }
         public machine packet() -> Packet reaches none crashes none { Packet { count: 41u32 } }",
            ),
            (
                "api/build.omg",
                r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
            Build::package(builder, "packet_api");
            let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../supplier" }));
        }"#,
            ),
            (
                "api/main.omg",
                r#"use packet_library\Packet;
           use packet_library\packet;
           public machine exported_packet() -> Packet reaches none crashes none { packet() }
           public machine read_packet(value: Packet) -> u32 reaches none crashes none { value.count }"#,
            ),
            (
                "consumer/build.omg",
                r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
            Build::application(builder, "packet_app");
            let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../api" }));
            Build::bind(builder, linux_x86_64::ProgramEntry, main);
        }"#,
            ),
        ],
    );
    let root = scratch.path().join("consumer");
    let compile = |arguments: &[&str], observation| {
        let main = format!(
            r#"use packet_api\exported_packet;
                   use packet_api\read_packet;
                   machine main() crashes Trap {{
                       transition {observation} == 41u32 {{ true -> done(), _ -> wrong() }}
                       machine done() {{}}
                       machine wrong() {{ crash Trap; }}
                   }}"#
        );
        project::write(&root, &[("main.omg", &main)]);
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
        let carried = compile(arguments, "read_packet(exported_packet())");
        assert!(carried.status.success(), "{carried:?}");
        let selected = compile(arguments, "exported_packet().count");
        let diagnostic = String::from_utf8_lossy(&selected.stderr);
        assert!(!selected.status.success());
        assert!(
            diagnostic.contains("Packet::count")
                && diagnostic.contains("without direct dependency authority"),
            "{diagnostic}"
        );
    }
}
