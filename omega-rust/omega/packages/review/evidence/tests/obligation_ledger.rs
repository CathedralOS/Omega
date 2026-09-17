use crate::support;

use compiler::CheckedCompileRequest;
use support::*;

#[test]
fn accepted_claim_results_join_encoded_keys_without_reordering_callables() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary machine aa() -> u64 ensures result == 0;\nboundary machine z() -> u64 ensures result == 1;\n");
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("two distinct accepted boundary claims check");
    let fresh = package_evidence::ledger::reconstruct_package_review(&checked)
        .expect("accepted claims rejoin their own canonical rows");
    let claims = fresh.results.open_accepted_claims();
    assert_eq!(
        claims
            .iter()
            .map(|claim| claim.callable().identity().path())
            .collect::<Vec<_>>(),
        ["aa", "z"]
    );
    let rows = fresh
        .canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].key_bytes().ends_with(b"z"));
    assert!(rows[1].key_bytes().ends_with(b"aa"));
    for claim in claims {
        let row = rows
            .iter()
            .find(|row| {
                row.key_bytes()
                    .ends_with(claim.callable().identity().path().as_bytes())
            })
            .expect("the exact nominal-key row exists");
        assert_eq!(claim.row().key_bytes(), row.key_bytes());
        assert_eq!(claim.row().canonical_bytes(), row.canonical_bytes());
    }
}

#[test]
fn dangerous_authority_results_join_service_keys_not_class_order() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait FilesystemHost {}
pub boundary trait Console {
    machine exit_process(return_code: i32) reaches Console;
}
pub data ConsoleNativeProvider {}
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
pub machine expose() reaches Console + FilesystemHost {}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<Console, ConsoleNativeProvider>();
}
"#,
    );
    let candidate = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("two exposed service candidates check");
    let filesystem = candidate
        .custody
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            package_identity(),
            "FilesystemHost",
        )
        .expect("derive exact filesystem binding");
    let plan = candidate
        .custody
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Console")
        .expect("selected Console plan");
    let console = package_compilation::AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        package_identity(),
        "Console",
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .expect("derive exact Console binding");
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(
            package_inputs(&package.0)
                .with_accepted_semantic_bindings(vec![filesystem, console])
                .expect("both bindings belong to the fixture package"),
        ),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("both accepted service identities check");
    let fresh = package_evidence::ledger::reconstruct_package_review(&checked)
        .expect("dangerous authorities rejoin their own canonical rows");
    let authorities = fresh.results.open_dangerous_authorities();
    assert_eq!(
        authorities
            .iter()
            .map(|authority| authority.authority().service().path())
            .collect::<Vec<_>>(),
        ["FilesystemHost", "Console"]
    );
    let rows = fresh
        .canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].key_bytes().ends_with(b"Console"));
    assert!(rows[1].key_bytes().ends_with(b"FilesystemHost"));
    for authority in authorities {
        let row = rows
            .iter()
            .find(|row| {
                row.key_bytes()
                    .ends_with(authority.authority().service().path().as_bytes())
            })
            .expect("the exact service-key row exists");
        assert_eq!(authority.row().key_bytes(), row.key_bytes());
        assert_eq!(authority.row().canonical_bytes(), row.canonical_bytes());
    }
}

#[test]
fn obligation_ledger_binds_and_recovers_application_root_role() {
    let Some(target) = host_target_name() else {
        return;
    };
    let application = TempPackage::new();
    application.write("main.omg", "pub data Token { value: u64; }\n");
    application.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.application("review-application"); }
"#,
    );
    let identity = package_identity();
    let inputs = PackageCompilationInputs::new(
        identity,
        BuildDeclarationKind::Application,
        vec![PackageSourceBinding::new(
            identity,
            "review-application",
            application.0.clone(),
        )],
        vec![],
    )
    .expect("application package graph");
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&application.0.join("main.omg"), Some(target))
    })
    .expect("application root should check");
    let rows = project_checked_package_review(&checked)
        .expect("application review")
        .canonical_rows()
        .expect("application rows");
    let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
        checked
            .custody
            .dependency_closure()
            .cloned()
            .expect("compiler retains dependency closure"),
        &rows,
    )
    .expect("application obligation ledger");
    assert_eq!(
        ledger.dependency_closure().root_role(),
        BuildDeclarationKind::Application
    );
    let encoded = encode_ordinary_package_obligation_ledger(&ledger).expect("encode ledger");
    let decoded = decode_ordinary_package_obligation_ledger(&encoded).expect("recover ledger");
    assert_eq!(
        decoded.dependency_closure().root_role(),
        BuildDeclarationKind::Application
    );

    let mut workspace_role = encoded;
    let role_offset = ledger_target_range(&workspace_role).end + 32;
    workspace_role[role_offset] = 2;
    assert!(
        decode_ordinary_package_obligation_ledger(&workspace_role)
            .expect_err("workspace role cannot enter a package ledger")
            .message()
            .contains("workspace role")
    );
}

#[test]
fn ordinary_package_obligation_ledger_requires_exact_local_reconstruction() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let original = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub const LIMIT: u64 = 7;
pub data Token [copy] { value: u64; }
pub machine constant<const Value: u64>() -> u64 { 0 }
pub machine observes_computed_member(value: u64)
requires (Token { value: value }).value == value
{}
pub machine observes_collection_view(bytes: [u8; 4])
requires valid_utf8(bytes.as_slice())
{}
boundary machine observes_named_const() -> u64
ensures result == constant<LIMIT>();
"#,
    );
    original.write("build.omg", build);
    let original_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&original.0)),
        ..CheckedCompileRequest::new(&original.0.join("main.omg"), Some(target))
    })
    .expect("ordinary package obligation fixture should check");
    let projection = project_checked_package_review(&original_checked)
        .expect("ordinary package obligations should project");
    let rows = projection
        .canonical_rows()
        .expect("ordinary package obligation rows");
    let dependency_closure = original_checked
        .custody
        .dependency_closure()
        .cloned()
        .expect("package-aware compilation retains its dependency closure");
    let ledger =
        ordinary_package_obligation_ledger_from_compiler_rows(dependency_closure.clone(), &rows)
            .expect("fresh compiler rows should form a canonical ledger");
    validate_ordinary_package_obligation_ledger(&ledger, &original_checked)
        .expect("unchanged checked semantics should reconstruct the same ledger");

    let ledger_bytes = encode_ordinary_package_obligation_ledger(&ledger)
        .expect("ordinary package obligation ledger should encode canonically");
    let decoded_ledger = decode_ordinary_package_obligation_ledger(&ledger_bytes)
        .expect("ordinary package obligation ledger should decode canonically");
    assert_eq!(decoded_ledger, ledger);
    validate_ordinary_package_obligation_ledger(&decoded_ledger, &original_checked)
        .expect("decoded ledger remains inert until exact local reconstruction succeeds");
    assert_eq!(
        ordinary_package_obligation_ledger_fingerprint(&decoded_ledger).unwrap(),
        ordinary_package_obligation_ledger_fingerprint(&ledger).unwrap()
    );

    let mut unknown_schema = ledger_bytes.clone();
    let schema_offset = LEDGER_MAGIC.len() + std::mem::size_of::<u16>();
    unknown_schema[schema_offset..schema_offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&unknown_schema)
        .expect_err("unknown obligation semantics must reject");
    assert!(
        error
            .message()
            .contains("unsupported ordinary package obligation schema")
    );

    let mut trailing = ledger_bytes.clone();
    trailing.push(0);
    let error = decode_ordinary_package_obligation_ledger(&trailing)
        .expect_err("trailing ledger bytes must reject");
    assert!(error.message().contains("trailing bytes"));

    let mut changed_package = ledger_bytes.clone();
    let package_offset = LEDGER_MAGIC.len() + 4 * std::mem::size_of::<u16>();
    changed_package[package_offset..package_offset + 32].copy_from_slice(&[88; 32]);
    let error = decode_ordinary_package_obligation_ledger(&changed_package)
        .expect_err("ledger package and row package must agree");
    assert!(error.message().contains("different package identity"));

    let mut changed_target = ledger_bytes.clone();
    let target_range = ledger_target_range(&changed_target);
    let replacement_target = if target == "linux_arm64" {
        b"macos_arm64".as_slice()
    } else {
        b"linux_arm64".as_slice()
    };
    let target_length_range = target_range.start - std::mem::size_of::<u64>()..target_range.start;
    let replacement_target_length =
        u64::try_from(replacement_target.len()).expect("canonical target length fits u64");
    changed_target[target_length_range].copy_from_slice(&replacement_target_length.to_le_bytes());
    changed_target.splice(target_range, replacement_target.iter().copied());
    let error = decode_ordinary_package_obligation_ledger(&changed_target)
        .expect_err("ledger target and row target must agree");
    assert!(error.message().contains("different target"));

    let row_frames = ledger_row_frames(&ledger_bytes);
    assert!(row_frames.len() >= 2);
    let mut reordered_rows = Vec::new();
    reordered_rows.extend_from_slice(&ledger_bytes[..row_frames[0].start]);
    reordered_rows.extend_from_slice(&ledger_bytes[row_frames[1].clone()]);
    reordered_rows.extend_from_slice(&ledger_bytes[row_frames[0].clone()]);
    reordered_rows.extend_from_slice(&ledger_bytes[row_frames[1].end..]);
    let error = decode_ordinary_package_obligation_ledger(&reordered_rows)
        .expect_err("reordered canonical ledger rows must reject");
    assert!(error.message().contains("strict canonical order"));

    let decoded = rows
        .iter()
        .map(|row| {
            let bytes = encode_package_review_canonical_row(row)
                .expect("compiler row recovery envelope should encode");
            decode_package_review_canonical_row(&bytes)
                .expect("compiler row recovery envelope should decode")
        })
        .collect::<Vec<_>>();
    let recovered =
        recover_ordinary_package_obligation_ledger(dependency_closure.clone(), &decoded)
            .expect("decoded rows should form the same canonical ledger");
    assert_eq!(recovered, ledger);
    validate_ordinary_package_obligation_ledger(&recovered, &original_checked)
        .expect("decoded framing is inert until exact local reconstruction succeeds");

    let mut missing = rows.clone();
    let removed = missing
        .iter()
        .position(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("fixture should produce a public-data row");
    missing.remove(removed);
    let incomplete =
        ordinary_package_obligation_ledger_from_compiler_rows(dependency_closure.clone(), &missing)
            .expect("an omitted semantic row remains structurally decodable");
    let diagnostics = validate_ordinary_package_obligation_ledger(&incomplete, &original_checked)
        .expect_err("local reconstruction must reject an omitted semantic row");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match local reconstruction")),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    let ordering_error =
        ordinary_package_obligation_ledger_from_compiler_rows(dependency_closure, &reordered)
            .expect_err("reordered rows are not a canonical ledger");
    assert!(ordering_error.message().contains("strict canonical order"));

    let changed = TempPackage::new();
    changed.write(
        "main.omg",
        r#"pub const LIMIT: u64 = 8;
pub data Token [copy] { value: i64; }
pub machine constant<const Value: u64>() -> u64 { 0 }
pub machine observes_computed_member(value: i64)
requires (Token { value: value }).value == value
{}
pub machine observes_collection_view(bytes: [u8; 4])
requires valid_utf8(bytes.as_slice())
{}
boundary machine observes_named_const() -> u64
ensures result == constant<LIMIT>();
"#,
    );
    changed.write("build.omg", build);
    let changed_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&changed.0)),
        ..CheckedCompileRequest::new(&changed.0.join("main.omg"), Some(target))
    })
    .expect("changed ordinary package obligation fixture should check");
    let diagnostics = validate_ordinary_package_obligation_ledger(&ledger, &changed_checked)
        .expect_err("stale semantic rows must reject against changed checked source");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not match local reconstruction")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn ordinary_package_obligation_ledger_binds_exact_dependency_closure_without_paths() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let root = TempPackage::new();
    root.write("main.omg", "pub data Token { value: u64; }\n");
    root.write("build.omg", build);
    let dependency = TempPackage::new();
    dependency.write("main.omg", "pub data DependencyToken {}\n");
    dependency.write("build.omg", build);
    let root_identity = package_identity();
    let dependency_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("dependency package identity");
    let graph_inputs = |root_path: &Path, dependency_path: &Path, alias: &str| {
        PackageCompilationInputs::new_package(
            root_identity,
            vec![
                PackageSourceBinding::new(root_identity, "review-fixture", root_path.to_owned()),
                PackageSourceBinding::new(
                    dependency_identity,
                    "graph-dependency",
                    dependency_path.to_owned(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_identity,
                alias,
                dependency_identity,
            )],
        )
        .expect("two-package graph should validate")
    };
    let compile_graph = |root_path: &Path, dependency_path: &Path, alias: &str| {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(graph_inputs(root_path, dependency_path, alias)),
            ..CheckedCompileRequest::new(&root_path.join("main.omg"), Some(target))
        })
        .expect("unused dependency graph should check")
    };
    let ledger_for = |checked: &ReviewFixture| {
        let rows = project_checked_package_review(checked)
            .expect("dependency-closure review should project")
            .canonical_rows()
            .expect("dependency-closure canonical rows");
        ordinary_package_obligation_ledger_from_compiler_rows(
            checked
                .custody
                .dependency_closure()
                .cloned()
                .expect("package-aware compilation retains its dependency closure"),
            &rows,
        )
        .expect("dependency-closure ledger should form")
    };

    let original_checked = compile_graph(&root.0, &dependency.0, "dependency");
    let renamed_checked = compile_graph(&root.0, &dependency.0, "renamed_dependency");
    let original_rows = project_checked_package_review(&original_checked)
        .expect("original review")
        .canonical_rows()
        .expect("original rows");
    let renamed_rows = project_checked_package_review(&renamed_checked)
        .expect("renamed review")
        .canonical_rows()
        .expect("renamed rows");
    assert_eq!(
        original_rows, renamed_rows,
        "an unused requester-local alias does not alter checked semantic rows"
    );
    let original_ledger = ledger_for(&original_checked);
    let renamed_ledger = ledger_for(&renamed_checked);
    assert_ne!(
        original_ledger, renamed_ledger,
        "the exact compiler-consumed alias still enters ledger identity"
    );
    assert_ne!(
        ordinary_package_obligation_ledger_fingerprint(&original_ledger).unwrap(),
        ordinary_package_obligation_ledger_fingerprint(&renamed_ledger).unwrap(),
        "the canonical whole-ledger identity binds requester-local aliases"
    );
    let original_bytes = encode_ordinary_package_obligation_ledger(&original_ledger).unwrap();
    assert_eq!(
        decode_ordinary_package_obligation_ledger(&original_bytes).unwrap(),
        original_ledger
    );

    let package_range = ledger_closure_package_range(&original_bytes);
    assert_eq!(package_range.len(), 64);
    let mut reordered_packages = original_bytes.clone();
    let first_package = reordered_packages[package_range.start..package_range.start + 32].to_vec();
    let second_package = reordered_packages[package_range.start + 32..package_range.end].to_vec();
    reordered_packages[package_range.start..package_range.start + 32]
        .copy_from_slice(&second_package);
    reordered_packages[package_range.start + 32..package_range.end].copy_from_slice(&first_package);
    let error = decode_ordinary_package_obligation_ledger(&reordered_packages)
        .expect_err("noncanonical closure package ordering must reject");
    assert!(error.message().contains("strict canonical order"));

    let alias = b"dependency";
    let alias_start = original_bytes
        .windows(alias.len())
        .position(|window| window == alias)
        .expect("canonical closure retains its requester-local alias");
    let mut invalid_alias = original_bytes.clone();
    invalid_alias[alias_start] = b'D';
    let error = decode_ordinary_package_obligation_ledger(&invalid_alias)
        .expect_err("noncanonical closure alias must reject");
    assert!(error.message().contains("noncanonical alias"));

    let mut open_edge = original_bytes.clone();
    open_edge[alias_start + alias.len()..alias_start + alias.len() + 32].copy_from_slice(&[77; 32]);
    let error = decode_ordinary_package_obligation_ledger(&open_edge)
        .expect_err("open closure edge must reject");
    assert!(error.message().contains("open edge"));
    let diagnostics =
        validate_ordinary_package_obligation_ledger(&original_ledger, &renamed_checked)
            .expect_err("a stale dependency closure must reject local reconstruction");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("dependency closure does not match local reconstruction")),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let without_dependency = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&root.0)),
        ..CheckedCompileRequest::new(&root.0.join("main.omg"), Some(target))
    })
    .expect("root-only graph should check");
    assert_ne!(
        original_ledger,
        ledger_for(&without_dependency),
        "adding or removing an otherwise unused reachable package changes the ledger"
    );

    let relocated_root = TempPackage::new();
    relocated_root.write("main.omg", "pub data Token { value: u64; }\n");
    relocated_root.write("build.omg", build);
    let relocated_dependency = TempPackage::new();
    relocated_dependency.write("main.omg", "pub data DependencyToken {}\n");
    relocated_dependency.write("build.omg", build);
    let relocated_checked = compile_graph(&relocated_root.0, &relocated_dependency.0, "dependency");
    assert_eq!(
        original_ledger,
        ledger_for(&relocated_checked),
        "source/cache relocation does not enter the dependency-closure coordinate"
    );

    let different_root =
        PackageKeyIdentity::from_digest([99; 32]).expect("different root package identity");
    let wrong_root_closure = PackageCompilationInputs::new_package(
        different_root,
        vec![PackageSourceBinding::new(
            different_root,
            "review-fixture",
            root.0.clone(),
        )],
        Vec::new(),
    )
    .expect("alternate root graph should validate")
    .dependency_closure();
    let error =
        ordinary_package_obligation_ledger_from_compiler_rows(wrong_root_closure, &original_rows)
            .expect_err("row package and dependency-closure root must agree");
    assert!(error.message().contains("different root package"));
}

#[test]
fn package_source_consumption_commitment_binds_loaded_bytes_not_cache_location() {
    let Some(target) = host_target_name() else {
        return;
    };
    let source = "pub data Token { value: i64; }\n";
    let changed_source = "// source-only change\npub data Token { value: i64; }\n";
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;

    let first = TempPackage::new();
    first.write("main.omg", source);
    first.write("build.omg", build);
    let first_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&first.0)),
        ..CheckedCompileRequest::new(&first.0.join("main.omg"), Some(target))
    })
    .expect("first package source should check");
    let first_commitment = first_checked
        .custody
        .source_consumption_commitment()
        .expect("package-aware compilation must retain source consumption");
    assert_ne!(first_commitment.digest(), [0; 32]);
    first_checked
        .custody
        .verify_current_source_consumption()
        .expect("unchanged loaded source should verify");

    let relocated = TempPackage::new();
    relocated.write("main.omg", source);
    relocated.write("build.omg", build);
    let relocated_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&relocated.0)),
        ..CheckedCompileRequest::new(&relocated.0.join("main.omg"), Some(target))
    })
    .expect("relocated package source should check");
    assert_eq!(
        first_commitment,
        relocated_checked
            .custody
            .source_consumption_commitment()
            .expect("relocated package source commitment"),
        "absolute cache location and source-id assignment are not source identity"
    );

    let changed = TempPackage::new();
    changed.write("main.omg", changed_source);
    changed.write("build.omg", build);
    let changed_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&changed.0)),
        ..CheckedCompileRequest::new(&changed.0.join("main.omg"), Some(target))
    })
    .expect("source-only changed package should check");
    assert_ne!(
        first_commitment,
        changed_checked
            .custody
            .source_consumption_commitment()
            .expect("changed package source commitment")
    );
    let first_review =
        project_checked_package_review(&first_checked).expect("first package review");
    let changed_review =
        project_checked_package_review(&changed_checked).expect("changed package review");
    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first package review bytes"),
        changed_review
            .canonical_review_bytes()
            .expect("changed package review bytes"),
        "source consumption and normalized capability/API comparison remain separate identities"
    );
    let first_rows = first_review.canonical_rows().expect("first canonical rows");
    let changed_rows = changed_review
        .canonical_rows()
        .expect("changed-source canonical rows");
    assert_eq!(
        first_rows, changed_rows,
        "source-only changes remain outside every normalized capability/API row"
    );
    let first_data = first_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("public data row");
    let changed_data = changed_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("changed-source public data row");
    let first_locations = first_data
        .source()
        .authored_locations()
        .expect("public data receives an authored source coordinate");
    let changed_locations = changed_data
        .source()
        .authored_locations()
        .expect("changed public data receives an authored source coordinate");
    let first_declaration = first_locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::Declaration)
        .expect("public data declaration location");
    let changed_declaration = changed_locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::Declaration)
        .expect("changed public data declaration location");
    assert_eq!(first_declaration.relative_path(), "main.omg");
    assert_eq!(changed_declaration.relative_path(), "main.omg");
    assert!(changed_declaration.start_byte() > first_declaration.start_byte());
    assert!(
        !first_declaration
            .relative_path()
            .contains(&first.0.display().to_string())
    );

    first.write("main.omg", changed_source);
    assert!(
        first_checked
            .custody
            .verify_current_source_consumption()
            .is_err(),
        "loaded source drift must reject against the retained compiler bytes"
    );

    let graph_root = TempPackage::new();
    graph_root.write("main.omg", source);
    graph_root.write("build.omg", build);
    let dependency = TempPackage::new();
    dependency.write("main.omg", "pub data DependencyToken {}\n");
    dependency.write("build.omg", build);
    let root_identity = package_identity();
    let dependency_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("dependency package identity");
    let graph_inputs = |alias: &str| {
        PackageCompilationInputs::new_package(
            root_identity,
            vec![
                PackageSourceBinding::new(root_identity, "graph-root", graph_root.0.clone()),
                PackageSourceBinding::new(
                    dependency_identity,
                    "graph-dependency",
                    dependency.0.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_identity,
                alias,
                dependency_identity,
            )],
        )
        .expect("two-package graph should validate")
    };
    let first_graph = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(graph_inputs("dependency")),
        ..CheckedCompileRequest::new(&graph_root.0.join("main.omg"), Some(target))
    })
    .expect("first reconciled graph should check");
    let renamed_graph = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(graph_inputs("renamed_dependency")),
        ..CheckedCompileRequest::new(&graph_root.0.join("main.omg"), Some(target))
    })
    .expect("renamed reconciled graph should check");
    assert_ne!(
        first_graph.custody.source_consumption_commitment(),
        renamed_graph.custody.source_consumption_commitment(),
        "requester-local dependency bindings must enter compiler-consumption identity even when unused"
    );
}

/// Substitute every independently representable ledger field exactly once.
/// Wire fields that cannot canonicalize reject at decode; record fields the
/// roster cannot hold reject at construction/recovery; every substitution a
/// corrupted record can still carry decodes to a different ledger whose
/// fingerprint diverges and whose replay against a real checked compilation
/// rejects.
///
/// The fixture is hand-encoded through the public row-recovery envelope and
/// the canonical dependency-closure constructor, so the codec gates run on
/// hosts that cannot compile a package-aware fixture. The replay leg joins a
/// real checked compilation whenever the host offers a native target profile.
#[test]
fn ordinary_package_obligation_ledger_rejects_every_one_field_substitution() {
    use package_compilation::PackageDependencyClosure;
    use package_evidence::ledger::OrdinaryPackageObligationLedger;

    let package = package_identity();
    let dependency = PackageKeyIdentity::from_digest([42; 32]).expect("dependency identity");
    let secondary = PackageKeyIdentity::from_digest([43; 32]).expect("secondary identity");
    let target = "linux_x86_64";

    // Raw row wire tags; the canonical-risk join requires these exact pairs.
    const KIND_PROJECTION_HEADER: u8 = 0;
    const KIND_PUBLIC_TRAIT: u8 = 1;
    const KIND_PUBLIC_DATA: u8 = 3;
    const KIND_SELECTED_PROVIDER_SET: u8 = 7;
    const KIND_PUBLIC_CONST: u8 = 12;
    const RISK_BLOCKING: u8 = 0;
    const RISK_OPAQUE_BLOCKING: u8 = 2;
    const DERIVATION_PROJECTION_HEADER: u8 = 0;
    const DERIVATION_EMPTY_PROVIDER_SET: u8 = 1;
    const ROLE_DECLARATION: u8 = 0;

    let canonical_rows = [
        crafted_canonical_row(
            package,
            target,
            KIND_PROJECTION_HEADER,
            RISK_BLOCKING,
            b"header",
            b"header-value",
        ),
        crafted_canonical_row(
            package,
            target,
            KIND_PUBLIC_DATA,
            RISK_BLOCKING,
            b"data/token",
            b"token-value",
        ),
        crafted_canonical_row(
            package,
            target,
            KIND_SELECTED_PROVIDER_SET,
            RISK_OPAQUE_BLOCKING,
            b"providers",
            b"provider-set",
        ),
    ];
    let envelopes = [
        crafted_row_envelope(&canonical_rows[0], &[DERIVATION_PROJECTION_HEADER]),
        crafted_authored_row_envelope(
            &canonical_rows[1],
            package,
            "main.omg",
            0,
            12,
            ROLE_DECLARATION,
        ),
        crafted_row_envelope(&canonical_rows[2], &[DERIVATION_EMPTY_PROVIDER_SET]),
    ];
    let decoded_rows = envelopes
        .iter()
        .map(|bytes| {
            decode_package_review_canonical_row(bytes)
                .expect("crafted row recovery envelope should decode")
        })
        .collect::<Vec<_>>();

    let closure = PackageDependencyClosure::from_canonical_parts(
        package,
        BuildDeclarationKind::Package,
        vec![package, dependency, secondary],
        vec![
            PackageDependencyBinding::new(package, "alpha_dep", dependency),
            PackageDependencyBinding::new(package, "beta_dep", secondary),
        ],
    )
    .expect("crafted dependency closure should validate");
    let ledger = recover_ordinary_package_obligation_ledger(closure.clone(), &decoded_rows)
        .expect("crafted rows should form a canonical ledger");
    let encoded =
        encode_ordinary_package_obligation_ledger(&ledger).expect("crafted ledger should encode");
    assert_eq!(
        decode_ordinary_package_obligation_ledger(&encoded).expect("crafted ledger should decode"),
        ledger
    );
    let fingerprint =
        ordinary_package_obligation_ledger_fingerprint(&ledger).expect("base fingerprint");

    let spans = ledger_field_spans(&encoded);
    assert_eq!(spans.packages.len(), 3);
    assert_eq!(spans.dependencies.len(), 2);
    assert_eq!(spans.rows.len(), canonical_rows.len());

    // The schema identity is deliberately not independently representable:
    // construction always seals the current vocabulary and decode rejects any
    // other version before a record can form, which is itself asserted below.

    let mut decodable_substitutions: Vec<(&'static str, OrdinaryPackageObligationLedger)> =
        Vec::new();
    let divergent = |name: &'static str, bytes: &[u8]| {
        let mutated = decode_ordinary_package_obligation_ledger(bytes)
            .unwrap_or_else(|error| panic!("{name} should still decode: {}", error.message()));
        assert_ne!(mutated, ledger, "{name} must change the recovered record");
        assert_eq!(
            encode_ordinary_package_obligation_ledger(&mutated)
                .expect("substituted ledger should re-encode"),
            bytes,
            "{name} must stay canonical once decoded"
        );
        let mutated_fingerprint = ordinary_package_obligation_ledger_fingerprint(&mutated)
            .expect("substituted ledger fingerprint");
        assert_ne!(
            mutated_fingerprint, fingerprint,
            "{name} must diverge the whole-ledger fingerprint"
        );
        mutated
    };

    // --- framing and vocabulary fields reject at decode ---

    let mut mutated = encoded.clone();
    mutated[spans.magic.start] ^= 0xFF;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("framing magic substitution must reject");
    assert!(error.message().contains("invalid framing magic"));

    let mut mutated = encoded.clone();
    mutated[spans.encoding_version.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("ledger encoding-version substitution must reject");
    assert!(error.message().contains("encoding version"));

    let mut mutated = encoded.clone();
    mutated[spans.schema_version.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("obligation-schema substitution must reject");
    assert!(
        error
            .message()
            .contains("unsupported ordinary package obligation schema")
    );

    for range in [spans.review_version.clone(), spans.row_version.clone()] {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let error = decode_ordinary_package_obligation_ledger(&mutated)
            .expect_err("review vocabulary substitution must reject");
        assert!(
            error
                .message()
                .contains("unsupported review-row vocabulary")
        );
    }

    // --- subject coordinates ---

    let mut mutated = encoded.clone();
    mutated[spans.package.clone()].copy_from_slice(&[0; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a zero package identity must reject");
    assert!(error.message().contains("invalid package identity"));

    let mut mutated = encoded.clone();
    mutated[spans.package.clone()].copy_from_slice(&[88; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a foreign package identity must reject");
    assert!(error.message().contains("different package identity"));

    let splice = |fill: &[u8]| -> Vec<u8> {
        let mut mutated = encoded.clone();
        mutated[spans.target_length.clone()]
            .copy_from_slice(&(u64::try_from(fill.len()).unwrap()).to_le_bytes());
        mutated.splice(spans.target.clone(), fill.iter().copied());
        mutated
    };
    let error = decode_ordinary_package_obligation_ledger(&splice(b"windows_x86_64"))
        .expect_err("a foreign target must reject");
    assert!(error.message().contains("different target"));
    let error = decode_ordinary_package_obligation_ledger(&splice(b"linux-x86-64"))
        .expect_err("a noncanonical target name must reject");
    assert!(error.message().contains("noncanonical target"));
    let error = decode_ordinary_package_obligation_ledger(&splice(&[0xFF, 0xFE]))
        .expect_err("non-UTF-8 target bytes must reject");
    assert!(error.message().contains("invalid UTF-8"));

    let mut mutated = encoded.clone();
    mutated[spans.target_length.clone()].copy_from_slice(&4097u64.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling target length must reject");
    assert!(error.message().contains("byte field exceeds its ceiling"));

    // --- dependency-closure fields ---

    let mut mutated = encoded.clone();
    mutated[spans.closure_root.clone()].copy_from_slice(&[77; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a closure root outside the roster must reject");
    assert!(
        error
            .message()
            .contains("does not contain its root package")
    );

    let mut mutated = encoded.clone();
    mutated[spans.root_role] = 9;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an unknown root-role tag must reject");
    assert!(error.message().contains("invalid root-role tag"));

    let mut mutated = encoded.clone();
    mutated[spans.root_role] = 2;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a workspace root role must reject");
    assert!(error.message().contains("workspace role"));

    // The package-role bit is independently representable: it decodes to a
    // different record, diverges the fingerprint, and replay rejects it.
    let mut mutated = encoded.clone();
    mutated[spans.root_role] = 1;
    decodable_substitutions.push((
        "application root role",
        divergent("application root role", &mutated),
    ));

    let mut mutated = encoded.clone();
    mutated[spans.package_count.clone()].copy_from_slice(&65_537u64.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling package count must reject");
    assert!(
        error
            .message()
            .contains("package count exceeds its ceiling")
    );

    // A duplicated roster identity breaks strict ordering.
    let mut mutated = encoded.clone();
    mutated[spans.packages[1].clone()].copy_from_slice(&package.digest());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a duplicated closure package must reject");
    assert!(error.message().contains("strict canonical order"));

    // A swapped roster order breaks strict ordering.
    let mut mutated = encoded.clone();
    let moved = mutated[spans.packages[2].clone()].to_vec();
    let first = mutated[spans.packages[0].clone()].to_vec();
    mutated[spans.packages[0].clone()].copy_from_slice(&moved);
    mutated[spans.packages[2].clone()].copy_from_slice(&first);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a reordered closure roster must reject");
    assert!(error.message().contains("strict canonical order"));

    // A dropped roster package leaves its edge open.
    let mut mutated = encoded.clone();
    mutated[spans.package_count.clone()].copy_from_slice(&2u64.to_le_bytes());
    let tail = mutated[spans.packages[2].end..].to_vec();
    mutated.truncate(spans.packages[2].start);
    mutated.extend_from_slice(&tail);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a dropped closure package must reject");
    assert!(error.message().contains("open edge"));

    let mut mutated = encoded.clone();
    mutated[spans.dependency_count.clone()].copy_from_slice(&262_145u64.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling dependency count must reject");
    assert!(
        error
            .message()
            .contains("dependency count exceeds its ceiling")
    );

    // A dropped edge leaves its target unreachable.
    let mut mutated = encoded.clone();
    mutated[spans.dependency_count.clone()].copy_from_slice(&1u64.to_le_bytes());
    let tail = mutated[spans.dependencies[1].requester.start..].to_vec();
    mutated.truncate(spans.dependencies[0].requester.start);
    mutated.extend_from_slice(&tail);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a dropped dependency edge must reject");
    assert!(error.message().contains("unreachable package"));

    // An edge requester outside the roster opens the edge.
    let mut mutated = encoded.clone();
    mutated[spans.dependencies[0].requester.clone()].copy_from_slice(&[77; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an edge requester outside the roster must reject");
    assert!(error.message().contains("open edge"));

    // A noncanonical alias is a representable-but-invalid name.
    let mut mutated = encoded.clone();
    mutated[spans.dependencies[0].alias.clone()].copy_from_slice(b"Alpha_dep");
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a noncanonical dependency alias must reject");
    assert!(error.message().contains("noncanonical alias"));

    // An over-ceiling alias length rejects at the length check.
    let mut mutated = encoded.clone();
    mutated[spans.dependencies[0].alias_length.clone()]
        .copy_from_slice(&(1024 * 1024 + 1u64).to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling alias length must reject");
    assert!(error.message().contains("byte field exceeds its ceiling"));

    // An edge target outside the roster opens the edge.
    let mut mutated = encoded.clone();
    mutated[spans.dependencies[0].target.clone()].copy_from_slice(&[77; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an edge target outside the roster must reject");
    assert!(error.message().contains("open edge"));

    // A same-length snake_case alias rename that keeps edge order is
    // representable: it decodes to a different closure, diverges the
    // fingerprint, and replay rejects it.
    let mut renamed_alias = encoded.clone();
    renamed_alias[spans.dependencies[1].alias.clone()].copy_from_slice(b"zeta_dep");
    decodable_substitutions.push((
        "dependency alias",
        divergent("dependency alias", &renamed_alias),
    ));

    // Swapped edge targets stay closed and reachable: representable, divergent,
    // and rejected by replay.
    let mut swapped_targets = encoded.clone();
    let alpha_target = swapped_targets[spans.dependencies[0].target.clone()].to_vec();
    let beta_target = swapped_targets[spans.dependencies[1].target.clone()].to_vec();
    swapped_targets[spans.dependencies[0].target.clone()].copy_from_slice(&beta_target);
    swapped_targets[spans.dependencies[1].target.clone()].copy_from_slice(&alpha_target);
    decodable_substitutions.push((
        "swapped edge targets",
        divergent("swapped edge targets", &swapped_targets),
    ));

    // Reordered edge frames break strict (requester, alias) ordering.
    let edge_frames_start = spans.dependencies[0].requester.start;
    let edge_frames_mid = spans.dependencies[1].requester.start;
    let edge_frames_end = spans.row_count.start;
    let first_edge = encoded[edge_frames_start..edge_frames_mid].to_vec();
    let second_edge = encoded[edge_frames_mid..edge_frames_end].to_vec();
    let mut mutated = encoded.clone();
    mutated.splice(
        edge_frames_start..edge_frames_end,
        second_edge.iter().chain(first_edge.iter()).copied(),
    );
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("reordered dependency edges must reject");
    assert!(error.message().contains("strict canonical order"));

    // A duplicated edge frame breaks strict ordering.
    let mut mutated = encoded.clone();
    mutated[spans.dependency_count.clone()].copy_from_slice(&3u64.to_le_bytes());
    mutated.splice(edge_frames_mid..edge_frames_mid, first_edge.iter().copied());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a duplicated dependency edge must reject");
    assert!(error.message().contains("strict canonical order"));

    // A cyclic closure rejects: drop the leaf package and bend its edge back
    // onto the root through the surviving dependency.
    let mut cyclic = encoded.clone();
    cyclic[spans.package_count.clone()].copy_from_slice(&2u64.to_le_bytes());
    let tail = cyclic[spans.packages[2].end..].to_vec();
    cyclic.truncate(spans.packages[2].start);
    cyclic.extend_from_slice(&tail);
    let cyclic_spans = ledger_field_spans(&cyclic);
    cyclic[cyclic_spans.dependencies[1].requester.clone()].copy_from_slice(&dependency.digest());
    cyclic[cyclic_spans.dependencies[1].target.clone()].copy_from_slice(&package.digest());
    let error = decode_ordinary_package_obligation_ledger(&cyclic)
        .expect_err("a cyclic dependency closure must reject");
    assert!(error.message().contains("cycle"));

    // --- row roster and row frames ---

    // An empty roster is a representable shape that cannot form a ledger.
    let mut mutated = encoded.clone();
    mutated[spans.row_count.clone()].copy_from_slice(&0u64.to_le_bytes());
    mutated.truncate(spans.rows[0].start);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a ledger with no rows must reject");
    assert!(error.message().contains("has no rows"));

    let mut mutated = encoded.clone();
    mutated[spans.row_count.clone()].copy_from_slice(&65_537u64.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling row count must reject");
    assert!(error.message().contains("row count exceeds its ceiling"));

    // A dropped required row cannot even decode: without its projection header
    // or selected-provider set the roster cannot finish a ledger.
    let mut mutated = encoded.clone();
    mutated[spans.row_count.clone()].copy_from_slice(&2u64.to_le_bytes());
    let tail = mutated[spans.rows[0].end..].to_vec();
    mutated.truncate(spans.rows[0].start);
    mutated.extend_from_slice(&tail);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("dropping the projection-header row must reject");
    assert!(error.message().contains("no projection header"));

    let mut mutated = encoded.clone();
    mutated[spans.row_count.clone()].copy_from_slice(&2u64.to_le_bytes());
    let tail = mutated[spans.rows[2].end..].to_vec();
    mutated.truncate(spans.rows[2].start);
    mutated.extend_from_slice(&tail);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("dropping the selected-provider-set row must reject");
    assert!(error.message().contains("no selected-provider set"));

    // A dropped ordinary row is representable: the smaller roster still
    // decodes, diverges the fingerprint, and replay rejects it.
    let mut dropped_row = encoded.clone();
    dropped_row[spans.row_count.clone()].copy_from_slice(&2u64.to_le_bytes());
    let tail = dropped_row[spans.rows[1].end..].to_vec();
    dropped_row.truncate(spans.rows[1].start);
    dropped_row.extend_from_slice(&tail);
    decodable_substitutions.push(("dropped row", divergent("dropped row", &dropped_row)));

    // A duplicated row frame collides on the strict (kind, key) coordinate.
    let row_frame = encoded[spans.rows[1].clone()].to_vec();
    let mut mutated = encoded.clone();
    mutated[spans.row_count.clone()].copy_from_slice(&4u64.to_le_bytes());
    mutated.splice(
        spans.rows[2].start..spans.rows[2].start,
        row_frame.iter().copied(),
    );
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a duplicated row must reject");
    assert!(error.message().contains("strict canonical order"));

    // A swapped row order breaks strict ordering.
    let mut mutated = encoded.clone();
    let first_row = mutated[spans.rows[1].clone()].to_vec();
    let second_row = mutated[spans.rows[2].clone()].to_vec();
    mutated.splice(
        spans.rows[1].start..spans.rows[2].end,
        second_row.iter().chain(first_row.iter()).copied(),
    );
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("reordered rows must reject");
    assert!(error.message().contains("strict canonical order"));

    // An over-ceiling row-frame length rejects at the length check.
    let mut mutated = encoded.clone();
    mutated[spans.rows[0].start..spans.rows[0].start + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an over-ceiling row length must reject");
    assert!(error.message().contains("byte field exceeds its ceiling"));

    // An inflated trailing row-frame length rejects as truncated.
    let mut mutated = encoded.clone();
    let last_frame_length = spans.rows[2].len() - 8;
    mutated[spans.rows[2].start..spans.rows[2].start + 8]
        .copy_from_slice(&(u64::try_from(last_frame_length).unwrap() + 1).to_le_bytes());
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an inflated trailing row length must reject");
    assert!(error.message().contains("truncated"));

    // --- fields inside one row's canonical frame ---

    let row_content_start = spans.rows[1].start + 8;
    let inner = canonical_row_spans(&encoded[row_content_start..spans.rows[1].end]);
    let absolute = |range: &std::ops::Range<usize>| {
        (range.start + row_content_start)..(range.end + row_content_start)
    };

    let mut mutated = encoded.clone();
    mutated[absolute(&inner.magic).start] ^= 0xFF;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a corrupt row magic must reject");
    assert!(error.message().contains("invalid canonical row"));

    for range in [inner.row_version.clone(), inner.review_version.clone()] {
        let mut mutated = encoded.clone();
        mutated[absolute(&range)].copy_from_slice(&u16::MAX.to_le_bytes());
        let error = decode_ordinary_package_obligation_ledger(&mutated)
            .expect_err("a row vocabulary substitution must reject");
        assert!(error.message().contains("invalid canonical row"));
    }

    // The row's own package identity joins against the ledger subject in both
    // directions: mutating the row digest rejects like mutating the ledger's.
    let mut mutated = encoded.clone();
    mutated[absolute(&inner.package)].copy_from_slice(&[88; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a row naming a foreign package must reject");
    assert!(error.message().contains("different package identity"));

    let mut mutated = encoded.clone();
    mutated[absolute(&inner.package)].copy_from_slice(&[0; 32]);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a row naming a zero package must reject");
    assert!(error.message().contains("invalid canonical row"));

    // The row's target joins against the ledger subject; a valid foreign
    // target rejects through the join while a noncanonical one rejects inside
    // the row framing.
    let splice_inner_target = |fill: &[u8]| -> Vec<u8> {
        let mut content = encoded[row_content_start..spans.rows[1].end].to_vec();
        content[inner.target.start - 8..inner.target.start]
            .copy_from_slice(&(u64::try_from(fill.len()).unwrap()).to_le_bytes());
        content.splice(inner.target.clone(), fill.iter().copied());
        let mut mutated = encoded.clone();
        mutated[spans.rows[1].start..spans.rows[1].start + 8]
            .copy_from_slice(&(u64::try_from(content.len()).unwrap()).to_le_bytes());
        mutated.splice(
            spans.rows[1].start + 8..spans.rows[1].end,
            content.iter().copied(),
        );
        mutated
    };
    let error = decode_ordinary_package_obligation_ledger(&splice_inner_target(b"windows_x86_64"))
        .expect_err("a row naming a foreign target must reject");
    assert!(error.message().contains("different target"));
    let error = decode_ordinary_package_obligation_ledger(&splice_inner_target(b"bad-target"))
        .expect_err("a row naming a noncanonical target must reject");
    assert!(error.message().contains("invalid canonical row"));

    // Unknown kind and noncanonical risk tags reject inside the row framing.
    let mut mutated = encoded.clone();
    mutated[row_content_start + inner.kind] = 99;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an unknown row kind must reject");
    assert!(error.message().contains("invalid canonical row"));

    let mut mutated = encoded.clone();
    mutated[row_content_start + inner.risk] = 9;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("an unknown row risk must reject");
    assert!(error.message().contains("invalid canonical row"));

    let mut mutated = encoded.clone();
    mutated[row_content_start + inner.risk] = RISK_OPAQUE_BLOCKING;
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a noncanonical kind/risk pair must reject");
    assert!(error.message().contains("invalid canonical row"));

    // A kind substitution that keeps canonical risk and roster order is
    // representable: it decodes to a different record and replay rejects it.
    let mut rekinded = encoded.clone();
    rekinded[row_content_start + inner.kind] = KIND_PUBLIC_TRAIT;
    decodable_substitutions.push(("row kind", divergent("row kind", &rekinded)));

    // A key-byte substitution keeps framing and order but changes identity.
    let mut rekeyed = encoded.clone();
    rekeyed[absolute(&inner.key).start] = b'X';
    decodable_substitutions.push(("row key", divergent("row key", &rekeyed)));

    // A value-byte substitution is opaque to framing but enters the retained
    // canonical bytes, so the fingerprint still binds it.
    let mut revalued = encoded.clone();
    revalued[absolute(&inner.value).start] ^= 0x01;
    decodable_substitutions.push(("row value", divergent("row value", &revalued)));

    // A truncated envelope and trailing bytes reject at the frame bounds.
    let mut mutated = encoded.clone();
    mutated.pop();
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("a truncated ledger must reject");
    assert!(error.message().contains("truncated"));

    let mut mutated = encoded.clone();
    mutated.push(0);
    let error = decode_ordinary_package_obligation_ledger(&mutated)
        .expect_err("trailing ledger bytes must reject");
    assert!(error.message().contains("trailing bytes"));

    // --- the recovered-record roster rejects the same one-field edits ---

    let reject_recovery =
        |rows: &[package_evidence::encoding::DecodedPackageReviewCanonicalRow], needle: &str| {
            let error = recover_ordinary_package_obligation_ledger(closure.clone(), rows)
                .expect_err("one-field roster substitution must reject");
            assert!(
                error.message().contains(needle),
                "expected `{needle}`, got `{}`",
                error.message()
            );
        };

    reject_recovery(&[], "has no rows");
    reject_recovery(&decoded_rows[1..], "no projection header");
    reject_recovery(&decoded_rows[..2], "no selected-provider set");

    let mut reordered = decoded_rows.clone();
    reordered.swap(0, 1);
    reject_recovery(&reordered, "strict canonical order");

    let mut duplicated = decoded_rows.clone();
    duplicated.insert(2, decoded_rows[1].clone());
    reject_recovery(&duplicated, "strict canonical order");

    // A row carrying a foreign package or a foreign target cannot mix into the
    // roster even though its framing is canonical.
    let foreign_package_row = decode_package_review_canonical_row(&crafted_row_envelope(
        &crafted_canonical_row(
            PackageKeyIdentity::from_digest([77; 32]).expect("foreign identity"),
            target,
            KIND_PUBLIC_CONST,
            RISK_BLOCKING,
            b"foreign",
            b"v",
        ),
        &[DERIVATION_PROJECTION_HEADER],
    ))
    .expect("foreign-package row should decode");
    let mut mixed = decoded_rows.clone();
    mixed.push(foreign_package_row);
    reject_recovery(&mixed, "mixes package identities");

    let foreign_target_row = decode_package_review_canonical_row(&crafted_row_envelope(
        &crafted_canonical_row(
            package,
            "windows_x86_64",
            KIND_PUBLIC_CONST,
            RISK_BLOCKING,
            b"foreign",
            b"v",
        ),
        &[DERIVATION_PROJECTION_HEADER],
    ))
    .expect("foreign-target row should decode");
    let mut mixed = decoded_rows.clone();
    mixed.push(foreign_target_row);
    reject_recovery(&mixed, "mixes targets");

    // A closure whose root differs from the roster's package rejects even when
    // the closure itself is well-formed.
    let foreign_root = PackageDependencyClosure::from_canonical_parts(
        PackageKeyIdentity::from_digest([77; 32]).expect("foreign identity"),
        BuildDeclarationKind::Package,
        vec![PackageKeyIdentity::from_digest([77; 32]).expect("foreign identity")],
        Vec::new(),
    )
    .expect("foreign-root closure should validate");
    let error = recover_ordinary_package_obligation_ledger(foreign_root, &decoded_rows)
        .expect_err("a closure rooted elsewhere must reject");
    assert!(error.message().contains("different root package"));

    // A forged row admitted through framing is representable: the extended
    // roster still finishes a ledger whose fingerprint diverges and whose
    // replay rejects.
    let forged_row = decode_package_review_canonical_row(&crafted_row_envelope(
        &crafted_canonical_row(
            package,
            target,
            KIND_PUBLIC_CONST,
            RISK_BLOCKING,
            b"extra",
            b"v",
        ),
        &[DERIVATION_PROJECTION_HEADER],
    ))
    .expect("forged row should decode");
    let mut forged_rows = decoded_rows.clone();
    forged_rows.push(forged_row);
    let forged = recover_ordinary_package_obligation_ledger(closure.clone(), &forged_rows)
        .expect("a forged row still forms a structurally valid ledger");
    assert_ne!(
        ordinary_package_obligation_ledger_fingerprint(&forged).expect("forged ledger fingerprint"),
        fingerprint,
        "a forged row must diverge the whole-ledger fingerprint"
    );
    decodable_substitutions.push(("forged row", forged));

    // --- replay: a real checked compilation rejects every carried record ---

    if let Some(host_target) = host_target_name() {
        let fixture_package = TempPackage::new();
        fixture_package.write("main.omg", "pub data Token { value: u64; }\n");
        fixture_package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&fixture_package.0)),
            ..CheckedCompileRequest::new(&fixture_package.0.join("main.omg"), Some(host_target))
        })
        .expect("replay fixture should check");
        let honest_rows = project_checked_package_review(&checked)
            .expect("replay review should project")
            .canonical_rows()
            .expect("replay review rows");
        let honest = ordinary_package_obligation_ledger_from_compiler_rows(
            checked
                .custody
                .dependency_closure()
                .cloned()
                .expect("replay compilation retains its closure"),
            &honest_rows,
        )
        .expect("replay ledger should form");
        validate_ordinary_package_obligation_ledger(&honest, &checked)
            .expect("the honest ledger should replay");

        // The crafted ledger is well-formed yet was never issued by this
        // compilation; replay must reject it like every substitution.
        assert!(
            validate_ordinary_package_obligation_ledger(&ledger, &checked).is_err(),
            "a foreign well-formed ledger must not settle"
        );
        for (name, mutated) in &decodable_substitutions {
            assert!(
                validate_ordinary_package_obligation_ledger(mutated, &checked).is_err(),
                "{name} must reject at local reconstruction replay"
            );
        }

        // A wire-level row drop on the real encoding decodes to a different
        // ledger whose replay rejects in the other direction.
        let honest_bytes =
            encode_ordinary_package_obligation_ledger(&honest).expect("honest encode");
        let honest_spans = ledger_field_spans(&honest_bytes);
        let dropped = honest_rows
            .iter()
            .position(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
            .expect("replay fixture should produce a public-data row");
        let mut mutated = honest_bytes.clone();
        mutated[honest_spans.row_count.clone()]
            .copy_from_slice(&(u64::try_from(honest_spans.rows.len()).unwrap() - 1).to_le_bytes());
        let tail = mutated[honest_spans.rows[dropped].end..].to_vec();
        mutated.truncate(honest_spans.rows[dropped].start);
        mutated.extend_from_slice(&tail);
        let smaller = decode_ordinary_package_obligation_ledger(&mutated)
            .expect("a dropped non-required row still decodes");
        assert_ne!(
            ordinary_package_obligation_ledger_fingerprint(&smaller)
                .expect("dropped-row fingerprint"),
            ordinary_package_obligation_ledger_fingerprint(&honest).expect("honest fingerprint")
        );
        assert!(
            validate_ordinary_package_obligation_ledger(&smaller, &checked).is_err(),
            "a dropped real row must reject at local reconstruction replay"
        );
    }
}
