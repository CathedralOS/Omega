#[path = "support/project.rs"]
mod project;

use compiler::{Artifact, CompileRequest};
use product_publication::CompletedProduct;
use std::path::Path;
use target::TargetProfile;

#[derive(Clone, Copy)]
enum Product {
    Runtime,
    Native,
}

fn compile(
    project: &Path,
    requested: Product,
) -> Result<CompletedProduct, Vec<diagnostics::Diagnostic>> {
    let profile = TargetProfile::host();
    let product = package_manager::project::open_product(
        &project.join("build.omg"),
        profile,
        true,
        &project.join("build"),
        None,
    )?;
    let stage = product.product_stage(&[profile])?;
    let program = stage.program(profile, Vec::new())?;
    let request = match requested {
        Product::Runtime => CompileRequest::Runtime {
            program,
            entry_memory: &[],
        },
        Product::Native => CompileRequest::Native {
            program,
            entry_memory: &[],
        },
    };
    let compiled = compiler::compile(request, Default::default())
        .map_err(compiler::stage_validators::NativeValidationError::into_diagnostics)?;
    match (requested, compiled.artifact) {
        (Product::Runtime, Artifact::Runtime(product))
        | (Product::Native, Artifact::Native(product)) => Ok(product),
        _ => panic!("unexpected compilation product"),
    }
}

#[test]
fn psi_admission_retains_the_native_borrowed_storage_layouts() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    package_compilation::toolchain_library::configure(&library).unwrap();
    let build = r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "checked_read_geometry" };
    Build::select_provider::<|app|::ReadCell, |app|::Reader>(builder, TARGET);
    Build::bind(builder, TARGET::ProgramEntry, |app|::main);
}
"#
    .replace("TARGET", TargetProfile::host().target_name());
    let source = "data Cell [copy] { prefix: u32; word: u32; suffix: u32; }
public boundary trait ReadCell {
    machine word(cell: @Cell) -> u32 reaches none crashes none;
}
Reader satisfies ReadCell {
    machine word(cell: @Cell) -> u32 { cell.word }
}
machine main() crashes Trap {
    let cell: Cell = Cell { prefix: 7, word: 11, suffix: 13 };
    let value: u32 = ReadCell::word(@cell);
    transition value == 11 {
        true -> done(),
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
";
    let scratch = project::stage(
        "psi-runtime-borrowed-layout",
        &[("main.omg", source), ("build.omg", &build)],
    );
    let project = scratch.path();
    let psi = compile(project, Product::Runtime);
    let native = compile(project, Product::Native);

    for command in ["run", "build"] {
        let execution = project::observe_command(
            std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
                .args([command, "build.omg"])
                .current_dir(project)
                .env("OMEGA_LIBRARY_ROOT", &library),
        );

        let report = String::from_utf8_lossy(&execution.stderr);
        assert!(execution.status.success(), "{report}");
    }

    let unbound = build
        .lines()
        .filter(|line| !line.contains("Build::select_provider"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(project.join("build.omg"), unbound).expect("staged missing provider");
    let refused = compile(project, Product::Runtime);
    drop(scratch);

    let psi = psi.expect("admit the checked provider and its runtime geometry");
    let native = native.expect("realize the same admitted provider");
    let psi_layouts = psi
        .runtime_provisions()
        .expect("Psi runtime provisions")
        .borrowed_storage_layouts();
    let native_layouts = native
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

    // Preserve the type ID, field paths, size and alignment, but reverse the
    // outer field offsets. The forged table has a valid canonical encoding.
    let wrong_record = runtime_abi::NativeBorrowedStorageLayout::new(
        record.structural_type,
        record.byte_size,
        record.alignment,
        record.fields().iter().map(|field| {
            (
                record.field_path(field).collect(),
                record.field_repetitions(field).to_vec(),
                8 - field.byte_offset,
                field.logical_scalar,
                field.stored_scalar,
            )
        }),
        [],
        [],
    );
    let mut changed = native_layouts.to_vec();
    let slot = changed
        .iter_mut()
        .find(|row| row.structural_type == record.structural_type)
        .unwrap();
    *slot = wrong_record;
    let changed = runtime_abi::NativeBorrowedStorageLayouts::seal(changed).unwrap();
    let admitted = psi.runtime_provisions().unwrap().host_provisions();
    assert_eq!(
        admitted.require_borrowed_storage_layouts(&changed),
        Err("borrowed storage geometry differs from the admitted runtime input"),
    );

    let Err(refusal) = refused else {
        panic!("a missing provider must prevent runtime admission");
    };
    let mut diagnostic = String::new();
    diagnostics::write_diagnostics(&mut diagnostic, &refusal).unwrap();
    assert!(
        diagnostic.contains("boundary requirement `ReadCell::word` has no provider"),
        "{diagnostic}"
    );
}

#[test]
fn an_entryless_package_checks_the_firmware_table_recast_against_its_policy() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    let tables =
        std::fs::read_to_string(library.join("system/targets/uefi_x86_64/tables.omg")).unwrap();
    let build = r#"use main.omg;
machine build(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: "table_views" }; }"#;
    let scratch = project::stage(
        "review-table-view",
        &[("build.omg", build), ("tables.omg", &tables)],
    );
    let write = |width| {
        std::fs::write(
            scratch.path().join("main.omg"),
            format!(
                r#"use tables.omg;
public machine inspect(table: @|tables|::EfiSystemTable) -> u8 {{
    let bytes: @[u8; {width}] = table as @[u8; {width}];
    bytes[0]
}}"#
            ),
        )
        .unwrap();
    };
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(scratch.path())
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args(args)
            .output()
            .unwrap()
    };
    let audit = [
        "packages",
        "audit",
        "--offline",
        "--target",
        "windows_x86_64",
    ];

    write(120);
    let valid = run(&audit);
    let report = String::from_utf8_lossy(&valid.stdout);
    assert!(
        valid.status.success() && report.contains("fresh-analysis complete"),
        "{report}"
    );
    let emitted = run(&[
        "--emit-psi",
        "table.psi",
        "--target",
        "windows_x86_64",
        "build.omg",
    ]);
    assert!(
        emitted.status.success(),
        "{}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    let bytes = std::fs::read(scratch.path().join("table.psi")).unwrap();
    let program = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = program.semantic_module();
    let mut layouts = Vec::new();
    for application in &module.layouts.target_layout_applications {
        for plan in &application.realizations {
            assert_eq!(plan.target, "windows_x86_64");
            let policy = module.identities.text(application.policy_identity).unwrap();
            let expected = match plan.storage.byte_size {
                24 => "CAbiPolicy",
                120 => "EfiSystemTableViewLayout",
                size => panic!("unexpected policy {policy} with size {size}"),
            };
            assert!(policy.contains(expected), "{policy}");
            let offsets: Vec<_> = plan
                .fields
                .iter()
                .map(|field| field.storage.byte_offset().unwrap())
                .collect();
            layouts.push((plan.storage.byte_size, plan.storage.byte_alignment, offsets));
        }
    }
    layouts.sort();
    assert_eq!(
        layouts,
        [
            (24, 8, vec![0, 8, 12, 16, 20]),
            (
                120,
                8,
                vec![0, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112]
            ),
        ]
    );

    write(119);
    let invalid = run(&audit);
    let report = String::from_utf8_lossy(&invalid.stdout);
    assert!(!invalid.status.success());
    assert!(
        report.contains("reference formation changes its captured referent type"),
        "{report}"
    );
}
