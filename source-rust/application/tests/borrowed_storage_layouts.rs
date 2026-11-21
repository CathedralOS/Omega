use compiler::{Artifact, CompileRequest};
use product_publication::CompletedProduct;
use std::path::{Path, PathBuf};
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

fn stage_project(name: &str, source: &str, build: &str) -> PathBuf {
    let project =
        std::env::temp_dir().join(format!("omega-psi-runtime-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&project).expect("staging directory");
    std::fs::write(project.join("main.omg"), source).expect("staged program");
    std::fs::write(project.join("build.omg"), build).expect("staged Build");
    project
}

#[test]
fn psi_admission_retains_the_native_borrowed_storage_layouts() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    package_compilation::toolchain_library::configure(&library).unwrap();
    let build = r#"machine build(builder: @mut Build) {
    Build::application(builder, "checked_read_geometry");
    Build::select_provider::<ReadCell, Reader>(builder, TARGET);
    Build::bind(builder, TARGET\ProgramEntry, main);
}
"#
    .replace("TARGET", TargetProfile::host().target_name());
    let project = stage_project(
        "borrowed-layout",
        "data Cell [copy] { prefix: u32; word: u32; suffix: u32; }
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
",
        &build,
    );
    let psi = compile(&project, Product::Runtime);
    let native = compile(&project, Product::Native);

    let execution = std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&project)
        .env("OMEGA_LIBRARY_ROOT", &library)
        .output()
        .expect("run the selected provider on both engines");

    let unbound = build
        .lines()
        .filter(|line| !line.contains("Build::select_provider"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(project.join("build.omg"), unbound).expect("staged missing provider");
    let refused = compile(&project, Product::Runtime);
    std::fs::remove_dir_all(&project).expect("clean staged project");

    let report = String::from_utf8_lossy(&execution.stderr);
    assert!(execution.status.success(), "{report}");
    assert!(
        report.lines().any(|line| line == "native exit 0"),
        "{report}"
    );
    assert!(
        report.lines().any(|line| line == "interp exit: 0"),
        "{report}"
    );

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
