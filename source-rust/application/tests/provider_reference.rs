//! A canonical safe boundary contract retains a real provider-owned cell.
//! Exercise ordinary produced provisions, rather than a symbol-specific model.

#![cfg(target_os = "linux")]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::Command;

fn compile(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {:?}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fixture {
    Scalar,
    Forwarded,
    Pair,
}

fn execute_provider_fixture(fixture: Fixture) {
    let forward = fixture == Fixture::Forwarded;
    let directory = TemporaryDirectory::create("provider-reference", false).unwrap();
    let root = directory.path().join("source");
    std::fs::create_dir(&root).unwrap();
    let library = directory.path().join("libomega_reference.so");
    let provider = directory.path().join("provider.c");
    let version = directory.path().join("provider.map");
    std::fs::write(
        &provider,
        if forward {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\nint32_t omega_reference_read(const int32_t *value) { return *value; }\n"
        } else if fixture == Fixture::Pair {
            "#include <stdint.h>\nstruct Pair { int32_t left; int32_t right; };\nstatic const struct Pair cell = { 37, 91 };\nconst struct Pair *omega_reference_cell(void) { return &cell; }\n"
        } else {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\n"
        },
    )
    .unwrap();
    std::fs::write(
        &version,
        if forward {
            "REFERENCE_1.0 { global: omega_reference_cell; omega_reference_read; local: *; };\n"
        } else {
            "REFERENCE_1.0 { global: omega_reference_cell; local: *; };\n"
        },
    )
    .unwrap();
    compile(
        Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args([
                "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg(format!("-Wl,--version-script={}", version.display()))
            .arg(&provider)
            .arg("-o")
            .arg(&library),
    );
    std::fs::write(
        root.join("main.omg"),
        if forward {
            r#"
boundary trait Source { machine cell() -> @i32 reaches none crashes none; }
boundary trait Sink { machine read(cell: @i32) -> i32 reaches none crashes none; }
machine main() reaches Source + Sink invokes Source invokes Sink crashes Trap {
    let observed: i32 = Sink::read(Source::cell());
    transition observed == 37i32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#
        } else if fixture == Fixture::Pair {
            r#"
data Pair { left: i32; right: i32; }
boundary trait Source { machine cell() -> @Pair reaches none crashes none; }
machine matches(pair: @Pair) -> bool { pair.left == 37i32 && pair.right == 91i32 }
machine main() reaches Source invokes Source crashes Trap {
    let observed: bool = matches(Source::cell());
    transition observed { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#
        } else {
            r#"
boundary trait Source { machine cell() -> @i32 reaches none crashes none; }
machine read(cell: @i32) -> i32 { cell }
machine main() reaches Source invokes Source crashes Trap {
    let observed: i32 = read(Source::cell());
    transition observed == 37i32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#
        },
    )
    .unwrap();
    let profile = target::TargetProfile::host();
    let forwarding_binding = if forward {
        format!(
            r#"    Build::bind::<Sink::read>(builder, {target}, ForeignBinding::DllImport {{
        import: DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_read", version: "REFERENCE_1.0" }}
    }});
"#,
            target = profile.target_name(),
            library = library.display()
        )
    } else {
        String::new()
    };
    std::fs::write(
        root.join("build.omg"),
        format!(
            r#"
use omega::language::core::DllImport;
use omega::language::core::ForeignBinding;
machine build(builder: @mut Build) {{
    Build::application(builder, "provider_reference");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
    Build::bind::<Source::cell>(builder, {target}, ForeignBinding::DllImport {{
        import: DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_cell", version: "REFERENCE_1.0" }}
    }});
{forwarding_binding}}}
"#,
            target = profile.target_name(),
            library = library.display(),
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "--keep", "main.omg"])
        .current_dir(&root)
        .env(
            "OMEGA_LIBRARY_ROOT",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library"),
        )
        .env("TMPDIR", directory.path())
        .env_remove("OMEGA_HOST_PACKAGE_LOCK")
        .env_remove("OMEGA_HOST_PACKAGE_LOCK_SHA256")
        .output()
        .unwrap();
    let report = String::from_utf8(output.stderr).unwrap();
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert!(
        report.lines().any(|line| line == "native exit 0"),
        "{report}"
    );
    let retained = report
        .lines()
        .find_map(|line| line.strip_prefix("build dir kept: "))
        .unwrap();
    let retained = std::path::Path::new(retained);
    assert!(retained.starts_with(directory.path()), "{retained:?}");
    let bytes = std::fs::read(retained.join("program.oterm")).unwrap();
    let program = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let boundary = program
        .view()
        .module()
        .interfaces.boundary_machines
        .iter()
        .find(|boundary| {
            matches!(
                boundary.result,
                terminal_psi::BoundaryMachineResult::Structural(_)
            )
        })
        .expect("the produced program retains its reference boundary");
    assert!(boundary.result_reference_regions.is_empty());
    let groups = boundary.result_lifetimes.as_ref().unwrap();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].inputs.is_empty());
    assert_eq!(groups[0].results, vec![Vec::new()]);
    if forward {
        assert!(!output.status.success(), "{report}");
        assert!(
            report.contains("resident scalar belongs to a different original allocation"),
            "{report}"
        );
    } else {
        assert!(output.status.success(), "{report}");
        assert!(
            report.lines().any(|line| line == "interp exit: 0"),
            "{report}"
        );
        assert!(
            std::fs::read(retained.join("terminal.stdout"))
                .unwrap()
                .is_empty()
        );
        assert!(
            std::fs::read(retained.join("terminal.stderr"))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn canonical_provider_reference_reads_its_live_cell() {
    execute_provider_fixture(Fixture::Scalar);
}

#[test]
fn provider_reference_forwarding_requires_actual_pointer_authority() {
    execute_provider_fixture(Fixture::Forwarded);
}

#[test]
fn canonical_provider_reference_reads_both_fields_of_its_live_pair() {
    execute_provider_fixture(Fixture::Pair);
}
