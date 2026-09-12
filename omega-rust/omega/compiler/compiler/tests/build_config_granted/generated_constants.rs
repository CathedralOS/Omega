//! Generated source consumes retained authored constants after admitted build execution.

use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn generated_bodies_use_retained_module_constants_after_build_execution() {
    let project = Project::new("generated-constants");
    project.write("main.omg", "use settings; data Main {}\n");
    project.write(
        "settings.omg",
        "module settings; pub const ANSWER: u8 = 42; pub const ROW: [u8; 2] = [7, 9];\n",
    );
    project.write("build.omg", r#"machine build(builder: &mut Build) {
        builder.package("generated-constants");
        let generated: BuildPath = builder.output.resolve("generated.omg");
        let descriptor: i32 = builder.output.create(generated, 438);
        let count: i64 = builder.output.write(descriptor,
            "use settings; pub machine generated() -> u8 { settings::ANSWER }\npub machine generated_row() -> [u8; 2] { settings::ROW }\n");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }"#);
    let session = Project::new("generated-constants-session");
    let session_root = std::fs::canonicalize(&session.root).unwrap();
    let sponsor = FilesystemSponsor::new(&session_root).unwrap();
    let build_dir = session_root.join("output");
    let bound = sponsor.bind_path(&build_dir).unwrap();
    let prepared = sponsor.prepare_create_directory(&bound).unwrap();
    std::fs::create_dir(&build_dir).unwrap();
    prepared.commit().unwrap();
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(
            &project.main(),
            Some(target::TargetProfile::host().target_name()),
        )
    })
    .expect("generated bodies reuse the exact admitted base constants");
    checked.verify_current_source_consumption().unwrap();
    let artifacts = ["generated", "generated_row"].map(|name| {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, name)
            .expect("generated constant use lowers without a runtime constant owner");
        (
            terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
            terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        )
    });
    drop(checked);
    let byte = |value| terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            8,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(value),
    };
    let execute = |(semantics, proof): &(Vec<u8>, Vec<u8>)| {
        terminal_interpreter::interpret_terminal_artifact(
            semantics,
            proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("independently decoded generated constant body executes")
    };
    assert_eq!(
        execute(&artifacts[0]),
        terminal_interpreter::TerminalExecutionResult::Scalar(byte(42))
    );
    let terminal_interpreter::TerminalExecutionResult::ScalarArray(row) = execute(&artifacts[1])
    else {
        panic!("retained array constant has actual payload");
    };
    assert_eq!(row.value.elements, [byte(7), byte(9)]);
}

#[test]
fn generated_record_arguments_rejoin_retained_declarations() {
    let project = Project::new("generated-record-arguments");
    project.write(
        "main.omg",
        "data Main {} data Point { value: u64; } data Other { value: u64; }\n",
    );
    project.write("build.omg", r#"machine build(builder: &mut Build) {
        builder.package("generated-record-arguments");
        let generated: BuildPath = builder.output.resolve("generated.omg");
        let descriptor: i32 = builder.output.create(generated, 438);
        let count: i64 = builder.output.write(descriptor,
            "data Cell<T> { value: T; }\ndata Generated { first: Cell<Point>; repeated: Cell<Point>; second: Cell<Other>; }\n");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }"#);
    let session = Project::new("generated-record-arguments-session");
    let session_root = std::fs::canonicalize(&session.root).unwrap();
    let sponsor = FilesystemSponsor::new(&session_root).unwrap();
    let build_dir = session_root.join("output");
    let bound = sponsor.bind_path(&build_dir).unwrap();
    let prepared = sponsor.prepare_create_directory(&bound).unwrap();
    std::fs::create_dir(&build_dir).unwrap();
    prepared.commit().unwrap();
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(
            &project.main(),
            Some(target::TargetProfile::host().target_name()),
        )
    })
    .expect("generated templates use exact retained argument declarations");
    checked.verify_current_source_consumption().unwrap();
    let instances = checked
        .data_definitions
        .iter()
        .filter_map(|(_, data)| data.generic_instance.map(|origin| (data.symbol, origin)))
        .collect::<Vec<_>>();
    assert_eq!(
        instances.len(),
        2,
        "repeated retained arguments share one instance"
    );
    assert_ne!(instances[0].0, instances[1].0);
    let mut arguments = instances
        .iter()
        .map(|(_, origin)| {
            let typed_trees::types::TypeReferenceNode::Generic { arguments, .. } =
                checked.tables.type_reference_table.type_reference(*origin)
            else {
                panic!("generated record retains its application");
            };
            let argument = checked
                .tables
                .type_reference_table
                .type_reference_handles(*arguments)[0];
            let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                checked.tables.type_reference_table.type_reference(argument)
            else {
                panic!("retained nominal argument");
            };
            checked.symbols.display_path(*symbol, "::")
        })
        .collect::<Vec<_>>();
    arguments.sort();
    assert_eq!(arguments, ["Other", "Point"]);
}
