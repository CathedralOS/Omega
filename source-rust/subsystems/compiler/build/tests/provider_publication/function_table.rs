//! The Build's plain data must agree with the Product signature and field it describes.

use semantic_vocabulary::NativeBindingLocator;
use target::TargetProfile;

#[test]
fn table_bindings_check_unreached_requirements_and_their_borrowed_fields() {
    let product = r"
public data Protocol { reset: address; output: address; revision: u64; }
public data Other { reset: address; output: address; }
public boundary trait Firmware {
    machine write(text: @[u16; 2], table: @Protocol) reaches none crashes none;
    machine clear(text: @[u16; 2], table: @Protocol) reaches none crashes none;
}
machine main() {}
";
    for (formal, field, expected) in [
        ("Firmware::write::table", "Protocol::output", None),
        (
            "Firmware::clear::table",
            "Protocol::output",
            Some("different requirement parameter"),
        ),
        (
            "Firmware::write::table",
            "Other::output",
            Some("different field, owner application or source revision"),
        ),
        (
            "Firmware::write::table",
            "Protocol::revision",
            Some("live, non-atomic address field"),
        ),
    ] {
        let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
            "function-table-product",
            false,
        )
        .unwrap();
        let root = scratch.path().join("build.omg");
        let build = r#"use {omega}/language/core/external_binding.omg as native;
use {omega}/language/core/reflect/reflect.omg as reflect;
machine build<'selection>(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "firmware" };
    let binding: |native|::FunctionFieldBinding<'selection> = |native|::FunctionFieldBinding {
        table: |reflect|::parameter(FORMAL),
        field: |reflect|::field(FIELD),
    };
    Build::bind_native::<Firmware::write>(builder, linux_x86_64, binding);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
"#;
        std::fs::write(
            &root,
            build.replace("FORMAL", formal).replace("FIELD", field),
        )
        .unwrap();
        std::fs::write(scratch.path().join("main.omg"), product).unwrap();
        let target = TargetProfile::LinuxX64;
        let project = super::open_project(&root, &[target]);
        let rows = &project.evaluated().configuration.native_binding_requests;
        let [row] = rows.as_slice() else {
            panic!("one executed function field binding")
        };
        let NativeBindingLocator::FunctionField(binding) = &row.locator else {
            panic!("Build lost its selected table");
        };
        assert_eq!(binding.table.declaration_position, 1);
        assert_eq!(
            binding.field.declaration_position,
            if field == "Protocol::revision" { 2 } else { 1 }
        );
        let checked = project.product_stage(&[target]).and_then(|stage| {
            source_compiler::compile_for_target(stage.source(), target.target_name(), None)
                .map(|_| ())
        });
        match expected {
            None => checked.unwrap(),
            Some(message) => assert!(
                checked
                    .unwrap_err()
                    .iter()
                    .any(|error| error.message.contains(message)),
                "{message}"
            ),
        }
    }
}
