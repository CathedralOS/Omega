use super::{
    Project, bound_build_output_session, package_inputs, set_canonical_source_tree_permissions,
    sponsored_build_session, write_interleaved_serialized_replay_project,
    write_serialized_replay_project,
};
use checked_interpreter::FilesystemSponsor;
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;

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
    assert_eq!(usage.replay_filesystem_operation_attempts, 6);
    assert_eq!(usage.fuel_units, usage.replay_fuel_units);
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
    assert_eq!(
        observation.realized(),
        build_evaluation::BuildObservationClass::Receipted,
    );
    assert!(observation.filesystem_replay_verdict().is_complete());
    assert!(observation.canonical_source_metadata_identity().is_some());
    let [handoff] = observation.included_source_handoffs() else {
        panic!("one exact generated-source handoff must remain in observation evidence")
    };
    assert_eq!(handoff.relative_path(), b"generated.omg");
    assert_eq!(handoff.filesystem_attempt_ordinal(), 6);
    let staged = observation
        .staged_output_tree()
        .expect("complete replay retains the staged output tree");
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
fn serialized_replay_record_reproduces_the_full_admitted_activation() {
    let profile = target::TargetProfile::WindowsX64;
    let project = Project::new("serialized-replay");
    write_serialized_replay_project(&project);
    let (session, sponsor, build_dir) = sponsored_build_session("serialized-replay");
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = package_inputs(&project.root);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("admitted build activation executes and appends generated source");
    assert!(
        checked
            .typed
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == "ReplayGenerated"),
        "generated source joins the final checked program"
    );

    // The activation's review-only record is canonical bytes: serialize it,
    // recover it, and replay the complete activation with no staged output or
    // sponsor authority. The replayed run must reach the identical result.
    let summary = checked
        .build_observation_summary()
        .expect("admitted activation retains observation custody");
    assert!(
        summary.filesystem_replay_verdict().is_complete(),
        "primary activation must complete its internal verifier replay"
    );
    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("capture the verified replay record")
        .expect("a complete receipted activation issues a replay record");
    let canonical = record.canonical_bytes().to_vec();
    let recovered =
        build_evaluation::recover_review_only_build_filesystem_replay_record(&canonical, limits)
            .expect("serialized replay record recovers");

    let replayed = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        replay_record: Some(recovered.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("serialized replay reproduces the full activation without host output staging");
    assert!(
        replayed
            .typed
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == "ReplayGenerated"),
        "replayed generated source joins the final checked program"
    );
    assert_eq!(
        replayed
            .package_generated_source_bundle()
            .expect("replayed activation retains its generated-source bundle")
            .sources(),
        checked
            .package_generated_source_bundle()
            .expect("primary activation retains its generated-source bundle")
            .sources(),
    );
    assert_eq!(
        replayed.source_consumption_commitment(),
        checked.source_consumption_commitment(),
        "replayed activation consumes the identical authored and generated source"
    );
    assert!(
        replayed
            .build_observation_summary()
            .expect("replayed activation retains observation custody")
            .filesystem_replay_verdict()
            .is_complete(),
        "the replayed activation is itself a complete receipted run"
    );

    // Source drift after the record was issued must reject before the admitted
    // build replays under a stale authority snapshot.
    set_canonical_source_tree_permissions(&project.root, false);
    project.write("input.txt", "drift\n");
    set_canonical_source_tree_permissions(&project.root, true);
    let drifted = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&project.root)),
        replay_record: Some(recovered),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect_err("a stale replay record must reject drifted source custody");
    assert!(
        drifted.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match the current canonical Source metadata identity")),
        "unexpected drift diagnostics: {drifted:#?}"
    );
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn serialized_replay_preserves_interleaved_output_descriptor_lifetimes() {
    use build_evaluation::BuildFilesystemLogicalHandleInputResolution::Resolved;

    let profile = target::TargetProfile::WindowsX64;
    let project = Project::new("interleaved-serialized-replay");
    write_interleaved_serialized_replay_project(&project);
    let (session, sponsor, build_dir) = sponsored_build_session("interleaved-serialized-replay");
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = package_inputs(&project.root);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir.clone()),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("interleaved admitted Output files execute and append generated source");
    let expected_generated = b"data ReplayGenerated { base: Main; }\n\n";
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
        [1, 1, 5, 5, 5, 5, 5, 5, 8, 8],
        "retain authored operation order rather than grouping by descriptor"
    );
    let generated_descriptor = attempts[0].logical_handle_output().unwrap().identity();
    let artifact_descriptor = attempts[1].logical_handle_output().unwrap().identity();
    assert_ne!(generated_descriptor, artifact_descriptor);
    for (attempt, descriptor) in attempts[2..].iter().zip([
        generated_descriptor,
        artifact_descriptor,
        generated_descriptor,
        artifact_descriptor,
        generated_descriptor,
        artifact_descriptor,
        artifact_descriptor,
        generated_descriptor,
    ]) {
        let [input] = attempt.logical_handle_inputs() else {
            panic!("one descriptor operand per write or close")
        };
        assert_eq!(input.resolution(), Resolved(descriptor));
    }
    assert_eq!(
        attempts[8].retired_logical_handles(),
        &[artifact_descriptor]
    );
    assert_eq!(
        attempts[9].retired_logical_handles(),
        &[generated_descriptor]
    );
    let [handoff] = summary.included_source_handoffs() else {
        panic!("only the explicitly included generated file is source")
    };
    assert_eq!(handoff.relative_path(), b"generated.omg");
    assert_eq!(handoff.filesystem_attempt_ordinal(), 10);
    assert!(
        summary.filesystem_replay_verdict().is_complete(),
        "interleaved descriptor lifetimes must establish Complete replay"
    );
    let staged = summary
        .staged_output_tree()
        .expect("complete output custody");
    assert_eq!(staged.entry_count(), 2);
    assert_eq!(staged.file_bytes(), expected_generated.len() as u64 + 5);
    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("capture interleaved replay record")
        .expect("complete replay issues a record");
    let recovered = build_evaluation::recover_review_only_build_filesystem_replay_record(
        record.canonical_bytes(),
        limits,
    )
    .expect("serialized interleaved record recovers");

    // Remove the live output before replay; neither a sponsor nor a build directory
    // is supplied to the provider-free activation.
    std::fs::remove_dir_all(&session).expect("remove primary activation's host output");
    let replayed = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        replay_record: Some(recovered),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("serialized replay reconstructs interleaved outputs without a provider");
    assert!(
        replayed
            .typed
            .data_definitions()
            .iter()
            .any(|definition| { definition.name.as_str() == "ReplayGenerated" })
    );
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed custody");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(replayed_summary.filesystem_operation_attempts(), attempts);
    assert_eq!(
        replayed_summary.included_source_handoffs(),
        summary.included_source_handoffs()
    );
    assert_eq!(replayed_summary.staged_output_tree(), Some(staged));
    assert_eq!(
        replayed
            .package_generated_source_bundle()
            .unwrap()
            .sources(),
        checked.package_generated_source_bundle().unwrap().sources(),
    );
    assert_eq!(
        replayed.source_consumption_commitment(),
        checked.source_consumption_commitment()
    );
}

#[test]
fn serialized_replay_record_rejects_activation_drift() {
    let profile = target::TargetProfile::WindowsX64;
    let project = Project::new("serialized-activation-drift");
    write_serialized_replay_project(&project);
    let (session, sponsor, build_dir) = sponsored_build_session("serialized-activation-drift");
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = package_inputs(&project.root);
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("admitted build activation executes and appends generated source");
    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(
        checked
            .build_observation_summary()
            .expect("admitted activation retains observation custody"),
        limits,
    )
    .expect("capture the verified replay record")
    .expect("a complete receipted activation issues a replay record");
    let recovered = build_evaluation::recover_review_only_build_filesystem_replay_record(
        record.canonical_bytes(),
        limits,
    )
    .expect("serialized replay record recovers");

    // The record's bound target is an observable build input: replaying the
    // same evidence under another selected target must reject rather than
    // substitute a stale activation.
    let drifted_target = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        replay_record: Some(recovered.clone()),
        ..CheckedCompileRequest::new(
            &project.main(),
            Some(target::TargetProfile::LinuxX64.target_name()),
        )
    })
    .expect_err("replay evidence bound to another target must reject");
    assert!(
        drifted_target
            .iter()
            .any(|diagnostic| diagnostic.message.contains("selected target profile")),
        "unexpected target-drift diagnostics: {drifted_target:#?}"
    );

    // Identical source bytes under a different root package occurrence are a
    // different activation even though the canonical metadata commitment is
    // unchanged.
    let foreign = PackageKeyIdentity::from_digest([98; 32]).expect("foreign root identity");
    let foreign_inputs = PackageCompilationInputs::new_package(
        foreign,
        vec![
            PackageSourceBinding::new(foreign, "build-facet", project.root.clone())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::new(),
    )
    .expect("foreign package occurrence input");
    let drifted_root = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(foreign_inputs),
        replay_record: Some(recovered.clone()),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect_err("replay evidence bound to another root package must reject");
    assert!(
        drifted_root
            .iter()
            .any(|diagnostic| diagnostic.message.contains("root package identity")),
        "unexpected root-drift diagnostics: {drifted_root:#?}"
    );

    // The authored declaration role is part of the activation: the same
    // sources under the application role must not replay package evidence.
    let application_inputs = PackageCompilationInputs::new(
        inputs.root(),
        package_compilation::BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(inputs.root(), "build-facet", project.root.clone())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::new(),
    )
    .expect("application-role package input");
    let drifted_role = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(application_inputs),
        replay_record: Some(recovered),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect_err("replay evidence bound to another declaration role must reject");
    assert!(
        drifted_role
            .iter()
            .any(|diagnostic| diagnostic.message.contains("root declaration role")),
        "unexpected role-drift diagnostics: {drifted_role:#?}"
    );
    let _ = std::fs::remove_dir_all(session);
}

/// A hosted profile other than the compiler host, so the drift below changes
/// only the admitted build execution profile.
fn foreign_build_execution_profile() -> target::TargetProfile {
    match target::TargetProfile::host() {
        target::TargetProfile::LinuxX64 => target::TargetProfile::LinuxArm64,
        _ => target::TargetProfile::LinuxX64,
    }
}

#[test]
fn serialized_replay_record_rejects_execution_profile_drift() {
    let profile = target::TargetProfile::WindowsX64;
    let project = Project::new("serialized-execution-profile-drift");
    write_serialized_replay_project(&project);
    let (session, sponsor, build_dir) =
        sponsored_build_session("serialized-execution-profile-drift");
    set_canonical_source_tree_permissions(&project.root, true);
    let inputs = package_inputs(&project.root);
    // A request naming no execution profile admits the compiler host; the
    // primary activation is captured under exactly that admitted profile.
    let checked = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(build_dir),
        package_inputs: Some(inputs.clone()),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("admitted build activation executes and appends generated source");
    let summary = checked
        .build_observation_summary()
        .expect("admitted activation retains observation custody");
    assert_eq!(
        summary.replay_activation().build_execution_profile(),
        Some(target::TargetProfile::host()),
        "the retained activation binds the admitted build execution profile"
    );
    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("capture the verified replay record")
        .expect("a complete receipted activation issues a replay record");
    let recovered = build_evaluation::recover_review_only_build_filesystem_replay_record(
        record.canonical_bytes(),
        limits,
    )
    .expect("serialized replay record recovers");
    assert_eq!(
        recovered.replay_activation().build_execution_profile(),
        Some(target::TargetProfile::host()),
        "the serialized record carries the build execution profile"
    );

    // Same root package, declaration role, and selected product target; only
    // the profile the build machine is admitted to execute under differs. The
    // evidence is stale for that activation and must not substitute.
    let foreign_execution_profile = foreign_build_execution_profile();
    let drifted_profile = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        replay_record: Some(recovered),
        build_execution_profile: Some(foreign_execution_profile),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect_err("replay evidence bound to another build execution profile must reject");
    let drift_message = drifted_profile
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("was captured for a different activation")
        })
        .map(|diagnostic| diagnostic.message.as_str())
        .unwrap_or_else(|| panic!("unexpected profile-drift diagnostics: {drifted_profile:#?}"));
    assert!(
        drift_message.contains("build execution profile"),
        "the rejection names the drifted execution profile: {drift_message}"
    );
    for equal_axis in [
        "root package identity",
        "root declaration role",
        "selected target profile",
    ] {
        assert!(
            !drift_message.contains(equal_axis),
            "{equal_axis} did not drift: {drift_message}"
        );
    }
    let _ = std::fs::remove_dir_all(session);
}
