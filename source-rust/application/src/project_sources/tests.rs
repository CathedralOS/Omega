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
    let profile = TargetProfile::MacosArm64;
    let product = super::open_product(
        &project.join("main.omg"),
        profile,
        true,
        None,
        None,
    )?;
    let stage = product.product_stage(&[profile])?;
    let program = stage.program(profile, Vec::new())?;
    match requested {
        Product::Runtime => match compiler::compile(CompileRequest::Runtime {
            program,
            entry_memory: &[],
        })? {
            Artifact::Runtime(product) => Ok(product),
            _ => panic!("expected runtime admission"),
        },
        Product::Native => match compiler::compile(CompileRequest::Native {
            program,
            entry_memory: &[],
        })? {
            Artifact::Native(product) => Ok(product),
            _ => panic!("expected native compilation"),
        },
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
    Build::select_provider<ReadCell, Reader>(builder, macos_arm64);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}
"#;
    let project = stage_project(
        "borrowed-layout",
        "data Cell [copy] { prefix: u32; word: u32; suffix: u32; }
public boundary trait ReadCell {
    machine word(cell: @Cell) -> u32 reaches crashes none;
}
Reader satisfies ReadCell {
    machine word(cell: @Cell) -> u32 { cell.word }
}
machine main() crashes Trap {
    let cell: Cell = Cell { prefix: 7, word: 11, suffix: 13 };
    let value: u32 = ReadCell::word(@cell);
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
    let psi = compile(&project, Product::Runtime);
    let native = compile(&project, Product::Native);

    let unbound = build.replace(
        "    Build::select_provider<ReadCell, Reader>(builder, macos_arm64);\n",
        "",
    );
    std::fs::write(project.join("build.omg"), unbound).expect("staged missing provider");
    let refused = compile(&project, Product::Runtime);
    std::fs::remove_dir_all(&project).expect("clean staged project");

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
