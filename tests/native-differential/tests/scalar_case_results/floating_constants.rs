//! Nested ordinary helper calls preserve floating constant bits after source removal.

use super::{CanonicalTerminalArtifact, NativeTarget, membership, publish};

struct Sources(std::path::PathBuf);

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_constant_bits(format: &str, literal: &str, expected_bits: u64) {
    let directory = std::env::temp_dir().join(format!(
        "omega-native-floating-constant-{}-{format}-{expected_bits:x}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let sources = Sources(directory);
    let root = sources.0.join("main.omg");
    std::fs::write(
        sources.0.join("settings.omg"),
        format!(
            "module settings;
             machine retain(value: {format}) -> {format} {{ value }}
             pub const VALUE: {format} = retain(retain({literal}));"
        ),
    )
    .unwrap();
    std::fs::write(
        &root,
        format!(
            "use settings;
             machine read_constant() -> {format} {{ settings::VALUE }}"
        ),
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&root, None))
        .expect("nested floating helper calls pass ordinary source compilation");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read_constant")
        .produce_artifact()
        .expect("floating constant reaches Terminal");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    drop(checked);
    drop(sources);
    assert!(
        !root.exists(),
        "native publication must not retain source files"
    );

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    let (result_type, bits_type, integer_macro) = match format {
        "f32" => ("float", "uint32_t", "UINT32_C"),
        "f64" => ("double", "uint64_t", "UINT64_C"),
        _ => panic!("unsupported floating fixture format"),
    };
    let driver = format!(
        "#include <stdint.h>\n#include <string.h>\n\
         extern {result_type} omega_entry(void);\n\
         int main(void) {{\n\
             {result_type} value = omega_entry();\n\
             {bits_type} bits;\n\
             memcpy(&bits, &value, sizeof(bits));\n\
             return bits == {integer_macro}(0x{expected_bits:x}) ? 0 : 1;\n\
         }}"
    );
    membership::execute(&artifact, &driver);
}

#[test]
fn nested_f32_constant_helpers_preserve_native_bits() {
    assert_constant_bits("f32", "1.5f32", u64::from(1.5_f32.to_bits()));
    assert_constant_bits("f32", "-0.0f32", u64::from((-0.0_f32).to_bits()));
}

#[test]
fn nested_f64_constant_helpers_preserve_native_bits() {
    assert_constant_bits("f64", "1.5f64", 1.5_f64.to_bits());
    assert_constant_bits("f64", "-0.0f64", (-0.0_f64).to_bits());
}
