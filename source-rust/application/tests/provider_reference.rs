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

fn compile_provider(
    directory: &TemporaryDirectory,
    source: &str,
    symbols: &str,
) -> std::path::PathBuf {
    let library = directory.path().join("libomega_reference.so");
    let provider = directory.path().join("provider.c");
    let version = directory.path().join("provider.map");
    std::fs::write(&provider, source).unwrap();
    std::fs::write(&version, symbols).unwrap();
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
    library
}

/// Writes the provider library, `main.omg` and the `build.omg` that binds the
/// provider, and returns the temporary directory and the source root.
fn write_provider_fixture(fixture: Fixture) -> (TemporaryDirectory, std::path::PathBuf) {
    let forward = fixture == Fixture::Forwarded;
    let directory = TemporaryDirectory::create("provider-reference", false).unwrap();
    let root = directory.path().join("source");
    std::fs::create_dir(&root).unwrap();
    let library = compile_provider(
        &directory,
        if forward {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\nint32_t omega_reference_read(const int32_t *value) { return *value; }\n"
        } else if fixture == Fixture::Pair {
            "#include <stdint.h>\nstruct Pair { int32_t left; int32_t right; };\nstatic const struct Pair cell = { 37, 91 };\nconst struct Pair *omega_reference_cell(void) { return &cell; }\n"
        } else {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\n"
        },
        if forward {
            "REFERENCE_1.0 { global: omega_reference_cell; omega_reference_read; local: *; };\n"
        } else {
            "REFERENCE_1.0 { global: omega_reference_cell; local: *; };\n"
        },
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
machine read(cell: @i32) -> i32 { $cell }
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
            r#"    Build::bind_native::<|app|::Sink::read>(builder, {target}, |native|::ForeignBinding::DllImport {{
        import: |native|::DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_read", version: "REFERENCE_1.0" }}
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
            r#"use main.omg as app;
use {{omega}}/language/core/external_binding.omg as native;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "provider_reference" }};
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Source::cell>(builder, {target}, |native|::ForeignBinding::DllImport {{
        import: |native|::DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_cell", version: "REFERENCE_1.0" }}
    }});
{forwarding_binding}}}
"#,
            target = profile.target_name(),
            library = library.display(),
        ),
    )
    .unwrap();
    (directory, root)
}

/// Exercise the emitted Linux program against the fixture's real provider library.
fn compiled_program(
    directory: &TemporaryDirectory,
    root: &std::path::Path,
) -> std::process::Output {
    let published = directory.path().join("compiled");
    compile(
        omega_command(directory, root)
            .args(["build", "--build-dir"])
            .arg(&published)
            .arg("build.omg"),
    );
    Command::new(published.join("omega-program"))
        .current_dir(root)
        .output()
        .unwrap()
}

fn publish_psi(directory: &TemporaryDirectory, root: &std::path::Path) -> Vec<u8> {
    let saved = directory.path().join("program.psi");
    compile(
        omega_command(directory, root)
            .arg("--emit-psi")
            .arg(&saved)
            .arg("build.omg"),
    );
    std::fs::read(saved).unwrap()
}

fn omega_command(directory: &TemporaryDirectory, root: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command
        .current_dir(root)
        .env(
            "OMEGA_LIBRARY_ROOT",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library"),
        )
        .env("TMPDIR", directory.path())
        .env_remove("OMEGA_HOST_PACKAGE_LOCK")
        .env_remove("OMEGA_HOST_PACKAGE_LOCK_SHA256")
        .stdin(std::process::Stdio::null());
    command
}

fn execute_provider_fixture(fixture: Fixture) {
    let forward = fixture == Fixture::Forwarded;
    let (directory, root) = write_provider_fixture(fixture);
    // The run names the Build file: `omega run <file>` selects the Build in
    // the named file and ignores a neighboring `build.omg`
    // (wiki/spec/build/command_line.md, "Build-first entry selection").
    let compiled = compiled_program(&directory, &root);
    assert!(compiled.status.success(), "{compiled:?}");
    assert!(
        compiled.stdout.is_empty() && compiled.stderr.is_empty(),
        "{compiled:?}"
    );
    let bytes = publish_psi(&directory, &root);
    let program = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let boundary = program
        .view()
        .module()
        .interfaces
        .boundary_machines
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
    let interpreted = omega_command(&directory, &root)
        .args(["run", "--keep", "build.omg"])
        .output()
        .unwrap();
    let report = String::from_utf8(interpreted.stderr).unwrap();
    if forward {
        assert!(!interpreted.status.success(), "{report}");
        assert!(
            report.contains("resident scalar belongs to a different original allocation"),
            "{report}"
        );
    } else {
        assert!(interpreted.status.success(), "{report}");
        assert!(interpreted.stdout.is_empty());
        assert!(report.lines().any(|line| line == "Psi exit 0"), "{report}");
        let kept = report
            .lines()
            .find_map(|line| line.strip_prefix("build dir kept: "))
            .unwrap();
        let kept = std::path::Path::new(kept);
        assert!(kept.starts_with(directory.path()), "{kept:?}");
        for stream in ["terminal.stdout", "terminal.stderr"] {
            assert!(std::fs::read(kept.join(stream)).unwrap().is_empty());
        }
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

/// The negative twin of the scalar program: run from `main.omg`, the program
/// gets core's default Build, which binds no provider for `Source::cell`. The
/// neighboring `build.omg` that binds one has no effect, so the compiler must
/// refuse the unsupplied requirement rather than find the binding anyway.
#[test]
fn the_program_file_alone_supplies_no_provider() {
    let (directory, root) = write_provider_fixture(Fixture::Scalar);
    let output = omega_command(&directory, &root)
        .args(["run", "main.omg"])
        .output()
        .unwrap();
    let report = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let target = target::TargetProfile::host().target_name();
    assert!(
        report.contains(&format!(
            "boundary requirement `Source::cell` has no provider on target `{target}`: \
             the Build selects none and the target package declares no default"
        )),
        "{report}"
    );
    assert!(!report.lines().any(|line| line == "Psi exit 0"), "{report}");
}

#[test]
fn native_table_uses_the_runtime_receiver_and_keeps_the_nonfirst_abi_argument() {
    let directory = TemporaryDirectory::create("provider-table", false).unwrap();
    let root = directory.path().join("source");
    std::fs::create_dir(&root).unwrap();
    let library = compile_provider(
        &directory,
        r"
#include <stdint.h>
struct protocol;
typedef int32_t (*operation)(int32_t, const struct protocol *);
struct protocol { uint8_t marker; operation apply; };
static int32_t add(int32_t value, const struct protocol *p) { return value + (int32_t)p->marker; }
static int32_t sub(int32_t value, const struct protocol *p) { return value - (int32_t)p->marker; }
static const struct protocol tables[] = {{7, add}, {3, sub}};
struct issued { struct protocol table; };
struct issued provide(uint32_t slot) { return (struct issued){tables[slot & 1]}; }
",
        "TABLE_1 { global: provide; local: *; };",
    );
    let program = r"
use {omega}/language/core/access/access.omg as policy;
use {omega}/language/core/c_layout.omg as abi;
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/reflect/reflect.omg as reflection;
public data ProtocolFields { marker: u8; apply: address; }
public data Protocol = |abi|::CLayout<ProtocolFields>;
public domain ProtocolFields::Initialized access ProtocolIssuance;
ProtocolIssuance satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflection|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        |policy|::new_plan(@schema, |layout|::Issuance::Only { routes: [
            |reflection|::callable(TableSource::provide)
        ] })
    }
}
public data IssuedTable { table: Protocol in ProtocolFields::Initialized; }
public boundary trait TableSource {
    machine provide(slot: u32) -> IssuedTable reaches none crashes none ensures result.table in ProtocolFields::Initialized;
}
public boundary trait Operation {
    machine apply(value: i32, table: @Protocol in ProtocolFields::Initialized) -> i32 reaches none crashes none;
}
public machine main() reaches none crashes Trap {
    let left: IssuedTable = TableSource::provide(0u32);
    let right: IssuedTable = TableSource::provide(1u32);
    let first: i32 = Operation::apply(25i32, @left.table);
    let second: i32 = Operation::apply(25i32, @right.table);
    transition first == 32i32 && second == 22i32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";
    std::fs::write(root.join("main.omg"), program).unwrap();
    let build = r#"use main.omg as app;
use {omega}/language/core/external_binding.omg as native;
use {omega}/language/core/reflect/reflect.omg as reflection;
machine build<'selection>(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "table_calls" };
    Build::bind(builder, TARGET::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::TableSource::provide>(builder, TARGET,
        |native|::ForeignBinding::DllImport { import: |native|::DllImport::ElfVersioned {
            object: "LIBRARY", symbol: "provide", version: "TABLE_1"
        } });
    let binding: |native|::FunctionFieldBinding<'selection> = |native|::FunctionFieldBinding {
        table: |reflection|::parameter(|app|::Operation::apply::table),
        field: |reflection|::field(|app|::ProtocolFields::apply),
    };
    Build::bind_native::<|app|::Operation::apply>(builder, TARGET, binding);
}
"#;
    let build = build
        .replace("TARGET", target::TargetProfile::host().target_name())
        .replace("LIBRARY", library.to_str().unwrap());
    std::fs::write(root.join("build.omg"), &build).unwrap();
    let observed = compiled_program(&directory, &root);
    assert!(observed.status.success(), "{observed:?}");

    // The checked table supplier and chosen dispatch field must also survive independent
    // host review and image publication after the source and SDK are unavailable.
    let saved = publish_psi(&directory, &root);
    let manifest = terminal_codec::CanonicalTerminalArtifact::from_bytes(&saved)
        .unwrap()
        .manifest()
        .identity()
        .to_string();
    let consumer = directory.path().join("consumer");
    std::fs::create_dir(&consumer).unwrap();
    std::fs::write(consumer.join("program.psi"), saved).unwrap();
    let target = target::TargetProfile::host().target_name();
    let review = omega_command(&directory, &root)
        .args([
            "packages",
            "update",
            "--project",
            ".",
            "--offline",
            "--target",
            target,
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "--dangerously-accept-all",
        ])
        .output()
        .unwrap();
    assert!(review.status.success(), "{review:?}");
    let lock = directory.path().join("approved.lock");
    std::fs::rename(root.join("omega.lock"), &lock).unwrap();
    let approval = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(std::fs::read(&lock).unwrap()))
    };
    let unavailable = directory.path().join("unavailable-source");
    std::fs::rename(&root, &unavailable).unwrap();
    let replay = omega_command(&directory, &consumer)
        .env("OMEGA_LIBRARY_ROOT", consumer.join("absent-sdk"))
        .env("OMEGA_HOST_PACKAGE_LOCK", &lock)
        .env("OMEGA_HOST_PACKAGE_LOCK_SHA256", approval)
        .args([
            "--trusted-artifact",
            &manifest,
            "--target",
            target,
            "--build-dir",
            "reopened",
            "program.psi",
        ])
        .output()
        .unwrap();
    assert!(replay.status.success(), "{replay:?}");
    let observed = Command::new(consumer.join("reopened/omega-program"))
        .status()
        .unwrap();
    assert!(observed.success(), "{observed}");
    std::fs::rename(unavailable, &root).unwrap();

    let other = "public data OtherTable { marker: u8; apply: address; }";
    std::fs::write(root.join("main.omg"), format!("{program}\n{other}")).unwrap();
    let wrong_owner = build.replace(
        "::field(|app|::ProtocolFields::apply)",
        "::field(|app|::OtherTable::apply)",
    );
    assert_ne!(build, wrong_owner);
    std::fs::write(root.join("build.omg"), wrong_owner).unwrap();
    let rejected = omega_command(&directory, &root)
        .args(["--target", target, "--emit-psi", "other.psi", "build.omg"])
        .output()
        .unwrap();
    let diagnostic = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success() && diagnostic.contains("different field"),
        "{diagnostic}"
    );
    std::fs::write(root.join("build.omg"), &build).unwrap();

    // The custody controls construct and modify ordinary semantic records.
    let ordinary_program = program.replace("|abi|::CLayout<ProtocolFields>", "ProtocolFields");
    let expected = "let left: IssuedTable = TableSource::provide(0u32);";
    let unissued = ordinary_program
        .replace(
            expected,
            "let left: Protocol = ProtocolFields { marker: 7, apply: 0 as address };",
        )
        .replace("@left.table)", "@left)");

    // Erasing the requirement's input qualification permits ordinary records
    // in source. Native publication must still require evidence for the table.
    let (declarations, _) = ordinary_program.split_once("machine main()").unwrap();
    let declarations = declarations.replace(
        "table: @Protocol in ProtocolFields::Initialized",
        "table: @Protocol",
    );
    let ordinary = r"machine main() reaches Operation invokes Operation crashes none {
    let marker: u64 = 42u64;
    let fake: Protocol = ProtocolFields { marker: 7u8, apply: address_of(@marker) };
    _ = Operation::apply(25i32, @fake);
}";
    let unrestricted = format!("{declarations}{ordinary}");
    std::fs::write(root.join("main.omg"), unrestricted).unwrap();
    let unchecked = omega_command(&directory, &root)
        .args(["--target", target, "--build-dir"])
        .arg(directory.path().join("unchecked"))
        .arg("build.omg")
        .output()
        .unwrap();
    assert!(!unchecked.status.success());
    let diagnostic = String::from_utf8_lossy(&unchecked.stderr);
    assert!(
        diagnostic.contains("native function table needs provenance"),
        "{diagnostic}"
    );

    let overwritten = ordinary_program
        .replace(
            expected,
            "let mutable left: IssuedTable = TableSource::provide(0u32);",
        )
        .replace(
            "    let first:",
            "    left.table.apply = 0 as address;\n    let first:",
        );
    for (input, reason) in [
        (unissued, "carries no domain"),
        (overwritten, "establishment routes"),
    ] {
        std::fs::write(root.join("main.omg"), input).unwrap();
        let outcome = omega_command(&directory, &root)
            .args(["--target", target, "--emit-psi", "refused.psi", "build.omg"])
            .output()
            .unwrap();
        let diagnostic = String::from_utf8_lossy(&outcome.stderr);
        assert!(
            !outcome.status.success() && diagnostic.contains(reason),
            "{diagnostic}"
        );
    }
}
