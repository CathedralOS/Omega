//! Early ordinary evaluation and deferred selected evaluation share each source interval.

use super::*;
use compiler::CheckedCompileRequest;

const FLOAT_LENGTH: &str = r#"
use omega::language::core::float_operations;
machine float_length() -> u64 {
    let left: f64 = 2.0;
    let right: f64 = 3.0;
    let product: f64 = left * right;
    transition product == 6.0 { true -> 4 _ -> 5 }
}
"#;

fn application_inputs(root: &Path) -> PackageCompilationInputs {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in fs::read_dir(root).expect("application source files") {
            fs::set_permissions(
                entry.expect("source entry").path(),
                fs::Permissions::from_mode(0o444),
            )
            .expect("seal application source");
        }
        fs::set_permissions(root, fs::Permissions::from_mode(0o555))
            .expect("seal application source root");
    }
    PackageCompilationInputs::new(
        identity(71),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(identity(71), "selected-const", root.to_owned())
                .with_canonical_source_metadata()
                .expect("canonical application source"),
        ],
        Vec::new(),
    )
    .expect("one application package")
}

fn assert_length(checked: &compiler::CheckedCompilation, owner: &str, expected: usize) {
    let data = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == owner)
        .expect("array owner");
    let field = checked
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "bytes" => {
                Some(field)
            }
            _ => None,
        })
        .expect("bytes field");
    assert!(
        checked
            .type_reference_table
            .fixed_array_lengths()
            .any(|(handle, length)| handle == field.type_reference
                && *length == typed_trees::types::FixedArrayLength::Literal(expected)),
        "{owner}.bytes must retain literal length {expected}"
    );
}

#[test]
fn ordinary_length_needed_by_build_evaluates_beside_pending_float_length() {
    let tree = TempTree::new();
    let root = tree.package("selected-const");
    TempTree::write(
        root.join("main.omg"),
        &format!(
            "{FLOAT_LENGTH}\nmachine ordinary_length() -> u64 {{ 3 }}\ndata Pending {{ bytes: [u8; float_length()]; }}"
        ),
    );
    TempTree::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("selected-const");
    let values: [u8; ordinary_length()];
    transition values.len == 3 {
        true -> observed(builder)
        false -> wrong(builder)
    }
    state observed(builder: &mut Build) { builder.log.write_line("ordinary length: 3"); }
    state wrong(builder: &mut Build) { builder.log.write_line("wrong ordinary length"); }
}
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(application_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("ordinary Build inputs evaluate before unrelated selected lengths resume");
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("observed Build execution")
            .build_log(),
        b"ordinary length: 3\n"
    );
    assert_length(&checked, "Pending", 4);
}

#[test]
fn generated_source_preserves_pending_root_length_and_wire_plan_once() {
    let tree = TempTree::new();
    let root = tree.package("selected-const");
    TempTree::write(
        root.join("main.omg"),
        &format!(
            "{FLOAT_LENGTH}\ndata PendingRoot {{ bytes: [u8; float_length()]; }}\ndata RootPacket {{ #1 value: u8; }}"
        ),
    );
    // Seeded extensions currently reject new wire schemas and all ConstCall
    // array lengths. Keep the deferred length and wire owner in the base;
    // ordinary generated data still exercises the extension continuation.
    let generated = "pub data Generated { bytes: [u8; 4]; }\n";
    let literal = generated
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    TempTree::write(
        root.join("build.omg"),
        &format!(
            r#"
machine build(builder: &mut Build) {{
    builder.application("selected-const");
    let generated: BuildPath = builder.output.resolve("selected.generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "{literal}");
    let close: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}}
"#
        ),
    );
    let session = tree.0.join("session");
    fs::create_dir(&session).expect("create build session");
    let session = session.canonicalize().expect("canonical build session");
    let sponsor = checked_interpreter::FilesystemSponsor::new(&session).expect("session sponsor");
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(session.join("output").to_owned()),
        package_inputs: Some(application_inputs(&root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("root and generated selected evaluation retain their original plan intervals");
    assert_eq!(
        checked
            .package_generated_source_bundle()
            .expect("generated bundle")
            .sources()
            .len(),
        1
    );
    assert_length(&checked, "PendingRoot", 4);
    assert_length(&checked, "Generated", 4);
    let schema = checked
        .typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == "RootPacket")
        .expect("authored wire schema");
    assert_eq!(
        checked
            .typed
            .wire_schema_plans
            .iter()
            .filter(|plan| plan.schema == schema.symbol)
            .count(),
        1,
        "RootPacket must publish exactly one plan across the evaluation continuations"
    );
    assert_eq!(
        checked.typed.wire_schema_plan(schema.symbol),
        Some([typed_trees::wire::WirePlacement::Varint { tag: 1 }].as_slice())
    );
}

#[test]
fn build_demand_for_pending_selected_length_reports_the_dependency() {
    let tree = TempTree::new();
    let root = tree.package("selected-const");
    TempTree::write(root.join("main.omg"), FLOAT_LENGTH);
    TempTree::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("selected-const");
    let values: [u8; float_length()];
    transition values.len == 4 {
        true -> observed(builder)
        false -> wrong(builder)
    }
    state observed(builder: &mut Build) { builder.log.write_line("selected length available"); }
    state wrong(builder: &mut Build) { builder.log.write_line("wrong selected length"); }
}
"#,
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(application_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect_err("Build cannot default an unresolved selected array to Unit");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("float_length")
                && diagnostic.message.contains("unresolved")),
        "the error must identify the unresolved evaluation dependency: {diagnostics:?}"
    );
}

#[test]
fn selected_lengths_retain_machine_parameter_result_and_local_slots() {
    let tree = TempTree::new();
    let root = tree.package("selected-const");
    TempTree::write(
        root.join("main.omg"),
        &format!(
            "{FLOAT_LENGTH}\nmachine retain(values: [u8; float_length()]) -> [u8; float_length()] {{\n        let copy: [u8; float_length()] = values; copy }}"
        ),
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(71),
        vec![PackageSourceBinding::new(
            identity(71),
            "selected-const",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("one package");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("selected lengths compose in ordinary receiving type positions");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retain")
        .expect("retaining machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("machine entry");
    let parameter = checked
        .state_parameters(state)
        .first()
        .expect("array parameter");
    let local = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "copy" =>
            {
                Some(local)
            }
            _ => None,
        })
        .expect("array local");
    for (position, reference) in [
        ("parameter", parameter.type_reference),
        ("result", state.return_type),
        ("local", local.type_reference),
    ] {
        assert!(
            matches!(
                checked.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(4),
                    ..
                }
            ),
            "the actual {position} type must retain literal length 4"
        );
    }
}

#[test]
fn selected_lengths_retain_trait_requirement_parameter_and_result_slots() {
    let tree = TempTree::new();
    let root = tree.package("selected-const");
    TempTree::write(
        root.join("main.omg"),
        &format!(
            "{FLOAT_LENGTH}\ntrait Reader {{ machine read(value: [u8; float_length()]) -> [u8; float_length()]; }}"
        ),
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(71),
        vec![PackageSourceBinding::new(
            identity(71),
            "selected-const",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("one package");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("selected lengths preserve the exact ordinary requirement slots");
    let definition = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Reader")
        .expect("Reader trait");
    let requirement = checked
        .trait_machine_signatures(definition)
        .first()
        .expect("read requirement");
    let parameter = checked
        .state_signature_parameters(requirement)
        .first()
        .expect("read parameter");
    for (position, reference) in [
        ("requirement parameter", parameter.type_reference),
        ("requirement result", requirement.return_type),
    ] {
        assert!(
            matches!(
                checked.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(4),
                    ..
                }
            ),
            "the actual {position} type must retain literal length 4"
        );
    }
}
