use super::{TempTree, generated_source, identity};
use build_declarations::DependencyPurpose;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, ExplicitTargetSet,
    TargetCompileConfiguration, compile, compile_to_checked,
};
use package_compilation::{
    BuildDeclarationKind, ConsumedSourceUnitKind, PackageCompilationInputs,
    PackageDependencyBinding, PackageGeneratedSourceBundle, PackageSourceBinding,
    PackageSourceConsumptionCommitment, derive_source_consumption_commitment,
};

#[test]
fn generated_dependency_handoff_keeps_build_and_product_occurrences_distinct() {
    let tree = TempTree::new();
    let producer = tree.package("dual-producer");
    let consumer = tree.package("dual-consumer");
    TempTree::write(producer.join("main.omg"), "// Generated API only.\n");
    TempTree::write(
        producer.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("dual-producer");
    transition builder.target {
        TargetProfile::WindowsX86_64 -> product(builder)
        _ -> helper(builder)
    }
    state product(builder: &mut Build) {
        let generated: BuildPath = builder.output.resolve("generated_api.omg");
        let descriptor: i32 = builder.output.create(generated, 438);
        let count: i64 = builder.output.write(descriptor, "pub machine generated_value() -> u64 { 17 }\n");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }
    state helper(builder: &mut Build) {
        let generated: BuildPath = builder.output.resolve("generated_api.omg");
        let descriptor: i32 = builder.output.create(generated, 438);
        let count: i64 = builder.output.write(descriptor, "pub machine generated_value() -> bool { true }\n");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }
}
"#,
    );
    TempTree::write(
        consumer.join("build.omg"),
        r#"use dependency::generated_api;
machine build(builder: &mut Build) {
    builder.package("dual-consumer");
    builder.build_depend_as("dependency", Source::Path { location: "../dual-producer" });
    builder.depend_as("dependency", Source::Path { location: "../dual-producer" });
    let generated: BuildPath = builder.output.resolve("consumer_generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    transition generated_value() {
        true -> correct(builder, generated, descriptor)
        false -> incorrect(builder, generated, descriptor)
    }
    state correct(builder: &mut Build, generated: BuildPath, descriptor: i32) {
        let count: i64 = builder.output.write(descriptor, "pub machine generated_by_helper() -> u64 { 29 }");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }
    state incorrect(builder: &mut Build, generated: BuildPath, descriptor: i32) {
        let count: i64 = builder.output.write(descriptor, "pub machine generated_by_helper() -> u64 { 0 }");
        let closed: i32 = builder.output.close(descriptor);
        builder.output.include_source(generated);
    }
}
"#,
    );
    TempTree::write(
        consumer.join("main.omg"),
        "use dependency::generated_api;\npub machine consume() -> u64 { generated_value() }\n",
    );
    let session_root = tree.0.join("dual-build");
    std::fs::create_dir(&session_root).unwrap();
    let session_root = std::fs::canonicalize(session_root).unwrap();
    let sponsor = checked_interpreter::FilesystemSponsor::new(&session_root).unwrap();
    let mut bundles = Vec::new();
    for (purpose, target, directory) in [
        (DependencyPurpose::Build, "linux_x86_64", "helper"),
        (DependencyPurpose::Product, "windows_x86_64", "product"),
    ] {
        let inputs = PackageCompilationInputs::new_package(
            identity(94),
            vec![
                PackageSourceBinding::new(identity(94), "dual-producer", producer.clone())
                    .with_canonical_source_metadata()
                    .unwrap(),
            ],
            Vec::new(),
        )
        .unwrap()
        .with_compilation_purpose(purpose)
        .unwrap();
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            build_execution_profile: Some(target::TargetProfile::LinuxX64),
            build_dir: Some(session_root.join(directory)),
            filesystem_sponsor: Some(sponsor.clone()),
            ..CheckedCompileRequest::new(&producer.join("main.omg"), Some(target))
        })
        .expect("one acquired producer checks independently for each purpose and target");
        bundles.push(checked.package_generated_source_bundle().unwrap());
    }
    assert_ne!(
        bundles[0].sources()[0].bytes(),
        bundles[1].sources()[0].bytes()
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(93),
        vec![
            PackageSourceBinding::new(identity(93), "dual-consumer", consumer.clone())
                .with_canonical_source_metadata()
                .unwrap(),
            PackageSourceBinding::new(identity(94), "dual-producer", producer.clone()),
        ],
        [DependencyPurpose::Build, DependencyPurpose::Product]
            .into_iter()
            .map(|purpose| {
                PackageDependencyBinding::for_purpose(
                    identity(93),
                    "dependency",
                    identity(94),
                    purpose,
                )
            })
            .collect(),
    )
    .unwrap()
    .with_complete_dependency_generated_sources(bundles)
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        build_execution_profile: Some(target::TargetProfile::LinuxX64),
        build_dir: Some(session_root.join("consumer")),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&consumer.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("host helper bool and product u64 resolve from different generated instances");
    let output = checked.package_generated_source_bundle().unwrap();
    assert_eq!(output.sources().len(), 1);
    assert_eq!(
        output.sources()[0].bytes(),
        b"pub machine generated_by_helper() -> u64 { 29 }"
    );
    assert!(!producer.join("generated_api.omg").exists());
    checked.verify_current_source_consumption().unwrap();
}

#[test]
fn generated_dependency_handoff_requires_both_purposes_even_for_the_same_target() {
    let tree = TempTree::new();
    let root = tree.package("purpose-consumer");
    let dependency = tree.package("purpose-producer");
    let inputs = PackageCompilationInputs::new_package(
        identity(95),
        vec![
            PackageSourceBinding::new(identity(95), "purpose-consumer", root),
            PackageSourceBinding::new(identity(96), "purpose-producer", dependency),
        ],
        [DependencyPurpose::Product, DependencyPurpose::Build]
            .into_iter()
            .map(|purpose| {
                PackageDependencyBinding::for_purpose(
                    identity(95),
                    "dependency",
                    identity(96),
                    purpose,
                )
            })
            .collect(),
    )
    .unwrap();
    let bundle = |purpose| {
        PackageGeneratedSourceBundle::from_checked(
            identity(96),
            purpose,
            target::TargetProfile::LinuxX64,
            Some(target::TargetProfile::LinuxX64),
            inputs.dependency_closure_for(identity(96)),
            PackageSourceConsumptionCommitment::for_test([96; 32]),
            vec![generated_source(
                b"generated_api.omg",
                b"pub machine generated_value() -> u64 { 17 }\n",
            )],
        )
    };
    for purpose in [DependencyPurpose::Product, DependencyPurpose::Build] {
        assert!(
            inputs
                .clone()
                .with_complete_dependency_generated_sources(vec![bundle(purpose)])
                .is_err(),
            "one purpose cannot satisfy the other occurrence even with equal target and bytes"
        );
        assert!(
            inputs
                .clone()
                .with_complete_dependency_generated_sources(vec![bundle(purpose), bundle(purpose)])
                .is_err(),
            "duplicate purpose cannot replace a missing occurrence"
        );
    }
    inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![
            bundle(DependencyPurpose::Product),
            bundle(DependencyPurpose::Build),
        ])
        .expect("both exact occurrences satisfy completeness even when bytes and targets match");
}

#[test]
fn build_package_target_must_match_its_admitted_execution_profile() {
    let tree = TempTree::new();
    let root = tree.package("build-context");
    TempTree::write(root.join("main.omg"), "// Build helper.\n");
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"build-context\"); }\n",
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(97),
        vec![PackageSourceBinding::new(
            identity(97),
            "build-context",
            root.clone(),
        )],
        Vec::new(),
    )
    .unwrap()
    .with_compilation_purpose(DependencyPurpose::Build)
    .unwrap();
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        build_execution_profile: Some(target::TargetProfile::LinuxX64),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("windows_x86_64"))
    })
    .err()
    .expect("a build helper cannot be compiled for a different product profile");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("target to equal the admitted build execution profile")),
        "{diagnostics:#?}"
    );
}

#[test]
fn generated_dependency_handoff_rejects_a_different_build_execution_profile() {
    let tree = TempTree::new();
    let producer = tree.package("profile-producer");
    let consumer = tree.package("profile-consumer");
    TempTree::write(producer.join("main.omg"), "// Generated API only.\n");
    TempTree::write(
        producer.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("profile-producer");
    let generated: BuildPath = builder.output.resolve("generated_api.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "pub machine generated_value() -> u64 { 17 }\n");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}
"#,
    );
    TempTree::write(
        consumer.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.package(\"profile-consumer\");\n    builder.depend_as(\"dependency\", Source::Path { location: \"../profile-producer\" });\n}\n",
    );
    TempTree::write(
        consumer.join("main.omg"),
        "use dependency::generated_api;\npub machine consume() -> u64 { generated_value() }\n",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in ["build.omg", "main.omg"] {
            std::fs::set_permissions(producer.join(file), std::fs::Permissions::from_mode(0o444))
                .unwrap();
        }
        std::fs::set_permissions(&producer, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    let producer_inputs = PackageCompilationInputs::new_package(
        identity(92),
        vec![
            PackageSourceBinding::new(identity(92), "profile-producer", producer.clone())
                .with_canonical_source_metadata()
                .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap();
    let session_root = tree.0.join("profile-build");
    std::fs::create_dir(&session_root).unwrap();
    let session_root = std::fs::canonicalize(session_root).unwrap();
    let sponsor = checked_interpreter::FilesystemSponsor::new(&session_root).unwrap();
    let checked_producer = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(producer_inputs),
        build_execution_profile: Some(target::TargetProfile::LinuxX64),
        build_dir: Some(session_root.join("producer")),
        filesystem_sponsor: Some(sponsor),
        ..CheckedCompileRequest::new(&producer.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("producer checks and retains its generated API");
    let bundle = checked_producer.package_generated_source_bundle().unwrap();
    assert_eq!(bundle.sources().len(), 1);
    assert_eq!(
        bundle.build_execution_profile(),
        Some(target::TargetProfile::LinuxX64)
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(91),
        vec![
            PackageSourceBinding::new(identity(91), "profile-consumer", consumer.clone()),
            PackageSourceBinding::new(identity(92), "profile-producer", producer.clone()),
        ],
        vec![PackageDependencyBinding::new(
            identity(91),
            "dependency",
            identity(92),
        )],
    )
    .unwrap()
    .with_complete_dependency_generated_sources(vec![bundle])
    .unwrap();
    let mut prepared: Option<compiler::PreparedCheckedSource> = None;
    for execution_profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::WindowsX64,
    ] {
        let retained = prepared.take();
        let request = CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            build_execution_profile: Some(execution_profile),
            prepared_source_output: Some(&mut prepared),
            ..CheckedCompileRequest::new(&consumer.join("main.omg"), Some("linux_x86_64"))
        };
        let result = match retained {
            Some(source) => source.compile_to_checked(request),
            None => compile_to_checked(request),
        };
        if execution_profile == target::TargetProfile::LinuxX64 {
            result.expect("matching build execution profile consumes retained generated source");
            assert!(
                prepared.is_some(),
                "retain the source frontier for the changed-profile child"
            );
        } else {
            let diagnostics = match result {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!(
                    "same-target generated source cannot substitute another build execution profile"
                ),
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("generated-source")
                        && diagnostic.message.contains("execution profile")),
                "{diagnostics:#?}"
            );
        }
    }
    assert!(!producer.join("generated_api.omg").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&producer, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn package_aware_import_cannot_mount_bundled_standard_library() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        "use omega::language::std::console;\n",
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("package-aware compilation must not mount bundled std");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("omega::language::std::console")
                && diagnostic.message.contains("ordinary package dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn package_aware_source_can_use_the_public_core_fixed_vec_carrier() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"use omega::language::core::fixed_vec;
data Readings {
    values: FixedVec<i32, 4>;
}
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("FixedVec is a source-visible core carrier for ordinary packages");
}

#[test]
fn compiler_consumes_retained_dependency_generated_source_without_a_physical_file() {
    let tree = TempTree::new();
    let root = tree.package("root-generated-consumer");
    let dependency = tree.package("dependency-generated-producer");
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("root-generated-consumer");
    builder.depend_as("dependency", Source::Path { location: "../dependency-generated-producer" });
}
"#,
    );
    TempTree::write(
        root.join("main.omg"),
        r#"use dependency::generated_api;
pub machine consume_generated_value() -> u64 {
    generated_value()
}
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(identity(1), "root-generated-consumer", root.clone()),
            PackageSourceBinding::new(
                identity(2),
                "dependency-generated-producer",
                dependency.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(2),
        )],
    )
    .expect("generated consumer graph should close");
    let bundle = PackageGeneratedSourceBundle::from_checked(
        identity(2),
        DependencyPurpose::Product,
        target::TargetProfile::WindowsX64,
        target::TargetProfile::host_if_supported(),
        inputs.dependency_closure_for(identity(2)),
        PackageSourceConsumptionCommitment::for_test([12; 32]),
        vec![generated_source(
            b"generated_api.omg",
            b"pub machine generated_value() -> u64 { 17 }\n",
        )],
    );
    let inputs = inputs
        .with_complete_dependency_generated_sources(vec![bundle])
        .expect("consumer should receive the complete dependency bundle");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("retained generated dependency source should enter initial frontend loading");
    assert!(
        !dependency
            .join(".omega/generated/generated_api.omg")
            .exists(),
        "dependency-generated source must remain compiler custody, not a physical snapshot mutation"
    );
    let dependency_generated = checked
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with("generated_api.omg"))
        .expect("dependency-generated source remains in frontend custody");
    assert_eq!(
        dependency_generated.resolution_stratum,
        source::SourceResolutionStratum::Base,
        "an imported dependency bundle is part of the next activation's base, not its extension"
    );
    checked
        .verify_current_source_consumption()
        .expect("generated bytes should verify from retained custody after compilation");
    assert!(checked.source_consumption_commitment().is_some());
    assert_eq!(
        checked.base_source_consumption_commitment(),
        checked.source_consumption_commitment(),
        "an imported dependency-generated bundle is already part of the admitted base"
    );
    assert_eq!(
        checked
            .package_compilation_subject()
            .expect("package subject")
            .root_role(),
        BuildDeclarationKind::Application
    );
    let package_role_inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root-generated-consumer", root.clone()),
            PackageSourceBinding::new(
                identity(2),
                "dependency-generated-producer",
                dependency.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(2),
        )],
    )
    .expect("same graph with package role should validate");
    let package_role_commitment = derive_source_consumption_commitment(
        checked
            .package_compilation_subject()
            .expect("package subject")
            .consumed_units(),
        &package_role_inputs,
    )
    .expect("same consumed sources can be committed under the alternate role");
    assert_ne!(
        checked.source_consumption_commitment(),
        Some(package_role_commitment),
        "root role must participate in exact source-consumption identity"
    );
    assert!(
        checked
            .package_compilation_subject()
            .expect("package subject")
            .consumed_units()
            .iter()
            .any(|unit| matches!(unit.kind(), ConsumedSourceUnitKind::PackageGenerated(_))),
        "final checked subject must classify retained generated source custody"
    );
}

#[test]
fn multi_target_generated_source_failure_is_child_local() {
    let tree = TempTree::new();
    let root = tree.package("multi-target-generated-consumer");
    let dependency = tree.package("multi-target-generated-producer");
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("multi-target-generated-consumer");
    builder.depend_as("dependency", Source::Path { location: "../multi-target-generated-producer" });
}
"#,
    );
    TempTree::write(
        root.join("main.omg"),
        "use dependency::generated_api;\npub machine consume() -> u64 { generated_value() }\n",
    );
    let base_inputs = PackageCompilationInputs::new(
        identity(71),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(
                identity(71),
                "multi-target-generated-consumer",
                root.clone(),
            ),
            PackageSourceBinding::new(identity(72), "multi-target-generated-producer", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(71),
            "dependency",
            identity(72),
        )],
    )
    .expect("multi-target generated-source graph should close");
    let child_inputs = |profile: target::TargetProfile| {
        let source = match profile {
            target::TargetProfile::LinuxX64 => {
                b"pub machine generated_value() -> u64 { 17 }\n".as_slice()
            }
            target::TargetProfile::WindowsX64 => b"pub machine generated_value( {\n".as_slice(),
            _ => unreachable!("fixture target set is exact"),
        };
        let bundle = PackageGeneratedSourceBundle::from_checked(
            identity(72),
            DependencyPurpose::Product,
            profile,
            target::TargetProfile::host_if_supported(),
            base_inputs.dependency_closure_for(identity(72)),
            PackageSourceConsumptionCommitment::for_test([73; 32]),
            vec![generated_source(b"generated_api.omg", source)],
        );
        base_inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![bundle])
            .expect("exact child should retain its dependency-generated bundle")
    };
    let targets = ExplicitTargetSet::from_caller_names(["windows_x64", "linux_x64"])
        .expect("explicit generated-source targets");
    let (sources, _) = base_inputs.clone().into_parts();
    let request = CompileRequest::new(CompileOptions {
        root_path: root.join("main.omg"),
        build_dir: None,
        target_name: None,
    })
    .with_package_sources(sources)
    .with_target_configurations(
        targets
            .profiles()
            .iter()
            .map(|&profile| {
                let (_, target_inputs) = child_inputs(profile).into_parts();
                TargetCompileConfiguration::new(profile)
                    .with_build_dir(tree.0.join("build").join(profile.target_name()))
                    .with_package_target_inputs(target_inputs)
            })
            .collect(),
    );
    let outcomes = compile(request).expect("multi-target request should admit");
    assert_eq!(outcomes.outcomes().len(), 2);
    assert!(outcomes.outcomes()[0].succeeded());
    assert_eq!(
        outcomes.outcomes()[0].target_profile(),
        Some(target::TargetProfile::LinuxX64),
    );
    assert!(!outcomes.outcomes()[1].succeeded());
    assert!(
        outcomes.outcomes()[1]
            .diagnostics()
            .expect("malformed Windows bundle should reject its child")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("generated_api.omg")),
    );
}

#[test]
fn reconciled_bindings_ignore_build_dependency_discovery() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let admitted = tree.package("admitted");
    let malicious = tree.package("malicious");

    TempTree::write(
        root.join("main.omg"),
        "use dep::values;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.package(\"root\");\n    builder.depend_as(\"dep\", Source::Path { location: \"../malicious\" });\n}\n",
    );
    TempTree::write(admitted.join("values.omg"), "const ANSWER: u32 = 42;\n");
    TempTree::write(malicious.join("values.omg"), "this is not Omega source\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "admitted", admitted),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("trusted package binding should be the only dependency authority");
}

#[test]
fn standalone_compilation_does_not_project_dependencies_from_build_syntax() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(
        root.join("main.omg"),
        "use dep::values;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.package(\"root\");\n    builder.depend_as(\"dep\", Source::Path { location: \"../dependency\" });\n}\n",
    );
    TempTree::write(dependency.join("values.omg"), "const ANSWER: u32 = 42;\n");

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&root.join("main.omg"), None))
        .expect_err("standalone compilation must not derive package aliases from build syntax");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(&root.join("dep").display().to_string()),
        "standalone resolution should remain confined to the root package: {rendered}"
    );
    assert!(
        !rendered.contains(&dependency.display().to_string()),
        "dependency declaration must not redirect standalone resolution: {rendered}"
    );
}

#[test]
fn canonical_build_dependency_vocabulary_typechecks() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.package("root");
    builder.depend(Source::Path { location: "../ordinary" });
    builder.depend(Source::Git {
        repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
        revision: "0123456789abcdef"
    });
    builder.depend_as("matrix", Source::Git {
        repository: "https://example.invalid/math.git",
        revision: "0123456789abcdef",
        selection: PackageSelection::Named { package: "matrix" }
    });
    builder.depend_as(
        "arithmetic_kernels",
        Source::Path { location: "../colliding" }
    );
    builder.build_depend(Source::Path { location: "../build-tool" });
    builder.build_depend_as("host_std", Source::Git {
        repository: "https://example.invalid/std.git",
        revision: "0123456789abcdef",
        selection: PackageSelection::Root
    });
}
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("canonical dependency vocabulary should typecheck");
}

#[test]
fn aliases_are_requester_local_and_dependency_imports_are_package_local() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use shared::root_value;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("root_value.omg"),
        "use shared::leaf_value;\nuse local_value;\nconst ROOT_VALUE: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("local_value.omg"),
        "const LOCAL_VALUE: u32 = 1;\n",
    );
    TempTree::write(leaf.join("leaf_value.omg"), "const LEAF_VALUE: u32 = 41;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::new(identity(2), "shared", identity(3)),
        ],
    )
    .expect("requester-local aliases should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("requester-local and package-local imports should compile");
}

#[test]
fn dependency_data_members_and_collection_length_finalize_exactly() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(
        root.join("main.omg"),
        r#"use dep::observation;
data Inspector { items: [u8; 4]; }
machine Inspector::inspect(&self, observation: Observation, bytes: &[u8]) -> u64 {
    transition { _ -> nested(observation, bytes) }

    state nested(&self, observation: Observation, bytes: &[u8]) -> u64 {
        let tag: u8 = observation.tag;
        let direct_length: u64 = bytes.len;
        (self.items[1..3]).len
    }
}
"#,
    );
    TempTree::write(
        dependency.join("observation.omg"),
        "pub data Observation { tag: u8; }\n",
    );

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("dependency graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("public dependency fields and intrinsic length should finalize");
    let selections = checked.authored_declaration_selections();
    assert!(selections.all_finalized(), "selections={selections:#?}");
    assert_eq!(
        selections
            .iter()
            .filter(|selection| {
                selection.kind()
                    == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::MemberAccess
                    && selection.target()
                        == language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
                            language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                        )
            })
            .count(),
        2,
    );
}

#[test]
fn package_measure_body_members_finalize_from_their_authored_source_scope() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(
        root.join("main.omg"),
        r#"use dep::noise;
data Card { power: u64; }
measure Card::PowerOrder(card: Card) -> u64 { card.power }
data Main { }
machine Main::weaken(&mut self, card: Card)
terminates by card -> Card::PowerOrder;
-> u64
{
    transition card.power > 0 {
        true -> weaken(Card { power: card.power - 1 })
        false -> card.power
    }
}
"#,
    );
    TempTree::write(dependency.join("noise.omg"), "pub data Noise { }\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("measure package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("measure-body member custody should rejoin its exact package declaration");
    assert!(
        checked.authored_declaration_selections().all_finalized(),
        "selections={:#?}",
        checked.authored_declaration_selections(),
    );
}

#[test]
fn intrinsic_scalar_operators_remain_builtin_beside_unrelated_operators() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"use omega::language::core::float_operations;

data ForeignNumber { value: f64; }
data Cursor {
    offset: u64 in Wrapping;
    length: u64 in Wrapping;
    total: u64 in Wrapping;
}


operator > ForeignNumber::greater(left: ForeignNumber, right: ForeignNumber) -> bool;
operator == ForeignNumber::equal(left: ForeignNumber, right: ForeignNumber) -> bool;
operator % ForeignNumber::remainder(left: ForeignNumber, right: ForeignNumber) -> ForeignNumber;
operator * ForeignNumber::multiply(left: ForeignNumber, right: ForeignNumber) -> ForeignNumber;

machine has_items(items: &[u8]) -> bool {
    transition items.len > 0 { true -> yes() _ -> no() }
    state yes() -> bool { true }
    state no() -> bool { false }
}

machine bit_is_clear(value: u32) -> bool {
    (value & 128) == 0
}

machine transferred_parameter_is_zero(value: u32) -> bool {
    transition { _ -> check(value) }
    state check(value: u32) -> bool { value == 0 }
}

machine scale_remainder(value: u64) -> u64 {
    (value % 1000) * 1000000
}

linux_x86_64 machine Cursor::record_fits(&self) -> bool {
    transition self.length > 0 && self.offset + self.length <= self.total {
        true -> yes()
        false -> no()
    }
    state yes(&self) -> bool { true }
    state no(&self) -> bool { false }
}
"#,
    );

    let root_identity = identity(1);
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "root",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("collection length remains a builtin u64 operand beside foreign operators");
    assert!(
        checked.authored_declaration_selections().iter().any(|selection| {
            selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Operator
                && selection.target()
                    == language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                    )
        }),
        "selections={:#?}",
        checked.authored_declaration_selections(),
    );
}

#[test]
fn package_build_time_float_operators_accept_toolchain_candidate_authority() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"use omega::language::core::float_operations;

machine array_length() -> u64 {
    let left: f64 = 2.0;
    let right: f64 = 3.0;
    let product: f64 = left * right;
    transition product == 6.0 { true -> 4 _ -> 5 }
}

data Main { bytes: [u8; array_length()]; }
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("toolchain float candidates are already confined package authority");
    assert!(checked.authored_declaration_selections().all_finalized());
    let main = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .expect("Main data");
    let field = checked
        .data_members(main)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "bytes" => {
                Some(field)
            }
            _ => None,
        })
        .expect("Main.bytes field");
    assert!(
        checked
            .type_reference_table
            .fixed_array_lengths()
            .any(|(handle, length)| handle == field.type_reference
                && *length == typed_trees::types::FixedArrayLength::Literal(4)),
        "Main.bytes must retain the evaluated length 4"
    );
}

#[test]
fn package_build_time_integer_comparison_ignores_unrelated_float_operators() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"
use omega::language::core::float_operations;
machine capacity() -> u64 {
    let value: u64 = 8;
    transition value < 256 { true -> 4 _ -> 5 }
}
data Main { bytes: [u8; capacity()]; }
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("integer comparison remains build-time admissible beside float operators");
    let main = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap();
    let [typed_trees::data::DataMember::Field(field)] = checked.data_members(main) else {
        panic!("Main retains exactly its authored byte array");
    };
    assert!(
        checked
            .type_reference_table
            .fixed_array_lengths()
            .any(|(handle, length)| handle == field.type_reference
                && *length == typed_trees::types::FixedArrayLength::Literal(4)),
        "the actual build-time branch must determine the array length"
    );
}

#[test]
fn authored_selection_requires_the_declaration_owner_as_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine root_value() -> u32 { leaf_value() }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_value() -> u32 { leaf_value() }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_value() -> u32 { 42 }\n",
    );

    let transitive_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("root may not select a transitive-only leaf declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the exact leaf declaration selection");
}

#[test]
fn boundary_machine_signatures_are_public_package_selection_positions() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nboundary machine expose(value: LeafValue);\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: LeafValue) { }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub data LeafValue { value: u64; }\n",
    );

    let transitive_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive boundary-signature graph should validate structurally");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a boundary signature may not select a transitive-only type");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected boundary-signature diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct boundary-signature graph should validate");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the boundary signature type");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.exposure()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.package_identity == Some(identity(1)))
            && matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::").contains("LeafValue")
            )
    }));

    TempTree::write(
        root.join("main.omg"),
        "data PrivateValue { value: u64; }\nboundary machine expose_private(value: PrivateValue);\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only boundary-signature graph should validate structurally");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a boundary signature may not expose a private same-package type");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private data")
                && diagnostic.message.contains("PrivateValue")
        }),
        "unexpected private boundary-signature diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn transparent_domain_alias_constituents_require_direct_package_admission() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\npub domain u64::RootAllowed = u64::LeafAllowed;\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: u64) -> u64 { value }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub domain u64::LeafAllowed;\n");
    let bindings = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle.clone()),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let transitive = PackageCompilationInputs::new_package(
        identity(1),
        bindings.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive alias graph should close");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a domain alias may not select a transitive-only constituent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected domain-alias diagnostics: {diagnostics:#?}"
    );

    let direct = PackageCompilationInputs::new_package(
        identity(1),
        bindings,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct alias graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct admission should admit the alias constituent");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.exposure()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment
            && checked.symbols.source_file(selection.source_span()).is_some_and(|source| source.package_identity == Some(identity(1)))
            && matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("LeafAllowed"))
    }));
}

#[test]
fn domain_establishment_routes_retain_exact_declarations_and_visibility() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"pub data Ticket { }
trait HiddenIssues {
    machine issue() -> Ticket in Ticket::Issued;
}
pub domain Ticket::Issued
established by HiddenIssues::issue;
"#,
    );
    let root_only = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only establishment graph should close")
    };
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a public catalog may retain a private trait requirement");

    TempTree::write(
        root.join("main.omg"),
        r#"pub data Ticket { }
pub trait Issues {
    machine issue() -> Ticket in Ticket::Issued;
}
pub domain Ticket::Issued
established by Issues::issue;

trait InternalIssues {
    machine hide() -> Ticket in Ticket::Internal;
}
domain Ticket::Internal
established by InternalIssues::hide;
"#,
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("matching route visibility should admit exact establishment selections");
    let rows = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.package_identity == Some(identity(1)))
                && matches!(
                    selection.target(),
                    Target::Resolved(target)
                        if ["Issues", "Issues::issue", "InternalIssues", "InternalIssues::hide"]
                            .iter()
                            .any(|path| checked.symbols.display_path(target.selected_symbol(), "::").ends_with(path))
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4, "rows={rows:#?}");
    assert_eq!(
        rows.iter()
            .filter(|selection| selection.kind() == Kind::DomainIssuerAuthorization)
            .count(),
        4
    );
    assert_eq!(
        rows.iter()
            .filter(|selection| selection.exposure() == Exposure::PublicInterface)
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|selection| selection.exposure() == Exposure::PrivateImplementation)
            .count(),
        2
    );
}

#[test]
fn trait_composition_requires_direct_package_admission() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub trait HeaderComposition: LeafPolicy {
}
pub trait BodyComposition {
    requires LeafPolicy;
}
trait PrivateComposition: LeafPolicy {
}
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: u64) -> u64 { value }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub trait LeafPolicy {\n}\n");
    let bindings = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle.clone()),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let transitive = PackageCompilationInputs::new_package(
        identity(1),
        bindings.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive trait-composition graph should close");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("trait composition may not select a transitive-only trait");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected trait-composition diagnostics: {diagnostics:#?}"
    );

    let direct = PackageCompilationInputs::new_package(
        identity(1),
        bindings,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct trait-composition graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct admission should admit trait composition");
    let leaf_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == Kind::TypeReference
                && checked
                    .symbols
                    .source_file(selection.source_span())
                    .is_some_and(|source| source.package_identity == Some(identity(1)))
                && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("LeafPolicy"))
        })
        .collect::<Vec<_>>();
    assert_eq!(leaf_selections.len(), 3);
    assert_eq!(
        leaf_selections
            .iter()
            .filter(|selection| selection.exposure() == Exposure::PublicInterface)
            .count(),
        2,
    );
    assert_eq!(
        leaf_selections
            .iter()
            .filter(|selection| selection.exposure() == Exposure::PrivateImplementation)
            .count(),
        1,
    );
}

#[test]
fn attached_machine_carriers_require_direct_package_admission() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine LeafData::public_extension(value: u64) -> u64 {
    value
}
machine LeafData::private_extension(value: u64) -> u64 {
    value
}
boundary machine LeafData::boundary_extension(value: u64) -> u64;
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: u64) -> u64 { value }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub data LeafData {\n}\n");
    let bindings = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle.clone()),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let transitive = PackageCompilationInputs::new_package(
        identity(1),
        bindings.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive attached-carrier graph should close");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("an attached machine may not select a transitive-only carrier");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected attached-carrier diagnostics: {diagnostics:#?}"
    );

    let direct = PackageCompilationInputs::new_package(
        identity(1),
        bindings,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct attached-carrier graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct admission should admit attached-machine carriers");
    let carrier_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == Kind::TypeReference
                && checked
                    .symbols
                    .source_file(selection.source_span())
                    .is_some_and(|source| source.package_identity == Some(identity(1)))
                && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("LeafData"))
        })
        .collect::<Vec<_>>();
    assert_eq!(carrier_selections.len(), 3);
    assert_eq!(
        carrier_selections
            .iter()
            .filter(|selection| selection.exposure() == Exposure::PublicInterface)
            .count(),
        2,
    );
    assert_eq!(
        carrier_selections
            .iter()
            .filter(|selection| selection.exposure() == Exposure::PrivateImplementation)
            .count(),
        1,
    );
}

#[test]
fn machine_satisfies_edges_require_exact_direct_package_admission() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine public_apply(value: u64) -> u64
    satisfies LeafPolicy::apply
{
    value
}
machine private_apply(value: u64) -> u64
    satisfies LeafPolicy::apply
{
    value
}
boundary machine boundary_apply(value: u64) -> u64
    satisfies LeafPolicy::apply;
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: u64) -> u64 { value }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait LeafPolicy {
    machine apply(value: u64) -> u64;
}
"#,
    );
    let bindings = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle.clone()),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let transitive = PackageCompilationInputs::new_package(
        identity(1),
        bindings.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive satisfies graph should close");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a machine may not satisfy a transitive-only requirement");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected machine-satisfies diagnostics: {diagnostics:#?}"
    );

    let direct = PackageCompilationInputs::new_package(
        identity(1),
        bindings,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct satisfies graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct admission should admit exact machine-satisfies edges");
    let root_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.package_identity == Some(identity(1)))
        })
        .collect::<Vec<_>>();
    for kind in [Kind::TypeReference, Kind::StaticPathSegment] {
        let selections = root_selections
            .iter()
            .filter(|selection| {
                selection.kind() == kind
                    && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains(if kind == Kind::TypeReference { "LeafPolicy" } else { "apply" }))
            })
            .collect::<Vec<_>>();
        assert_eq!(selections.len(), 3);
        assert_eq!(
            selections
                .iter()
                .filter(|selection| selection.exposure() == Exposure::PublicInterface)
                .count(),
            2,
        );
        assert_eq!(
            selections
                .iter()
                .filter(|selection| selection.exposure() == Exposure::PrivateImplementation)
                .count(),
            1,
        );
    }
    for machine in checked.machines().iter().filter(|machine| {
        matches!(
            machine.name.as_str(),
            "public_apply" | "private_apply" | "boundary_apply"
        )
    }) {
        let [conformance] = checked.machine_trait_conformances(machine) else {
            panic!("one exact satisfies edge for {}", machine.name)
        };
        assert!(conformance.requirement_symbol.is_valid());
        assert_eq!(
            checked.symbols.name(conformance.requirement_symbol),
            "apply"
        );
    }
}

#[test]
fn public_machine_satisfies_edges_reject_private_requirements() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only satisfies graph should close")
    };
    let source = |machine_prefix: &str| {
        format!(
            r#"trait PrivatePolicy {{
    machine apply(value: u64) -> u64;
}}
{machine_prefix}machine apply(value: u64) -> u64
    satisfies PrivatePolicy::apply
{{
    value
}}
"#,
        )
    };

    TempTree::write(root.join("main.omg"), &source("pub "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public satisfier may not expose a private requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private")
                && (diagnostic.message.contains("PrivatePolicy")
                    || diagnostic.message.contains("apply"))
        }),
        "unexpected public private-requirement diagnostics: {diagnostics:#?}"
    );

    TempTree::write(root.join("main.omg"), &source(""));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a private satisfier may select its package-private requirement");

    TempTree::write(root.join("main.omg"), &source("boundary "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("an exported boundary satisfier may not expose a private requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private")
                && (diagnostic.message.contains("PrivatePolicy")
                    || diagnostic.message.contains("apply"))
        }),
        "unexpected boundary private-requirement diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn nominal_machine_parameter_requirements_need_exact_direct_package_admission() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine public_register<machine Schema>()
where machine Schema<machine Selected>()
where machine Selected satisfies LeafPolicy::apply;
{}
machine private_register<machine Selected>()
where machine Selected satisfies LeafPolicy::apply;
{}
boundary machine boundary_register<machine Selected>()
where machine Selected satisfies LeafPolicy::apply;
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine relay(value: u64) -> u64 { value }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait LeafPolicy {
    machine apply(value: u64) -> u64;
}
"#,
    );
    let bindings = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle.clone()),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let transitive = PackageCompilationInputs::new_package(
        identity(1),
        bindings.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive nominal-requirement graph should close");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a nominal machine parameter may not select a transitive-only requirement");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected nominal machine-parameter diagnostics: {diagnostics:#?}"
    );

    let direct = PackageCompilationInputs::new_package(
        identity(1),
        bindings,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct nominal-requirement graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct admission should admit exact nominal machine-parameter requirements");
    let root_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.package_identity == Some(identity(1)))
        })
        .collect::<Vec<_>>();
    for (kind, selected_name) in [
        (Kind::TypeReference, "LeafPolicy"),
        (Kind::StaticPathSegment, "apply"),
    ] {
        let selections = root_selections
            .iter()
            .filter(|selection| {
                selection.kind() == kind
                    && matches!(selection.target(), Target::Resolved(target) if checked.symbols.name(target.selected_symbol()) == selected_name)
            })
            .collect::<Vec<_>>();
        assert_eq!(selections.len(), 3);
        assert_eq!(
            selections
                .iter()
                .filter(|selection| selection.exposure() == Exposure::PublicInterface)
                .count(),
            2,
        );
        assert_eq!(
            selections
                .iter()
                .filter(|selection| selection.exposure() == Exposure::PrivateImplementation)
                .count(),
            1,
        );
    }
}

#[test]
fn public_nominal_machine_parameters_reject_private_requirements() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only nominal-requirement graph should close")
    };
    let source = |machine_prefix: &str| {
        format!(
            r#"trait PrivatePolicy {{
    machine apply(value: u64) -> u64;
}}
{machine_prefix}machine register<machine Selected>()
where machine Selected satisfies PrivatePolicy::apply;
{{}}
"#,
        )
    };

    TempTree::write(root.join("main.omg"), &source("pub "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public nominal binder may not expose a private requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private")
                && (diagnostic.message.contains("PrivatePolicy")
                    || diagnostic.message.contains("apply"))
        }),
        "unexpected public nominal-requirement diagnostics: {diagnostics:#?}"
    );

    TempTree::write(root.join("main.omg"), &source(""));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a private nominal binder may select its package-private requirement");

    TempTree::write(root.join("main.omg"), &source("boundary "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("an exported boundary nominal binder may not expose a private requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private")
                && (diagnostic.message.contains("PrivatePolicy")
                    || diagnostic.message.contains("apply"))
        }),
        "unexpected boundary nominal-requirement diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn private_domain_issuer_catalog_does_not_grant_dependency_access() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let library = tree.package("library");
    TempTree::write(
        library.join("issuance.omg"),
        r#"
pub domain u64::Issued requires self > 0; established by issue;
machine issue() -> u64 in Issued { 7 }
pub machine forward() -> u64 in Issued { issue() }
trait HiddenIssues { machine grant() -> u64 in Issued; }
"#,
    );
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "library", library.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "library",
                identity(2),
            )],
        )
        .unwrap()
    };
    TempTree::write(
        root.join("main.omg"),
        "use library::issuance; machine consume() -> u64 in Issued { forward() }",
    );
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("consumer may forward a public wrapper's issued value");
    for source in [
        "use library::issuance; machine consume() -> u64 in Issued { issue() }",
        "use library::issuance; boundary machine grant() -> u64 in Issued satisfies HiddenIssues::grant;",
    ] {
        TempTree::write(root.join("main.omg"), source);
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs()),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("catalog metadata cannot grant private declaration access");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("private")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn private_issuer_catalog_exception_does_not_publish_other_interface_dependencies() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for source in [
        "data Hidden {} pub domain Hidden::Issued established by issue; machine issue() -> Hidden in Issued { Hidden {} }",
        "pub domain u64::Issued requires self>0; established by issue; machine issue()->u64 in Issued {7} pub domain u64::Leaked requires issue()>0;",
        "data Factory {} pub domain u64::Issued requires self>0; established by Factory::issue; machine Factory::issue()->u64 in Issued {7} pub machine forward(value:Factory)->u64 in Issued {Factory::issue()}",
    ] {
        TempTree::write(root.join("main.omg"), source);
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .unwrap();
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err(
            "ordinary public carrier, predicate and signature occurrences remain nameable API",
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("public interface selects private")),
            "{source}: {diagnostics:?}"
        );
    }
}
