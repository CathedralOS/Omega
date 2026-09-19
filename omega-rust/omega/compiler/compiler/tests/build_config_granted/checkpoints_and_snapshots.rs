use super::{
    Project, bound_build_output_session, package_inputs, set_canonical_source_tree_permissions,
    sponsored_build_session, write_interleaved_build_project,
    write_mixed_interleaved_build_project,
};
use checked_interpreter::FilesystemSponsor;
use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn admitted_build_checkpoint_retains_configuration_and_execution_evidence() {
    let profile = target::TargetProfile::WindowsX64;
    let project = Project::new("generated-source");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write("input.txt", "input\n");
    project.write(
        "build.omg",
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("build-facet-generated-source");
    builder.subsystem = Subsystem::Gui;
    let input: BuildPath = builder.source.resolve("input.txt");
    let input_descriptor: i32 = builder.source.open(input, 0);
    let mut input_bytes: [u8; 6];
    let input_count: i64 = builder.source.read(input_descriptor, &mut input_bytes, 6);
    let input_close: i32 = builder.source.close(input_descriptor);

    let generated: BuildPath = builder.output.resolve("generated.omg");
    let output_descriptor: i32 = builder.output.create(generated, 438);
    let output_count: i64 = builder.output.write(
        output_descriptor,
        "data Generated {{ base: Main; }}\npub data MachineConfig {{ count: u8; enabled: bool; }}\npub data ConfigIndexed<const C: MachineConfig> {{ marker: u8; }}\npub trait GeneratedOperation {{ machine apply(value: u64) -> u64; }}\npub machine identity<'value, T>(value: &'value T) -> &'value T {{ value }}\npub machine bounded<T [copy]>(value: &T) {{}}\npub machine apply<machine Selected>(value: u64) -> u64 where machine Selected(value: u64) -> u64; {{ Selected(value) }}\npub machine apply_nominal<machine Selected>(value: u64) -> u64 where machine Selected satisfies GeneratedOperation::apply; {{ Selected(value) }}\npub machine width<const N: u64>(value: [u8; N]) {{}}\npub machine configured<const C: MachineConfig>(value: ConfigIndexed<C>) {{}}\n"
    );
    let output_close: i32 = builder.output.close(output_descriptor);
    builder.output.include_source(generated);
}}
"#,
        ),
    );

    let session =
        std::env::temp_dir().join(format!("omega-build-facet-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create build session");
    let session = std::fs::canonicalize(session).expect("canonicalize build session");
    let sponsor = FilesystemSponsor::new(&session).expect("create build sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind build output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare build output root");
    std::fs::create_dir(&build_dir).expect("create build output root");
    prepared_build_dir
        .commit()
        .expect("commit build output root");
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("compiler-owned Build facets should execute and publish generated source");
    set_canonical_source_tree_permissions(&project.root, false);

    let generated = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated source reaches final checked program");
    let main = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("authored source remains in final checked program");
    let identity = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .expect("generated generic machine reaches final checked program");
    let [identity_parameter] = checked.typed.machine_type_parameters(identity) else {
        panic!("generated identity machine retains one Type parameter")
    };
    assert_eq!(identity_parameter.name.as_str(), "T");
    assert_eq!(
        identity
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["value"]
    );
    let bounded = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "bounded")
        .expect("generated property-bounded machine reaches final checked program");
    let [bounded_parameter] = checked.typed.machine_type_parameters(bounded) else {
        panic!("generated bounded machine retains one Type parameter")
    };
    assert_eq!(
        bounded_parameter.bounds,
        typed_trees::data::DataProperties {
            carry: None,
            multiplicity: language_semantics::Multiplicity::Unrestricted,
        },
    );
    let apply = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("generated static-machine generic reaches final checked program");
    let [apply_parameter] = checked.typed.machine_type_parameters(apply) else {
        panic!("generated apply machine retains one static-machine parameter")
    };
    let typed_trees::data::TypeParameterKind::Machine { contract } = &apply_parameter.kind else {
        panic!("apply.Selected remains a static-machine binder")
    };
    let typed_trees::data::MachineParameterContract::Structural(signature) = contract else {
        panic!("apply.Selected retains its structural signature")
    };
    assert_eq!(signature.symbol, apply_parameter.symbol);
    assert_eq!(checked.typed.state_signature_parameters(signature).len(), 1);
    let generated_operation = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "GeneratedOperation")
        .expect("generated extension retains the nominal contract trait");
    let generated_requirement = checked
        .typed
        .trait_machine_signatures(generated_operation)
        .first()
        .expect("generated extension retains the nominal contract requirement");
    let apply_nominal = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply_nominal")
        .expect("generated nominal static-machine generic reaches final checked program");
    let [nominal_parameter] = checked.typed.machine_type_parameters(apply_nominal) else {
        panic!("generated apply_nominal retains one static-machine parameter")
    };
    let typed_trees::data::TypeParameterKind::Machine {
        contract:
            typed_trees::data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            },
    } = &nominal_parameter.kind
    else {
        panic!("apply_nominal.Selected retains its exact nominal contract")
    };
    assert_eq!(*trait_definition, generated_operation.symbol);
    assert_eq!(*requirement, generated_requirement.symbol);
    assert!(
        checked
            .typed
            .expression_table
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(
                    expression,
                    typed_trees::expression::ExpressionNode::Call(call)
                        if call.target_symbol == nominal_parameter.symbol
                )
            })
    );
    let width = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "width")
        .expect("generated scalar-const machine reaches final checked program");
    let [width_parameter] = checked.typed.machine_type_parameters(width) else {
        panic!("generated width machine retains one const parameter")
    };
    assert_eq!(width_parameter.name.as_str(), "N");
    assert!(matches!(
        width_parameter.kind,
        typed_trees::data::TypeParameterKind::Const { .. }
    ));
    let configured = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "configured")
        .expect("generated structured-const machine reaches final checked program");
    let [config_parameter] = checked.typed.machine_type_parameters(configured) else {
        panic!("generated configured machine retains one const parameter")
    };
    let machine_config = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "MachineConfig")
        .expect("generated structured const carrier reaches final checked program");
    let config_indexed = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ConfigIndexed")
        .expect("generated structured const template reaches final checked program");
    let typed_trees::data::TypeParameterKind::Const { type_reference } = config_parameter.kind
    else {
        panic!("configured.C remains a const binder")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(type_reference),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == machine_config.symbol && name.as_str() == "MachineConfig"
    ));
    let [configured_state] = checked.typed.machine_states(configured) else {
        panic!("configured retains one entry state")
    };
    let [configured_value] = checked.typed.state_parameters(configured_state) else {
        panic!("configured retains one value parameter")
    };
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        arguments,
        ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(configured_value.type_reference)
    else {
        panic!("configured value retains the structured const application")
    };
    assert_eq!(*base_symbol, config_indexed.symbol);
    assert_eq!(base_name.as_str(), "ConfigIndexed");
    let [config_argument] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("configured value retains one structured const argument")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(*config_argument),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == config_parameter.symbol && name.as_str() == "C"
    ));
    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("selected build machine remains in the final checked program");
    assert_eq!(checked.selected_build_machine_symbol(), Some(build.symbol));
    let build_identity = checked
        .typed
        .normalized_machine_overload_identity(build)
        .expect("selected build machine retains a normalized callable identity")
        .identity();
    assert_eq!(
        checked.selected_build_machine_identity(),
        Some(build_identity.as_str()),
    );
    assert_eq!(checked.selected_target_profile(), Some(profile));
    assert_eq!(
        checked.selected_native_target(),
        Some(target::NativeTarget::windows_x64()),
    );
    assert_eq!(checked.subsystem(), 2, "Gui is the non-default subsystem");
    let [typed_trees::data::DataMember::Field(base)] = checked.typed.data_members(generated) else {
        panic!("Generated has one nominal field")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .typed
        .type_reference_table
        .type_reference(base.type_reference)
    else {
        panic!("Generated.base remains nominal")
    };
    assert_eq!(*symbol, main.symbol);
    let usage = checked
        .build_evaluation_usage()
        .expect("selected build execution retains deterministic usage");
    assert_eq!(usage.filesystem_operation_attempts, 6);
    assert!(usage.fuel_units > 0);
    assert_eq!(usage.sponsor_schema_version, None);
    assert_eq!(usage.session_filesystem_attempt_ceiling, None);

    let observation = checked
        .build_observation_summary()
        .expect("facet execution retains observations");
    assert_eq!(
        observation.filesystem_operation_attempts().len(),
        6,
        "open/read/close and create/write/close execute once; handoff is separate custody"
    );
    assert!(observation.filesystem_host_observed());
    assert!(observation.canonical_source_metadata_identity().is_some());
    let [handoff] = observation.included_source_handoffs() else {
        panic!("one exact generated-source handoff must remain in observation evidence")
    };
    assert_eq!(handoff.relative_path(), b"generated.omg");
    assert_eq!(handoff.filesystem_attempt_ordinal(), 6);
    let staged = observation
        .staged_output_tree()
        .expect("execution retains the staged output tree");
    assert_eq!(staged.entry_count(), 1);
    let expected_generated = b"data Generated { base: Main; }\npub data MachineConfig { count: u8; enabled: bool; }\npub data ConfigIndexed<const C: MachineConfig> { marker: u8; }\npub trait GeneratedOperation { machine apply(value: u64) -> u64; }\npub machine identity<'value, T>(value: &'value T) -> &'value T { value }\npub machine bounded<T [copy]>(value: &T) {}\npub machine apply<machine Selected>(value: u64) -> u64 where machine Selected(value: u64) -> u64; { Selected(value) }\npub machine apply_nominal<machine Selected>(value: u64) -> u64 where machine Selected satisfies GeneratedOperation::apply; { Selected(value) }\npub machine width<const N: u64>(value: [u8; N]) {}\npub machine configured<const C: MachineConfig>(value: ConfigIndexed<C>) {}\n";
    assert_eq!(staged.file_bytes(), expected_generated.len() as u64);

    assert_eq!(
        checked.source_file_count(),
        4,
        "authored, generated, and compiler-injected source custody remains exact",
    );
    assert_ne!(
        checked.base_source_consumption_commitment(),
        checked.source_consumption_commitment(),
        "generated source must advance the final source commitment",
    );
    let generated_bundle = checked
        .package_generated_source_bundle()
        .expect("checked package retains its generated-source custody");
    assert_eq!(generated_bundle.target(), profile);
    assert_eq!(
        generated_bundle.source_consumption_commitment(),
        checked
            .source_consumption_commitment()
            .expect("package-aware source commitment"),
    );
    let [generated_source] = generated_bundle.sources() else {
        panic!("one generated source must remain in the checked bundle")
    };
    assert_eq!(generated_source.relative_path(), b"generated.omg");
    assert_eq!(generated_source.bytes(), expected_generated);
    checked
        .verify_current_source_consumption()
        .expect("generated bytes remain tied to staged-output custody");
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn build_snapshot_binds_captured_source_reads_and_linear_output_completion() {
    let profile = target::TargetProfile::host();
    let project = Project::new("snapshot-outputs");
    let main_source = "data Main { value: u8; }\n";
    project.write("main.omg", main_source);
    let templates = project.root.join("templates");
    std::fs::create_dir(&templates).expect("create template directory");
    let banner = b"HELLO {{name}}\n";
    std::fs::write(templates.join("banner.tmpl"), banner).expect("write template");
    let build_source = r#"machine build(builder: &mut Build) {
    builder.application("build-snapshot-outputs");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 7];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 7);
    let source_close: i32 = builder.source.close(template_descriptor);

    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let output_descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(output_descriptor, "banner read\n");
    let output_close: i32 = builder.output.close(output_descriptor);
}
"#;
    project.write("build.omg", build_source);

    let (session, sponsor, build_dir) = bound_build_output_session("granted");
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(build_evaluation::BuildSnapshotRequest::new([
            b"artifact.txt".to_vec(),
        ])),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("captured snapshot reads and a completed required output publish the build");
    set_canonical_source_tree_permissions(&project.root, false);

    let observation = checked
        .build_observation_summary()
        .expect("snapshot execution retains observations");
    let inventory = observation
        .captured_source_inventory()
        .expect("the snapshot occurrence retains captured inventory evidence");
    assert_eq!(
        inventory.entry_count(),
        5,
        "root, both sources, the template directory, and the template"
    );
    assert_eq!(
        inventory.file_bytes(),
        (main_source.len() + build_source.len() + banner.len()) as u64,
    );
    assert!(observation.canonical_source_metadata_identity().is_some());
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let artifact = staged
        .sealed_entry(b"artifact.txt")
        .expect("the required output is discoverable in sealed custody");
    let build_output::BuildStagedOutputEntryKind::File { bytes, executable } = artifact.kind()
    else {
        panic!("a required output completes only as a sealed regular file")
    };
    assert_eq!(bytes, b"banner read\n");
    assert!(!executable);
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn build_snapshot_rejects_an_omitted_required_output() {
    let profile = target::TargetProfile::host();
    let project = Project::new("snapshot-omitted");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("build-snapshot-omitted");
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let output_descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(output_descriptor, "partial\n");
    let output_close: i32 = builder.output.close(output_descriptor);
}
"#,
    );

    let (session, sponsor, build_dir) = bound_build_output_session("omitted");
    set_canonical_source_tree_permissions(&project.root, true);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.to_owned()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(sponsor),
        build_snapshot: Some(build_evaluation::BuildSnapshotRequest::new([
            b"missing.txt".to_vec(),
        ])),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect_err("an invocation-required output the build never completed must reject");
    set_canonical_source_tree_permissions(&project.root, false);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("required build output `missing.txt`")
                && diagnostic.message.contains("omitted")
        }),
        "unexpected snapshot diagnostics: {diagnostics:#?}"
    );
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn filesystem_build_charges_the_sponsor_for_one_execution() {
    use checked_interpreter::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};

    let project = Project::new("single-execution-accounting");
    write_interleaved_build_project(&project);
    let (session, filesystem_sponsor, build_dir) =
        sponsored_build_session("single-execution-accounting");
    set_canonical_source_tree_permissions(&project.root, true);
    // Leave room for repeated execution so equality, not exhaustion, detects it.
    let evaluation_sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(
            1_000_000, 1_000_000, 1024, 64, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
        )
        .expect("nonzero execution ceilings"),
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.clone()),
        package_inputs: Some(package_inputs(&project.root)),
        filesystem_sponsor: Some(filesystem_sponsor),
        evaluation_sponsor: Some(evaluation_sponsor.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"))
    })
    .expect("the real filesystem build executes under its sponsor");
    let observation = checked
        .build_observation_summary()
        .expect("filesystem execution retains observations");
    let usage = checked
        .build_evaluation_usage()
        .expect("execution retains resource usage");
    let attempt_count = u64::try_from(observation.filesystem_operation_attempts().len())
        .expect("bounded attempt count");
    assert_eq!(attempt_count, 10);
    assert_eq!(usage.filesystem_operation_attempts, attempt_count);
    assert_eq!(
        evaluation_sponsor.consumed_filesystem_operation_attempts(),
        attempt_count,
        "the sponsor charges only the observed execution"
    );
    assert!(usage.fuel_units > 0);
    assert_eq!(
        evaluation_sponsor.consumed_fuel_units(),
        usage.fuel_units,
        "compilation must not silently execute the build a second time"
    );
    assert_eq!(
        std::fs::read(build_dir.join("artifact.txt")).unwrap(),
        b"abZcd"
    );
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn build_execution_preserves_interleaved_output_descriptor_lifetimes() {
    assert_interleaved_build_execution(false);
}

#[test]
fn build_execution_preserves_interleaved_source_and_output_descriptor_lifetimes() {
    assert_interleaved_build_execution(true);
}

fn assert_interleaved_build_execution(mixed_source: bool) {
    use build_evaluation::BuildFilesystemLogicalHandleInputResolution::Resolved;

    let profile = target::TargetProfile::WindowsX64;
    let label = if mixed_source {
        "mixed-interleaved-build-execution"
    } else {
        "interleaved-build-execution"
    };
    let project = Project::new(label);
    if mixed_source {
        write_mixed_interleaved_build_project(&project);
    } else {
        write_interleaved_build_project(&project);
    }
    let (session, sponsor, build_dir) = sponsored_build_session(label);
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = package_inputs(&project.root);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.clone()),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("interleaved admitted Output files execute and append generated source");
    let expected_generated = b"data BuildGenerated { base: Main; }\n\n";
    let expected_operations: &[u16] = if mixed_source {
        &[2, 1, 2, 1, 4, 5, 5, 4, 5, 5, 5, 5, 8, 8, 4, 8, 8]
    } else {
        &[1, 1, 5, 5, 5, 5, 5, 5, 8, 8]
    };
    assert_eq!(
        std::fs::read(build_dir.join("generated.omg")).unwrap(),
        expected_generated
    );
    assert_eq!(
        std::fs::read(build_dir.join("artifact.txt")).unwrap(),
        b"abZcd"
    );
    let summary = checked
        .build_observation_summary()
        .expect("observation custody");
    let attempts = summary.filesystem_operation_attempts();
    assert_eq!(
        attempts
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        expected_operations,
        "retain authored operation order rather than grouping by descriptor"
    );
    let (creation_count, descriptor_origins, handoff_ordinal): (usize, &[usize], u64) =
        if mixed_source {
            (4, &[0, 1, 3, 2, 1, 3, 1, 3, 3, 1, 0, 2, 0], 14)
        } else {
            (2, &[0, 1, 0, 1, 0, 1, 1, 0], 10)
        };
    let descriptors: Vec<_> = attempts[..creation_count]
        .iter()
        .map(|attempt| attempt.logical_handle_output().unwrap().identity())
        .collect();
    for (position, descriptor) in descriptors.iter().enumerate() {
        assert!(!descriptors[..position].contains(descriptor));
    }
    for (attempt, &origin) in attempts[creation_count..].iter().zip(descriptor_origins) {
        let descriptor = descriptors[origin];
        let [input] = attempt.logical_handle_inputs() else {
            panic!("one descriptor operand per read, write, or close")
        };
        assert_eq!(input.resolution(), Resolved(descriptor));
        if attempt.operation_tag() == 8 {
            assert_eq!(attempt.retired_logical_handles(), &[descriptor]);
        } else {
            assert!(attempt.retired_logical_handles().is_empty());
        }
    }
    let [handoff] = summary.included_source_handoffs() else {
        panic!("only the explicitly included generated file is source")
    };
    assert_eq!(handoff.relative_path(), b"generated.omg");
    assert_eq!(handoff.filesystem_attempt_ordinal(), handoff_ordinal);
    let staged = summary
        .staged_output_tree()
        .expect("complete output custody");
    assert_eq!(staged.entry_count(), 2);
    assert_eq!(staged.file_bytes(), expected_generated.len() as u64 + 5);
    set_canonical_source_tree_permissions(&project.root, false);
    let _ = std::fs::remove_dir_all(session);
}
