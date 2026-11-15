use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use orchestration::{
    BuildError, BuildReport, BuildRequest, BuildTargets, NativeDestination, ProjectProduct, build,
};

fn stage_project(name: &str, source: &str, build: &str) -> PathBuf {
    let project =
        std::env::temp_dir().join(format!("omega-psi-runtime-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&project).expect("staging directory");
    std::fs::write(project.join("main.omg"), source).expect("staged program");
    std::fs::write(project.join("build.omg"), build).expect("staged Build");
    project
}

fn assert_psi_runs(name: &str, source: &str) {
    let project = stage_project(
        name,
        source,
        r#"machine build(builder: &mut Build) {
    Build::application(builder, "structural_domain_issuer");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--engine", "psi", "main.omg"])
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    std::fs::remove_dir_all(&project).expect("clean staged project");

    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{report}");
    assert!(
        report.lines().any(|line| line == "terminal exit 0"),
        "{report}",
    );
}

#[test]
fn psi_runs_an_owned_record_sequence_literal() {
    assert_psi_runs(
        "owned-sequence",
        "data Ticket { number: u32; }
machine main() {
    let tickets: [Ticket] = [Ticket { number: 11u32 }];
}
",
    );
}

#[test]
fn psi_moves_a_returned_owned_sequence_into_a_by_value_argument() {
    assert_psi_runs(
        "move-owned-sequence",
        "data Ticket { number: u32; }
machine main() {
    consume(make(11u32));
}
machine make(number: u32) -> [Ticket] {
    [Ticket { number: number }]
}
machine consume(tickets: [Ticket]) {}
",
    );
}

#[test]
fn psi_runs_the_bundled_atomic_fence_in_its_module() {
    assert_psi_runs(
        "atomic-fence",
        r#"use omega::language::core::Atomic;
use omega::language::core::Atomic::fence;
use omega::language::core::FenceOrdering;
machine main() {
    Atomic::fence(FenceOrdering::Receive);
    fence(FenceOrdering::Publish);
    Atomic::fence(FenceOrdering::ReceivePublish);
}
"#,
    );
}

#[test]
fn psi_runs_a_named_sum_constant_with_its_case_payload() {
    assert_psi_runs(
        "sum-constant",
        r#"data Policy [copy] {
    case Read(number: u64);
    case Write(number: u64);
}
const selected: Policy = Policy::Read { number: 7u64 };
machine main() crashes Trap {
    let result: u64 = match selected {
        Policy::Read { number } -> number
        _ -> 99u64
    };
    transition result == 7u64 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
    );
}

fn compile(project: &Path, destination: NativeDestination) -> Result<BuildReport, BuildError> {
    let outcomes = build(BuildRequest {
        root_path: project.join("main.omg"),
        build_directory: None,
        targets: BuildTargets::Single(target::TargetProfile::MacosArm64),
        product: ProjectProduct::Native {
            destination,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
        },
        timings: false,
        offline: false,
        verify: false,
        require_package_project: false,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })?;
    let Ok([outcome]): Result<[orchestration::TargetBuildOutcome; 1], _> = outcomes.try_into()
    else {
        panic!("one selected program target");
    };
    outcome.result?.into_report()
}

#[test]
fn psi_admission_retains_the_native_borrowed_storage_layouts() {
    let build = r#"machine build(builder: &mut Build) {
    Build::application(builder, "checked_read_geometry");
    Build::select_provider<ReadCell, Reader>(builder, macos_arm64);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}
"#;
    let project = stage_project(
        "borrowed-layout",
        "data Cell [copy] { prefix: u32; word: u32; suffix: u32; }
public boundary trait ReadCell {
    machine word(cell: &Cell) -> u32 reaches crashes none;
}
Reader satisfies ReadCell {
    machine word(cell: &Cell) -> u32 { cell.word }
}
machine main() crashes Trap {
    let cell: Cell = Cell { prefix: 7, word: 11, suffix: 13 };
    let value: u32 = ReadCell::word(&cell);
    transition value == 11 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
",
        build,
    );
    let psi = compile(&project, NativeDestination::RuntimeAdmission);
    let native = compile(&project, NativeDestination::Retained);

    let unbound = build.replace(
        "    Build::select_provider<ReadCell, Reader>(builder, macos_arm64);\n",
        "",
    );
    std::fs::write(project.join("build.omg"), unbound).expect("staged missing provider");
    let refused = compile(&project, NativeDestination::RuntimeAdmission);
    std::fs::remove_dir_all(&project).expect("clean staged project");

    let psi = psi.expect("admit the checked provider and its runtime geometry");
    let native = native.expect("realize the same admitted provider");
    let psi_layouts = psi
        .report
        .runtime_provisions()
        .expect("Psi runtime provisions")
        .borrowed_storage_layouts();
    let native_layouts = native
        .report
        .runtime_provisions()
        .expect("native runtime provisions")
        .borrowed_storage_layouts();
    assert_eq!(psi_layouts, native_layouts);
    let record = psi_layouts
        .iter()
        .find(|layout| layout.byte_size == 12 && layout.alignment == 4)
        .expect("borrowed Cell geometry");
    let offsets: Vec<_> = record
        .fields()
        .iter()
        .map(|field| field.byte_offset)
        .collect();
    assert_eq!(offsets, [0, 4, 8]);

    let Err(refusal) = refused else {
        panic!("a missing provider must prevent runtime admission");
    };
    let diagnostic = refusal.to_string();
    assert!(
        diagnostic.contains("boundary requirement `ReadCell::word` has no provider"),
        "{diagnostic}"
    );
}
