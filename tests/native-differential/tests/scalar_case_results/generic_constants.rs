//! Closed generic constant invocations reach native execution without source.

use super::{CanonicalTerminalArtifact, NativeTarget, membership, publish};

struct Sources(std::path::PathBuf);

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_native_constant(carrier: &str, initializer: &str, driver: &str) {
    assert_native_source(
        carrier,
        &format!(
            "module settings;
         machine identity<T>(value: T) -> T {{ value }}
         machine forward<T>(value: T) -> T {{ identity<T>(value) }}
         machine amount<const N: u64>() -> u64 {{ N }}
         pub const VALUE: {carrier} = {initializer};"
        ),
        carrier,
        "settings::VALUE",
        driver,
    );
}

fn assert_native_source(
    label: &str,
    declarations: &str,
    carrier: &str,
    expression: &str,
    driver: &str,
) {
    let directory = std::env::temp_dir().join(format!(
        "omega-native-generic-constant-{}-{label}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let sources = Sources(directory);
    let root = sources.0.join("main.omg");
    std::fs::write(sources.0.join("settings.omg"), declarations).unwrap();
    std::fs::write(
        &root,
        format!("use settings; machine read_constant() -> {carrier} {{ {expression} }}"),
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&root, None))
        .expect("closed generic constant passes ordinary source compilation");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read_constant")
        .produce_artifact()
        .expect("generic constant reaches Terminal");
    let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    drop(checked);
    drop(sources);
    assert!(
        !root.exists(),
        "native publication cannot read original source"
    );
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        publish(&artifact, target);
    }
    membership::execute(&artifact, driver);
}

#[test]
fn nested_generic_record_tables_execute_after_source_removal() {
    let declarations = "module settings;
        pub data Cell<T [copy]> [copy] { value: T; }
        pub data Row<T [copy]> [copy] { cells: [Cell<T>; 2]; }
        pub const TABLE: [Row<u64>; 2] = [
            Row { cells: [Cell { value: 7 }, Cell { value: 9 }] },
            Row { cells: [Cell { value: 18364758544493064720 }, Cell { value: 31 }] }
        ];
        pub const FLAGS: [Cell<bool>; 2] = [Cell { value: false }, Cell { value: true }];
        pub const COPIED: [Row<u64>; 2] = TABLE;";
    assert_native_source(
        "record-table-integer",
        declarations,
        "u64",
        "settings::TABLE[1].cells[0].value",
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == UINT64_C(0xfedcba9876543210) ? 0 : 1; }",
    );
    assert_native_source(
        "record-table-copied",
        declarations,
        "u64",
        "settings::COPIED[1].cells[0].value",
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == UINT64_C(0xfedcba9876543210) ? 0 : 1; }",
    );
    assert_native_source(
        "record-table-boolean",
        declarations,
        "bool",
        "settings::FLAGS[1].value",
        "#include <stdbool.h>\nextern bool omega_entry(void);\nint main(void) { return omega_entry() ? 0 : 1; }",
    );
}

#[test]
fn nested_type_and_const_applications_execute_after_source_removal() {
    assert_native_constant(
        "u64",
        "forward<u64>(identity<u64>(amount<7>())) + amount<9>()",
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\n\
         int main(void) { return omega_entry() == UINT64_C(16) ? 0 : 1; }",
    );
    assert_native_constant(
        "bool",
        "forward<bool>(true)",
        "#include <stdbool.h>\nextern bool omega_entry(void);\n\
         int main(void) { return omega_entry() ? 0 : 1; }",
    );
}

#[test]
fn generic_floating_helpers_preserve_native_negative_zero() {
    for (carrier, c_type, bits_type, macro_name, bits) in [
        ("f32", "float", "uint32_t", "UINT32_C", "80000000"),
        ("f64", "double", "uint64_t", "UINT64_C", "8000000000000000"),
    ] {
        assert_native_constant(
            carrier,
            &format!("forward<{carrier}>(identity<{carrier}>(-0.0{carrier}))"),
            &format!(
                "#include <stdint.h>\n#include <string.h>\n\
                 extern {c_type} omega_entry(void);\n\
                 int main(void) {{ {c_type} value = omega_entry(); {bits_type} bits;\n\
                     memcpy(&bits, &value, sizeof(bits));\n\
                     return bits == {macro_name}(0x{bits}) ? 0 : 1; }}"
            ),
        );
    }
}
