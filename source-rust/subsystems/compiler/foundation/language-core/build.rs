use std::fmt::Write;
use std::path::Path;

use source_provenance::toolchain_owned_identity_digest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Saved programs carry these owners past the point where source files are available.
    let core = Path::new("../../../../../source/library/core");
    let mut constants = String::new();
    for declaration_file in [
        "extent",
        "filesystem_authority",
        "float_operations",
        "layout",
        "process_exit",
        "process_exit_native",
        "program_image",
    ] {
        let filename = format!("{declaration_file}.omg");
        let path = core.join(&filename);
        println!("cargo:rerun-if-changed={}", path.display());
        let source = std::fs::read(path)?;
        let revision = toolchain_owned_identity_digest("core", vec![filename], &source)?;
        let name = declaration_file.to_ascii_uppercase();
        writeln!(constants, "pub const {name}: [u8; 32] = {revision:?};")?;
    }
    let output = std::env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR");
    std::fs::write(Path::new(&output).join("core_revisions.rs"), constants)?;
    Ok(())
}
