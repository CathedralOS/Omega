mod support;

use support::*;

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
    let checked =
        compile_to_checked_with_packages(&application.0.join("main.omg"), Some(target), inputs)
            .expect("application root should check");
    let rows = project_checked_package_review(&checked)
        .expect("application review")
        .canonical_rows()
        .expect("application rows");
    let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
        checked
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
    let original_checked = compile_to_checked_with_packages(
        &original.0.join("main.omg"),
        Some(target),
        package_inputs(&original.0),
    )
    .expect("ordinary package obligation fixture should check");
    let projection = project_checked_package_review(&original_checked)
        .expect("ordinary package obligations should project");
    let rows = projection
        .canonical_rows()
        .expect("ordinary package obligation rows");
    let dependency_closure = original_checked
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
    let changed_checked = compile_to_checked_with_packages(
        &changed.0.join("main.omg"),
        Some(target),
        package_inputs(&changed.0),
    )
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
        compile_to_checked_with_packages(
            &root_path.join("main.omg"),
            Some(target),
            graph_inputs(root_path, dependency_path, alias),
        )
        .expect("unused dependency graph should check")
    };
    let ledger_for = |checked: &compiler::CheckedCompilation| {
        let rows = project_checked_package_review(checked)
            .expect("dependency-closure review should project")
            .canonical_rows()
            .expect("dependency-closure canonical rows");
        ordinary_package_obligation_ledger_from_compiler_rows(
            checked
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

    let without_dependency = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some(target),
        package_inputs(&root.0),
    )
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
    let first_checked = compile_to_checked_with_packages(
        &first.0.join("main.omg"),
        Some(target),
        package_inputs(&first.0),
    )
    .expect("first package source should check");
    let first_commitment = first_checked
        .source_consumption_commitment()
        .expect("package-aware compilation must retain source consumption");
    assert_ne!(first_commitment.digest(), [0; 32]);
    first_checked
        .verify_current_source_consumption()
        .expect("unchanged loaded source should verify");

    let relocated = TempPackage::new();
    relocated.write("main.omg", source);
    relocated.write("build.omg", build);
    let relocated_checked = compile_to_checked_with_packages(
        &relocated.0.join("main.omg"),
        Some(target),
        package_inputs(&relocated.0),
    )
    .expect("relocated package source should check");
    assert_eq!(
        first_commitment,
        relocated_checked
            .source_consumption_commitment()
            .expect("relocated package source commitment"),
        "absolute cache location and source-id assignment are not source identity"
    );

    let changed = TempPackage::new();
    changed.write("main.omg", changed_source);
    changed.write("build.omg", build);
    let changed_checked = compile_to_checked_with_packages(
        &changed.0.join("main.omg"),
        Some(target),
        package_inputs(&changed.0),
    )
    .expect("source-only changed package should check");
    assert_ne!(
        first_commitment,
        changed_checked
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
        first_checked.verify_current_source_consumption().is_err(),
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
    let first_graph = compile_to_checked_with_packages(
        &graph_root.0.join("main.omg"),
        Some(target),
        graph_inputs("dependency"),
    )
    .expect("first reconciled graph should check");
    let renamed_graph = compile_to_checked_with_packages(
        &graph_root.0.join("main.omg"),
        Some(target),
        graph_inputs("renamed_dependency"),
    )
    .expect("renamed reconciled graph should check");
    assert_ne!(
        first_graph.source_consumption_commitment(),
        renamed_graph.source_consumption_commitment(),
        "requester-local dependency bindings must enter compiler-consumption identity even when unused"
    );
}
