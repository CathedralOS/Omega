use omega_compiler::{
    BuildObservationClass, CheckedCompilation, PACKAGE_REVIEW_ENCODING_VERSION,
    PACKAGE_REVIEW_ROW_ENCODING_VERSION, PackageCompilationInputs, PackageDependencyBinding,
    PackageReviewArithmeticDomain, PackageReviewByteSequencePredicate, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCastForm, PackageReviewCheckedServiceReach, PackageReviewConformanceSubject,
    PackageReviewContractBinaryOperator, PackageReviewContractExpression,
    PackageReviewContractFact, PackageReviewContractKind, PackageReviewContractOperatorMeaning,
    PackageReviewContractStaticArgument, PackageReviewCrashInterface, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthorityClass, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainSemanticRole,
    PackageReviewExternalBinding, PackageReviewExternalRequirement,
    PackageReviewMachineParameterContract, PackageReviewNominalOwner,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPublicPropositionBody,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSynchronousInvocation, PackageReviewSyntheticSourceKind,
    PackageReviewTypeParameterKind, PackageSourceBinding, compile_to_checked_with_packages,
    decode_ordinary_package_obligation_ledger, decode_package_review_canonical_row,
    encode_ordinary_package_obligation_ledger, encode_package_review_canonical_row,
    ordinary_package_obligation_ledger_fingerprint,
    ordinary_package_obligation_ledger_from_compiler_rows, project_checked_package_review,
    recover_ordinary_package_obligation_ledger, validate_ordinary_package_obligation_ledger,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const LEDGER_MAGIC: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER\0";

fn read_ledger_u64(bytes: &[u8], position: &mut usize) -> usize {
    let end = *position + 8;
    let value = u64::from_le_bytes(bytes[*position..end].try_into().unwrap());
    *position = end;
    usize::try_from(value).unwrap()
}

fn ledger_target_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let mut position = LEDGER_MAGIC.len() + 4 * std::mem::size_of::<u16>() + 32;
    let length = read_ledger_u64(bytes, &mut position);
    position..position + length
}

fn ledger_closure_package_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let target = ledger_target_range(bytes);
    let mut position = target.end + 32;
    let count = read_ledger_u64(bytes, &mut position);
    position..position + count * 32
}

fn ledger_row_frames(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let packages = ledger_closure_package_range(bytes);
    let mut position = packages.end;
    let dependencies = read_ledger_u64(bytes, &mut position);
    for _ in 0..dependencies {
        position += 32;
        let alias_length = read_ledger_u64(bytes, &mut position);
        position += alias_length + 32;
    }
    let rows = read_ledger_u64(bytes, &mut position);
    (0..rows)
        .map(|_| {
            let start = position;
            let length = read_ledger_u64(bytes, &mut position);
            position += length;
            start..position
        })
        .collect()
}

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-review-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

fn public_quotient_source(
    carrier: &str,
    relation: &str,
    evidence: &str,
    reverse_relation: bool,
) -> String {
    let (
        relation_body,
        symmetric_requires,
        symmetric_ensures,
        transitive_requires,
        transitive_ensures,
    ) = if reverse_relation {
        ("b == a", "b == a", "a == b", "b == a\n    c == b", "c == a")
    } else {
        ("a == b", "a == b", "b == a", "a == b\n    b == c", "a == c")
    };
    format!(
        r#"use omega::language::core::relation;

pub data {carrier} {{
    case Zero;
    case Next(previous: {carrier});
}}

pub proposition {relation}(a: {carrier}, b: {carrier}) = {relation_body};

machine equivalent_reflexive(a: {carrier})
ensures a == a
{{
}}

machine equivalent_symmetric(a: {carrier}, b: {carrier})
requires {symmetric_requires}
ensures {symmetric_ensures}
{{
}}

machine equivalent_transitive(a: {carrier}, b: {carrier}, c: {carrier})
requires
    {transitive_requires}
ensures {transitive_ensures}
{{
}}

{evidence}: satisfies Equivalence<{carrier}, {relation}> {{
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}}

pub data EquivalenceClass = {carrier} % {relation}
where {relation} satisfies
    Equivalence<{carrier}, {relation}>
    as {evidence};
"#,
    )
}

#[test]
fn ordinary_package_obligation_ledger_requires_exact_local_reconstruction() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let original = TempPackage::new();
    original.write("main.omg", "pub data Token { value: u64; }\n");
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
    assert_eq!(target_range.len(), b"linux_arm64".len());
    changed_target[target_range].copy_from_slice(b"linux_arm64");
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
    changed.write("main.omg", "pub data Token { value: i64; }\n");
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
        PackageCompilationInputs::new(
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
    let ledger_for = |checked: &omega_compiler::CheckedCompilation| {
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
    let wrong_root_closure = PackageCompilationInputs::new(
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    assert_eq!(first_locations.len(), 1);
    assert_eq!(changed_locations.len(), 1);
    assert_eq!(first_locations[0].relative_path(), "main.omg");
    assert_eq!(changed_locations[0].relative_path(), "main.omg");
    assert!(changed_locations[0].start_byte() > first_locations[0].start_byte());
    assert!(
        !first_locations[0]
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
        PackageCompilationInputs::new(
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

#[test]
fn canonical_row_sorting_keeps_exact_declaration_sources_paired() {
    let Some(target) = host_target_name() else {
        return;
    };
    let source = "pub data Zed {}\npub data Alpha {}\n";
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("out-of-source-order declarations should check");
    let review = project_checked_package_review(&checked).expect("package review should close");
    let canonical_rows = review.canonical_rows().expect("canonical review rows");
    let data_rows = canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .collect::<Vec<_>>();
    assert_eq!(review.public_data().len(), 2);
    assert_eq!(data_rows.len(), 2);
    for row in data_rows {
        let [location] = row
            .source()
            .authored_locations()
            .expect("public data declaration source")
        else {
            panic!("one exact declaration source")
        };
        let start = usize::try_from(location.start_byte()).expect("source start fits usize");
        let end = usize::try_from(location.end_byte()).expect("source end fits usize");
        let declaration_name = &source[start..end];
        assert!(matches!(declaration_name, "Alpha" | "Zed"));
        assert!(
            row.key_bytes()
                .windows(declaration_name.len())
                .any(|window| window == declaration_name.as_bytes()),
            "the canonical row key and retained source must name the same exact declaration"
        );
    }
}

#[test]
fn callable_review_sources_join_all_authored_checked_body_call_forms() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = r#"machine make() -> u64 { 1u64 }
machine consume(value: u64) { }
pub machine api() {
    consume(make());
    transition { _ -> done() }

    state done() { }
}
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked body-call source fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked body-call sources should join package review");
    let api = review
        .canonical_rows()
        .expect("body-call canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("api".len())
                    .any(|window| window == b"api")
        })
        .expect("public api callable row");
    let mut body_call_text = api
        .source()
        .authored_locations()
        .expect("public api declaration and body calls")
        .iter()
        .filter(|location| location.role() == PackageReviewSourceLocationRole::BodyCall)
        .map(|location| {
            let start = usize::try_from(location.start_byte()).unwrap();
            let end = usize::try_from(location.end_byte()).unwrap();
            source[start..end].to_owned()
        })
        .collect::<Vec<_>>();
    body_call_text.sort();
    assert!(body_call_text.iter().any(|text| text == "consume"));
    assert!(body_call_text.iter().any(|text| text == "make"));
    assert!(body_call_text.iter().any(|text| text == "done"));
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(&api).expect("encode body-call source row"),
    )
    .expect("recover body-call source row");
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| locations
                .iter()
                .any(|location| { location.role() == PackageReviewSourceLocationRole::BodyCall }))
    );
}

#[test]
fn carried_transitive_types_project_exact_package_qualified_dependency_rows() {
    let Some(target) = host_target_name() else {
        return;
    };
    let root = TempPackage::new();
    let middle = TempPackage::new();
    let leaf = TempPackage::new();
    root.write(
        "main.omg",
        "use middle::middle;\nmachine relay() { consume(make()); }\n",
    );
    root.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    middle.write(
        "middle.omg",
        r#"use leaf::leaf;
pub machine make() -> Token { Token { value: 7u64 } }
pub machine consume(value: Token) {}
"#,
    );
    leaf.write("leaf.omg", "pub data Token { value: u64; }\n");

    let root_identity = package_identity();
    let middle_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("middle package identity");
    let leaf_identity = PackageKeyIdentity::from_digest([43; 32]).expect("leaf package identity");
    let inputs = PackageCompilationInputs::new(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "root", root.0.clone()),
            PackageSourceBinding::new(middle_identity, "middle", middle.0.clone()),
            PackageSourceBinding::new(leaf_identity, "leaf", leaf.0.clone()),
        ],
        vec![
            PackageDependencyBinding::new(root_identity, "middle", middle_identity),
            PackageDependencyBinding::new(middle_identity, "leaf", leaf_identity),
        ],
    )
    .expect("transitive package graph should validate");
    let checked = compile_to_checked_with_packages(&root.0.join("main.omg"), Some(target), inputs)
        .expect("carried transitive type should check without direct leaf authority");
    let review = project_checked_package_review(&checked).expect("semantic dependency review");

    for kind in [
        PackageReviewSemanticDependencyKind::NominalIdentity,
        PackageReviewSemanticDependencyKind::Layout,
        PackageReviewSemanticDependencyKind::OwnershipBehavior,
    ] {
        let dependency = review
            .semantic_dependencies()
            .iter()
            .find(|dependency| {
                dependency.consumer().path() == "relay"
                    && dependency.dependency().path() == "Token"
                    && dependency.dependency().owner()
                        == PackageReviewNominalOwner::Package(leaf_identity)
                    && dependency.exposure()
                        == PackageReviewSemanticDependencyExposure::PrivateImplementation
                    && dependency.kind() == kind
            })
            .unwrap_or_else(|| panic!("missing leaf-owned {kind:?} row"));
        assert_eq!(
            dependency.consumer().owner(),
            PackageReviewNominalOwner::Package(root_identity)
        );
    }

    let canonical = review
        .canonical_rows()
        .expect("canonical semantic dependency rows");
    let rows = canonical
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::SemanticDependency)
        .filter(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role()
                        == PackageReviewSourceLocationRole::SemanticDependencyDeclaration
                        && location.owner()
                            == PackageReviewSourceLocationOwner::Package(leaf_identity)
                        && location.relative_path() == "leaf.omg"
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(rows.len() >= 3);
    for row in rows {
        assert_eq!(row.risk(), PackageReviewCanonicalRowRisk::Blocking);
        let locations = row
            .source()
            .authored_locations()
            .expect("semantic dependency row source");
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::SemanticDependencyConsumer
                && location.owner() == PackageReviewSourceLocationOwner::Package(root_identity)
                && location.relative_path() == "main.omg"
        }));
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::SemanticDependencyDeclaration
                && location.owner() == PackageReviewSourceLocationOwner::Package(leaf_identity)
                && location.relative_path() == "leaf.omg"
        }));
    }
}

#[test]
fn semantic_dependency_projection_rejects_retained_evidence_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token { value: u64; }
pub machine make() -> Token { Token { value: 7u64 } }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("semantic dependency evidence fixture should check");
    assert!(
        !checked.facts.flow.semantic_dependencies.rows.is_empty(),
        "fixture must carry at least one semantic dependency"
    );
    project_checked_package_review(&checked).expect("unaltered retained evidence should rederive");

    let mut missing = checked.clone();
    missing.facts.flow.semantic_dependencies.rows.pop();
    assert_semantic_dependency_rederivation_rejects(&missing, "missing row");

    let mut duplicate = checked.clone();
    let duplicated_row = duplicate.facts.flow.semantic_dependencies.rows[0];
    duplicate
        .facts
        .flow
        .semantic_dependencies
        .rows
        .push(duplicated_row);
    assert_semantic_dependency_rederivation_rejects(&duplicate, "duplicate row");

    let mut reordered = checked.clone();
    assert!(
        reordered.facts.flow.semantic_dependencies.rows.len() >= 2,
        "fixture must carry enough rows to test canonical ordering"
    );
    reordered.facts.flow.semantic_dependencies.rows.swap(0, 1);
    assert_semantic_dependency_rederivation_rejects(&reordered, "reordered rows");

    let mut altered = checked;
    let row = &mut altered.facts.flow.semantic_dependencies.rows[0];
    row.exposure = match row.exposure {
        psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
            psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface
        }
        psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
            psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation
        }
    };
    assert_semantic_dependency_rederivation_rejects(&altered, "altered row");
}

fn assert_semantic_dependency_rederivation_rejects(
    checked: &omega_compiler::CheckedCompilation,
    mutation: &str,
) {
    let diagnostics = match project_checked_package_review(checked) {
        Err(diagnostics) => diagnostics,
        Ok(_) => panic!("{mutation} should reject retained semantic-dependency drift"),
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "retained checked semantic-dependency evidence does not equal compiler rederivation",
        )
    }));
}

#[test]
fn dangerous_authority_classification_requires_exact_toolchain_provenance() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

pub data Journal { files: FilesystemHost; written: i64; }

pub machine Journal::append(&mut self, descriptor: i32, bytes: &[u8])
reaches FilesystemHost
invokes FilesystemHost;
{
    self.written = self.files.write(descriptor, bytes);
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical filesystem fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical filesystem review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical filesystem authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Filesystem
    );
    let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
        panic!("canonical filesystem authority must retain exact toolchain source")
    };
    assert_ne!(source.digest(), [0; 32]);
    assert_eq!(authority.service().path(), "FilesystemHost");
    assert!(canonical_review.dangerous_authority_slack().is_empty());
    let rows = canonical_review
        .canonical_rows()
        .expect("filesystem review rows");
    let authority_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("dangerous authority canonical row");
    let locations = authority_row
        .source()
        .authored_locations()
        .expect("dangerous authority row must retain authored provenance");
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Toolchain(_)
            )
    }));
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Package(_)
            )
            && location.relative_path() == "main.omg"
    }));
    assert!(locations.iter().all(|location| {
        !location
            .relative_path()
            .contains(canonical.0.to_string_lossy().as_ref())
    }));

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait FilesystemHost {
    machine write(descriptor: i32, bytes: &[u8]) -> i64;
}

pub data Journal { files: FilesystemHost; written: i64; }

pub machine Journal::append(&mut self, descriptor: i32, bytes: &[u8])
reaches FilesystemHost
invokes FilesystemHost;
{
    self.written = self.files.write(descriptor, bytes);
}
"#,
    );
    lookalike.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let lookalike_checked = compile_to_checked_with_packages(
        &lookalike.0.join("main.omg"),
        Some(target),
        package_inputs(&lookalike.0),
    )
    .expect("package-owned filesystem lookalike should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned filesystem lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled naming must not mint a compiler-owned risk class"
    );
}

#[test]
fn process_authority_classification_requires_exact_toolchain_console() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::std::console;

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical console fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical console review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical process authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
        panic!("canonical process authority must retain exact toolchain source")
    };
    assert_ne!(source.digest(), [0; 32]);
    assert_eq!(authority.service().path(), "Console");
    assert!(canonical_review.dangerous_authority_slack().is_empty());
    let rows = canonical_review
        .canonical_rows()
        .expect("process review rows");
    let authority_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("dangerous process authority canonical row");
    assert_eq!(
        authority_row.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    let locations = authority_row
        .source()
        .authored_locations()
        .expect("dangerous process authority row must retain authored provenance");
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Toolchain(_)
            )
            && location.relative_path() == "console.omg"
    }));
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Package(_)
            )
            && location.relative_path() == "main.omg"
    }));

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait Console {
    machine exit_process(return_code: i32);
}

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
}
"#,
    );
    lookalike.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let lookalike_checked = compile_to_checked_with_packages(
        &lookalike.0.join("main.omg"),
        Some(target),
        package_inputs(&lookalike.0),
    )
    .expect("package-owned console lookalike should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned console lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled Console naming must not mint process authority"
    );
}

#[test]
fn empty_boundary_body_is_checked_callable_and_remains_directly_invocable() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

boundary machine adapter() reaches FilesystemHost { }

pub machine caller() reaches FilesystemHost {
    adapter();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("an explicit empty boundary body remains executable");
    let review =
        project_checked_package_review(&checked).expect("empty boundary body review should close");
    let adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "adapter")
        .expect("adapter review row");
    assert!(matches!(
        adapter.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(review.dangerous_authority_slack().iter().any(|slack| {
        slack.class() == PackageReviewDangerousAuthorityClass::Filesystem
            && slack.callable().path() == "adapter"
    }));
}

#[test]
fn package_review_rejects_impossible_supply_body_combinations() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write("main.omg", "pub machine api() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary package should check");

    let mut missing_body = checked.clone();
    missing_body
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine")
        .body_is_present = false;
    let diagnostics = project_checked_package_review(&missing_body)
        .expect_err("checked supply without a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("classified as checked supply but has no retained body")
    }));

    let mut bodyful_accepted = checked;
    let api = bodyful_accepted
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine");
    api.supply_mode = psi_language_semantics::MachineSupplyMode::Accepted;
    let diagnostics = project_checked_package_review(&bodyful_accepted)
        .expect_err("bodyless supply with a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has bodyless supply but retains a body")
    }));
}

#[test]
fn review_projects_every_external_executable_supply_mechanism_as_opaque_blocking() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine imported() reaches ExternalSurface;
    machine syscalled() reaches ExternalSurface;
    machine intrinsic() reaches ExternalSurface;
    machine slot() reaches ExternalSurface;
    machine field() reaches ExternalSurface;
    machine table() reaches ExternalSurface;
}

pub data DispatchTable {
    dispatch: addr;
    invoke: addr;
}

pub machine import_leaf()
    satisfies ExternalSurface::imported
    via Binding::DllImport("libomega", "omega_entry");
pub machine syscall_leaf()
    satisfies ExternalSurface::syscalled
    via Binding::Syscall(61);
machine intrinsic_leaf()
    satisfies ExternalSurface::intrinsic
    via Binding::CompilerIntrinsic;
pub machine slot_leaf()
    satisfies ExternalSurface::slot
    via Binding::VtableSlot(7);
pub machine DispatchTable::field_leaf()
    satisfies ExternalSurface::field
    via Binding::VtableField(dispatch);
pub machine DispatchTable::table_leaf()
    satisfies ExternalSurface::table
    via Binding::TableFunction(invoke);
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external executable-supply fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("external executable-supply review should close");

    let expected = [
        (
            "import_leaf",
            PackageReviewExternalBinding::Import {
                library: "libomega".to_owned(),
                symbol: "omega_entry".to_owned(),
            },
        ),
        (
            "syscall_leaf",
            PackageReviewExternalBinding::Syscall { number: 61 },
        ),
        (
            "intrinsic_leaf",
            PackageReviewExternalBinding::CompilerIntrinsic,
        ),
        (
            "slot_leaf",
            PackageReviewExternalBinding::VtableSlot { index: 7 },
        ),
        (
            "DispatchTable::field_leaf",
            PackageReviewExternalBinding::VtableField {
                field: "dispatch".to_owned(),
            },
        ),
        (
            "DispatchTable::table_leaf",
            PackageReviewExternalBinding::TableFunction {
                field: "invoke".to_owned(),
            },
        ),
    ];
    let expected_count = expected.len();
    assert_eq!(review.external_executable_supply().len(), expected_count);
    for (callable, binding) in expected {
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable)
            .unwrap_or_else(|| panic!("missing external supply for {callable}"));
        assert_eq!(supply.binding(), &binding);
        assert_eq!(
            supply
                .conformance()
                .expect("trait-bound external supply")
                .trait_identity()
                .path(),
            "ExternalSurface"
        );
        let callable_row = review
            .callables()
            .iter()
            .find(|candidate| candidate.identity() == supply.callable());
        if callable == "intrinsic_leaf" {
            assert!(
                callable_row.is_none(),
                "a private external leaf must not become public callable API"
            );
        } else {
            assert!(callable_row.is_some_and(|candidate| {
                candidate.supply() == PackageReviewCallableSupply::ExternalRealization
            }));
        }
    }

    let rows = review
        .canonical_rows()
        .expect("canonical external-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), expected_count);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                })
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
}

#[test]
fn review_joins_external_boundary_operator_supply_without_implying_visibility() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
pub boundary operator F32::maximum(left: f32, right: f32) -> f32;

pub data FloatProvider {}
pub machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
machine FloatProvider::maximum(left: f32, right: f32) -> f32
    satisfies F32::maximum
    via Binding::CompilerIntrinsic;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external boundary-operator fixture should check and select exact intrinsics");
    let review = project_checked_package_review(&checked)
        .expect("external boundary-operator supply should project exactly");

    assert_eq!(review.external_executable_supply().len(), 2);
    for requirement in ["minimum", "maximum"] {
        let callable_path = format!("FloatProvider::{requirement}");
        let declaration = review
            .public_operators()
            .iter()
            .find(|operator| {
                operator.coordinate().identity().path() == format!("F32::{requirement}")
            })
            .unwrap_or_else(|| panic!("missing public operator {requirement}"));
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable_path)
            .unwrap_or_else(|| panic!("missing external supply for {callable_path}"));
        assert!(matches!(
            supply.requirement(),
            PackageReviewExternalRequirement::Operator(operator)
                if operator == declaration.coordinate()
        ));
        assert_eq!(supply.operator(), Some(declaration.coordinate()));
        assert_eq!(supply.conformance(), None);
        assert_eq!(
            supply.binding(),
            &PackageReviewExternalBinding::CompilerIntrinsic
        );

        let selected = review
            .selected_providers()
            .iter()
            .find(|provider| provider.schema_declaration() == declaration.coordinate().identity())
            .unwrap_or_else(|| panic!("missing selected provider for {requirement}"));
        let [selected_row] = selected.row_declarations() else {
            panic!("one selected realization for {requirement}")
        };
        assert_eq!(selected_row.realization(), supply.callable());
        assert_eq!(
            selected_row.requirement().owner(),
            declaration.coordinate().identity().owner()
        );
        assert!(matches!(
            selected.rows()[0].binding,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ));
    }

    let public_callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "FloatProvider::minimum")
        .expect("public external operator leaf should remain public callable API");
    assert_eq!(
        public_callable.supply(),
        PackageReviewCallableSupply::ExternalRealization
    );
    assert_eq!(public_callable.operator_realizations().len(), 1);
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "FloatProvider::maximum"),
        "private external operator leaf must not become public callable API"
    );

    let rows = review
        .canonical_rows()
        .expect("canonical external operator-supply rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), 2);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                })
            })
    }));
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("external operator-supply recovery envelope should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("external operator-supply recovery envelope should decode");
        assert_eq!(
            decoded.kind(),
            PackageReviewCanonicalRowKind::ExternalExecutableSupply
        );
        assert_eq!(
            decoded.risk(),
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        );
        assert_eq!(decoded.key_bytes(), row.key_bytes());
    }
}

#[test]
fn external_boundary_operator_overloads_keep_distinct_requirement_coordinates() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Float {}
pub boundary operator Float::add(left: f32, right: f32) -> f32;
pub boundary operator Float::add(left: f64, right: f64) -> f64;

pub data F32Provider {}
pub machine F32Provider::add(left: f32, right: f32) -> f32
    satisfies Float::add
    via Binding::CompilerIntrinsic;
pub data F64Provider {}
pub machine F64Provider::add(left: f64, right: f64) -> f64
    satisfies Float::add
    via Binding::CompilerIntrinsic;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external boundary-operator overloads should select independently");
    let review = project_checked_package_review(&checked)
        .expect("external boundary-operator overloads should project exactly");
    let overloads = review
        .public_operators()
        .iter()
        .filter(|operator| operator.coordinate().identity().path() == "Float::add")
        .collect::<Vec<_>>();
    assert_eq!(overloads.len(), 2);
    assert_ne!(
        overloads[0].coordinate().parameter_dispatch(),
        overloads[1].coordinate().parameter_dispatch()
    );

    for (callable, primitive) in [("F32Provider::add", "f32"), ("F64Provider::add", "f64")] {
        let supply = review
            .external_executable_supply()
            .iter()
            .find(|supply| supply.callable().path() == callable)
            .unwrap_or_else(|| panic!("missing external supply for {callable}"));
        let operator = supply.operator().expect("operator requirement");
        assert!(operator.parameter_dispatch().contains(primitive));
        assert!(
            overloads
                .iter()
                .any(|declaration| declaration.coordinate() == operator)
        );
        let callable_row = review
            .callables()
            .iter()
            .find(|candidate| candidate.identity() == supply.callable())
            .expect("public external leaf callable");
        assert_eq!(callable_row.operator_realizations(), [operator.clone()]);
    }
}

#[test]
fn unsupported_external_boundary_operator_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let cases = [
        (
            "private-operator",
            r#"data F32 {}
boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "realizes non-public operator",
        ),
        (
            "aliased",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum as Selected
    via Binding::CompilerIntrinsic;
"#,
            "through an alias not yet represented",
        ),
        (
            "fixed-token",
            r#"pub data Float {}
pub boundary operator + Float::add(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::add(left: f32, right: f32) -> f32
    satisfies Float::add
    via Binding::CompilerIntrinsic;
"#,
            "before external token dispatch is represented",
        ),
        (
            "generic-machine",
            r#"pub data F32 {}
pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
data FloatProvider {}
machine FloatProvider::minimum<T>(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
"#,
            "generic or lifetime-parameterized boundary operator",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported external operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }
}

#[test]
fn external_binding_changes_only_the_supply_row_for_a_stable_callable() {
    let Some(target) = host_target_name() else {
        return;
    };
    let project = |number: i64| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub boundary trait ExternalSurface {{
    machine invoke() reaches ExternalSurface;
}}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall({number});
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("external syscall fixture should check");
        project_checked_package_review(&checked)
            .expect("external syscall package review should close")
            .canonical_rows()
            .expect("external syscall canonical rows")
    };

    let old = project(60);
    let new = project(61);
    let old_callable = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("old callable row");
    let new_callable = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("new callable row");
    assert_eq!(old_callable.key_bytes(), new_callable.key_bytes());
    assert_eq!(
        old_callable.canonical_bytes(),
        new_callable.canonical_bytes()
    );

    let old_supply = old
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("old external-supply row");
    let new_supply = new
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("new external-supply row");
    assert_eq!(old_supply.key_bytes(), new_supply.key_bytes());
    assert_ne!(old_supply.canonical_bytes(), new_supply.canonical_bytes());
}

#[test]
fn external_executable_supply_projection_rejects_inconsistent_checked_state() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait ExternalSurface {
    machine invoke() reaches ExternalSurface;
}
pub machine invoke_leaf()
    satisfies ExternalSurface::invoke
    via Binding::Syscall(60);
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("external tamper fixture should check");

    fn replace_external_binding(
        checked: &mut CheckedCompilation,
        identity: psi_language_semantics::ExternalBindingIdentity,
    ) {
        let mechanism = identity.mechanism();
        let binding = checked.typed.external_bindings.intern(identity);
        let leaf = checked
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.name.as_str() == "invoke_leaf")
            .expect("external leaf");
        let satisfies = leaf.satisfies;
        leaf.supply_mode =
            psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism };
        checked
            .typed
            .machine_trait_conformances
            .span_mut_or_empty(satisfies)[0]
            .external_binding = Some(binding);
    }

    let mut mechanism_mismatch = checked.clone();
    let leaf = mechanism_mismatch
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, .. } =
        leaf.supply_mode
    else {
        panic!("external leaf supply")
    };
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Import,
    };
    let diagnostics = project_checked_package_review(&mechanism_mismatch)
        .expect_err("mechanism mismatch must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("supply mechanism inconsistent with its exact binding identity")
    }));

    let mut missing_conformance_binding = checked.clone();
    let satisfies = missing_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    missing_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = None;
    let diagnostics = project_checked_package_review(&missing_conformance_binding)
        .expect_err("missing conformance binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance without its exact external binding")
    }));

    let mut missing_binding_identity = checked.clone();
    let invalid_binding = psi_language_semantics::ExternalBindingId(u32::MAX);
    let leaf = missing_binding_identity
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf");
    let satisfies = leaf.satisfies;
    leaf.supply_mode = psi_language_semantics::MachineSupplyMode::ExternalRealization {
        binding: invalid_binding,
        mechanism: psi_language_semantics::ExternalBindingMechanism::Syscall,
    };
    missing_binding_identity
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(invalid_binding);
    let diagnostics = project_checked_package_review(&missing_binding_identity)
        .expect_err("missing binding-table identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact binding-table identity")
    }));

    let mut bodyful_external = checked.clone();
    bodyful_external
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .body_is_present = true;
    let diagnostics = project_checked_package_review(&bodyful_external)
        .expect_err("bodyful external supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("retains an implementation body")
    }));

    let mut missing_conformance = checked.clone();
    missing_conformance
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies = Default::default();
    let diagnostics = project_checked_package_review(&missing_conformance)
        .expect_err("external supply without a conformance must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 0 conformance applications; expected exactly one")
    }));

    let mut duplicate_conformance = checked.clone();
    let leaf_index = duplicate_conformance
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf index");
    let duplicate = duplicate_conformance
        .typed
        .machine_trait_conformances(&duplicate_conformance.typed.machines()[leaf_index])[0]
        .clone();
    let machine_roots = duplicate_conformance.typed.roots.machines;
    let tables = &mut duplicate_conformance.typed.tables;
    let leaf = &mut tables.machines.span_mut_or_empty(machine_roots)[leaf_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut leaf.satisfies, duplicate);
    let diagnostics = project_checked_package_review(&duplicate_conformance)
        .expect_err("multiple external conformances must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has 2 conformance applications; expected exactly one")
    }));

    let mut mismatched_conformance_binding = checked.clone();
    let different_binding = mismatched_conformance_binding
        .typed
        .external_bindings
        .intern(psi_language_semantics::ExternalBindingIdentity::Syscall { number: 61 });
    let satisfies = mismatched_conformance_binding
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .satisfies;
    mismatched_conformance_binding
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .external_binding = Some(different_binding);
    let diagnostics = project_checked_package_review(&mismatched_conformance_binding)
        .expect_err("different valid conformance binding must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance binding inconsistent with its supply mode")
    }));

    let mut nonexternal_supply = checked.clone();
    nonexternal_supply
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "invoke_leaf")
        .expect("external leaf")
        .supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;
    let diagnostics = project_checked_package_review(&nonexternal_supply)
        .expect_err("external conformance binding on ordinary supply must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external conformance binding without external supply")
    }));

    let malformed = [
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: String::new(),
                symbol: "entry".to_owned(),
            },
            "has no exact import-library identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Import {
                library: "omega".to_owned(),
                symbol: String::new(),
            },
            "has no exact import-symbol identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::Syscall { number: -1 },
            "has a syscall number outside 0..=u32::MAX",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableSlot { index: -1 },
            "has a negative vtable-slot index",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::VtableField {
                field: String::new(),
            },
            "has no exact table-field identity",
        ),
        (
            psi_language_semantics::ExternalBindingIdentity::TableFunction {
                field: "invoke".to_owned(),
            },
            "has table-field supply without one exact attached provider data declaration",
        ),
    ];
    for (identity, expected) in malformed {
        let mut tampered = checked.clone();
        replace_external_binding(&mut tampered, identity);
        let diagnostics = project_checked_package_review(&tampered)
            .expect_err("malformed external binding payload must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic containing {expected:?}: {diagnostics:?}"
        );
    }
}

#[test]
fn dangerous_hardware_authorities_require_exact_toolchain_provenance() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::core::interrupt;
use omega::language::core::extent;

pub machine exercise_hardware()
reaches MachineControl + PortIo + InterruptMaskControl + InterruptEntry + ExtentRootProvider
{
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical hardware-authority fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical hardware-authority review should close");
    let classes = canonical_review
        .dangerous_authorities()
        .iter()
        .map(|authority| authority.class())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        classes,
        std::collections::BTreeSet::from([
            PackageReviewDangerousAuthorityClass::MachineControl,
            PackageReviewDangerousAuthorityClass::PortIo,
            PackageReviewDangerousAuthorityClass::InterruptControl,
            PackageReviewDangerousAuthorityClass::InterruptEntry,
            PackageReviewDangerousAuthorityClass::RootMemory,
        ])
    );
    assert!(
        canonical_review
            .dangerous_authorities()
            .iter()
            .all(|authority| matches!(
                authority.service().owner(),
                PackageReviewNominalOwner::ToolchainSource(_)
            ))
    );
    let hardware = canonical_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "exercise_hardware")
        .expect("hardware callable review");
    assert!(matches!(
        hardware.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    let slack_classes = canonical_review
        .dangerous_authority_slack()
        .iter()
        .map(|slack| slack.class())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(slack_classes, classes);
    assert!(
        canonical_review
            .dangerous_authority_slack()
            .iter()
            .all(|slack| {
                slack.callable().path() == "exercise_hardware"
                    && matches!(
                        slack.service().owner(),
                        PackageReviewNominalOwner::ToolchainSource(_)
                    )
            })
    );
    let slack_rows = canonical_review
        .canonical_rows()
        .expect("hardware canonical rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthoritySlack)
        .collect::<Vec<_>>();
    assert_eq!(slack_rows.len(), 5);
    assert!(slack_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::AuditRecommended
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
                }) && locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::AuthorityExposure
                })
            })
    }));

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait MachineControl {}
pub boundary trait PortIo {}
pub boundary trait InterruptMaskControl {}
pub boundary trait InterruptEntry {}
pub boundary trait ExtentRootProvider {}

pub machine exercise_hardware()
reaches MachineControl + PortIo + InterruptMaskControl + InterruptEntry + ExtentRootProvider
{
}
"#,
    );
    lookalike.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let lookalike_checked = compile_to_checked_with_packages(
        &lookalike.0.join("main.omg"),
        Some(target),
        package_inputs(&lookalike.0),
    )
    .expect("package-owned hardware lookalikes should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned hardware-lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled hardware names must not mint compiler-owned risk classes"
    );
    assert!(
        lookalike_review.dangerous_authority_slack().is_empty(),
        "package-controlled hardware names must not mint compiler-owned slack classes"
    );
}

#[test]
fn representation_tcb_retains_private_opaque_data_as_unbound() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary data InternalToken;\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("opaque representation fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("opaque representation review should close");
    let [row] = review.representation_tcb() else {
        panic!("one private representation-TCB row")
    };
    assert_eq!(row.declaration().path(), "InternalToken");
    assert_eq!(
        row.declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(row.abi(), PackageReviewRepresentationAbiCommitment::Unbound);
    assert_eq!(
        row.mechanism(),
        PackageReviewRepresentationMechanism::Unbound
    );
    assert!(
        review.public_data().is_empty(),
        "ordinary public API projection remains visibility-scoped"
    );

    let control = TempPackage::new();
    control.write("main.omg", "data InternalToken { }\n");
    control.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let control_checked = compile_to_checked_with_packages(
        &control.0.join("main.omg"),
        Some(target),
        package_inputs(&control.0),
    )
    .expect("ordinary private representation fixture should check");
    let control_review = project_checked_package_review(&control_checked)
        .expect("ordinary private representation review should close");
    assert!(control_review.public_data().is_empty());
    assert!(control_review.representation_tcb().is_empty());
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("opaque review encoding"),
        control_review
            .canonical_review_bytes()
            .expect("ordinary review encoding"),
        "a private opaque representation-TCB row must enter comparison identity"
    );
}

#[test]
fn review_projects_root_boundary_and_build_authority() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine host_ping() reaches <= Host;
boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::VtableSlot(1);
data Receipt [linear] { code: i32; }
pub data Packet [copy] { #1 value: u32; }
pub domain Packet::Ready;
domain Packet::Private;
data PrivatePacket { hidden: u32; }
machine helper()
crashes Abort
{
    crash Abort;
}
pub machine public_api() { }
machine private_api() { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build)
crashes Abort
{
    builder.package("review-fixture");
    helper();
    let receipt: Receipt = Receipt { code: 1 };
    crash Abort;
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("package fixture should check");
    let observations = checked
        .build_observation_summary()
        .expect("selected build machine publishes build observation evidence");
    assert_eq!(observations.ceiling(), BuildObservationClass::Hermetic);
    assert_eq!(observations.realized(), BuildObservationClass::Hermetic);
    let review = project_checked_package_review(&checked).expect("review projection should close");
    let encoded = review
        .canonical_review_bytes()
        .expect("review projection should have a canonical comparison encoding");
    let magic = b"OMEGA-PACKAGE-REVIEW\0";
    assert!(encoded.starts_with(magic));
    assert_eq!(
        &encoded[magic.len()..magic.len() + 2],
        &PACKAGE_REVIEW_ENCODING_VERSION.to_le_bytes(),
    );
    assert_eq!(
        encoded,
        review
            .canonical_review_bytes()
            .expect("repeated encoding must be deterministic")
    );
    let rows = review
        .canonical_rows()
        .expect("review projection should have canonical comparison rows");
    assert_eq!(
        rows,
        review
            .canonical_rows()
            .expect("repeated row encoding must be deterministic")
    );
    assert!(rows.windows(2).all(|pair| {
        (pair[0].kind(), pair[0].key_bytes()) < (pair[1].kind(), pair[1].key_bytes())
    }));
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
    );
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
    );
    let row_magic = b"OMEGA-PACKAGE-REVIEW-ROW\0";
    for row in &rows {
        assert!(row.canonical_bytes().starts_with(row_magic));
        assert_eq!(
            &row.canonical_bytes()[row_magic.len()..row_magic.len() + 2],
            &PACKAGE_REVIEW_ROW_ENCODING_VERSION.to_le_bytes()
        );
    }

    assert_eq!(review.package(), package_identity());
    assert_eq!(
        review.target().target_name(),
        target,
        "review identity must retain the deployment profile, not only its native ABI",
    );
    assert_eq!(PACKAGE_REVIEW_ENCODING_VERSION, 72);
    assert_eq!(PACKAGE_REVIEW_ROW_ENCODING_VERSION, 30);
    let [ready] = review.public_domains() else {
        panic!("one package-owned public domain row")
    };
    assert_eq!(ready.identity().path(), "Packet::Ready");
    assert!(ready.type_parameters().is_empty());
    assert!(!ready.target_type().canonical().is_empty());
    assert!(ready.index_arguments().is_empty());
    let [packet] = review.public_data() else {
        panic!("one package-owned public data row")
    };
    assert_eq!(packet.identity().path(), "Packet");
    assert_eq!(packet.lifetime_parameter_count(), 0);
    assert_eq!(packet.members().len(), 1);
    let PackageReviewDataMember::Field(value) = &packet.members()[0] else {
        panic!("Packet value field")
    };
    assert_eq!(value.identity(), Some(1));
    assert_eq!(value.name(), "value");
    assert!(!value.type_identity().canonical().is_empty());
    assert_eq!(review.callables().len(), 3);
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary row");
    assert_eq!(boundary.identity().path(), "host_ping");
    assert_eq!(boundary.lifetime_parameter_count(), 0);
    assert!(boundary.type_parameters().is_empty());
    assert!(boundary.parameters().is_empty());
    assert!(!boundary.return_type().canonical().is_empty());
    assert_eq!(
        boundary.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let [declared] = boundary
        .declared_service_reach()
        .expect("installation-bound declaration retains its upper bound")
    else {
        panic!("one declared upper-bound service")
    };
    assert_eq!(declared.path(), "Host");
    assert_eq!(
        declared.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(boundary.capability_flows().is_empty());
    assert_eq!(boundary.declared_synchronous_invocations(), Some(&[][..]));
    assert!(boundary.realized_synchronous_invocations().is_empty());
    let [installation] = boundary.unresolved_installation_reaches() else {
        panic!("one normalized installation-bound reach row")
    };
    assert_eq!(
        installation.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(installation.requirement().path().contains("host_ping"));
    let [upper_bound] = installation.upper_bound() else {
        panic!("one normalized installation upper-bound service")
    };
    assert_eq!(
        upper_bound.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(upper_bound.path(), "Host");

    let build = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Build)
        .expect("build row");
    assert_eq!(build.identity().path(), "build");
    let [builder] = build.parameters() else {
        panic!("build entry retains its builder parameter")
    };
    assert_eq!(builder.name(), "builder");
    assert!(builder.is_mutable());
    assert!(!builder.is_const());
    assert!(!builder.is_self());
    assert!(!builder.type_identity().canonical().is_empty());
    assert!(
        builder
            .type_identity()
            .canonical()
            .contains("toolchain-source-owner"),
        "source-backed Build must retain its exact toolchain source owner: {}",
        builder.type_identity().canonical(),
    );
    assert_eq!(build.declared_service_reach(), None);
    assert_eq!(build.declared_synchronous_invocations(), None);

    let public = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("ordinary public callable row");
    assert_eq!(public.identity().path(), "public_api");
    assert_eq!(
        public.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(public.declared_service_reach(), Some(&[][..]));
    assert_eq!(public.declared_synchronous_invocations(), Some(&[][..]));
    assert!(matches!(
        public.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(public.realized_synchronous_invocations().is_empty());
    assert_eq!(
        public.checked_crash().interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "private_api")
    );
    let crash = build.checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    let [published_crash] = crash.published() else {
        panic!("one normalized published crash route")
    };
    assert_eq!(
        published_crash.cause(),
        psi_checked_trees::CrashCause::Abort
    );
    assert_eq!(
        published_crash.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let [crash_site] = crash.checked_sites() else {
        panic!("one normalized checked crash site")
    };
    assert_eq!(
        crash_site.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(crash_site.guard_covering_buckets(), [1]);
    assert!(!crash_site.frontier_lower_bound().is_empty());
    assert!(
        crash_site
            .frontier_lower_bound()
            .iter()
            .all(|claim| claim.machine().owner()
                == PackageReviewNominalOwner::Package(package_identity())
                && claim.state().owner() == PackageReviewNominalOwner::Package(package_identity()))
    );
    let mut helper_calls = crash
        .checked_calls()
        .iter()
        .filter(|call| call.target_machine().path() == "helper");
    let crash_call = helper_calls
        .next()
        .expect("one normalized helper crash call");
    assert!(
        helper_calls.next().is_none(),
        "the helper crash route must remain unique"
    );
    assert_eq!(
        crash_call.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        crash_call.target_machine().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_call.target_machine().path(), "helper");
    assert_eq!(
        crash_call.target_state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let [provider] = review.selected_providers() else {
        panic!("one selected provider review row")
    };
    assert_eq!(provider.realizing_package(), Some(package_identity()));
    assert_eq!(provider.schema_declaration().path(), "Host");
    assert_eq!(
        provider.schema_declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(provider.provider_type_package(), None);
    assert_eq!(provider.provider_type_declaration(), None);
    assert_eq!(provider.service_schema(), "Host");
    assert_eq!(
        provider.schema().trait_package_identity,
        Some(package_identity())
    );
    assert_eq!(
        provider.schema().methods[0].requirement_owner_package_identity,
        Some(package_identity())
    );
    assert_eq!(provider.rows().len(), 1);
    let [provider_declarations] = provider.row_declarations() else {
        panic!("one exact requirement/realization declaration pair")
    };
    assert_eq!(
        provider_declarations.requirement().path(),
        provider.schema().methods[0].requirement_identity
    );
    assert_eq!(
        provider_declarations.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(provider_declarations.realization().path(), "ping_leaf");
    assert_eq!(
        provider_declarations.realization().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::VtableSlot { index: 1 }
    ));
    let provider_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("selected provider canonical row");
    assert_eq!(
        provider_row.source().compiler_derivations(),
        [
            PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection,
            PackageReviewSyntheticSourceKind::FreeExternalProviderType,
        ]
    );
    let provider_locations = provider_row
        .source()
        .authored_locations()
        .expect("implicit provider still retains authored schema and realization provenance");
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderSchemaDeclaration
            && location.relative_path() == "main.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderRequirementDeclaration
            && location.relative_path() == "main.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderRealization
            && location.relative_path() == "main.omg"
    }));
    assert!(!provider_locations.iter().any(|location| matches!(
        location.role(),
        PackageReviewSourceLocationRole::ProviderSelection
            | PackageReviewSourceLocationRole::ProviderTypeDeclaration
    )));
}

#[test]
fn review_projects_exact_accepted_boundary_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let zero_source = "boundary machine trusted_zero() -> u64\nensures result == 0;\n";
    let compile_claim = |value: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("boundary machine trusted_zero() -> u64\nensures result == {value};\n"),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("accepted boundary claim should check");
        project_checked_package_review(&checked).expect("accepted boundary contract review")
    };

    let zero = compile_claim(0);
    let boundary = zero
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary callable row");
    assert_eq!(
        boundary.supply(),
        PackageReviewCallableSupply::Accepted,
        "a bodyless boundary guarantee must remain an explicit trust-bearing accepted claim",
    );
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(zero.dangerous_authority_slack().is_empty());
    let [contract] = boundary.contracts() else {
        panic!("one exact accepted contract row")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Ensures);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        meaning,
        operator,
        left,
        right,
    }) = contract.fact()
    else {
        panic!("exact equality expression")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    assert_eq!(**left, PackageReviewContractExpression::Result);
    assert_eq!(
        **right,
        PackageReviewContractExpression::Integer("0".to_owned())
    );
    let zero_rows = zero.canonical_rows().expect("zero claim rows");
    let accepted_claims = zero_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .collect::<Vec<_>>();
    let [accepted_claim] = accepted_claims.as_slice() else {
        panic!("one explicit accepted-claim row")
    };
    assert_eq!(
        accepted_claim.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(
        accepted_claim
            .key_bytes()
            .windows("trusted_zero".len())
            .any(|window| window == b"trusted_zero")
    );
    let claim_locations = accepted_claim
        .source()
        .authored_locations()
        .expect("accepted claim declaration and contract source");
    assert!(claim_locations.iter().any(|location| {
        location.relative_path() == "main.omg"
            && location.role() == PackageReviewSourceLocationRole::Declaration
    }));
    assert!(claim_locations.iter().any(|location| {
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        location.relative_path() == "main.omg"
            && location.role() == PackageReviewSourceLocationRole::ContractClause
            && &zero_source[start..end] == "ensures"
    }));
    let recovered_claim = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(accepted_claim).expect("encode accepted claim row"),
    )
    .expect("recover accepted claim row");
    assert!(
        recovered_claim
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );

    let one = compile_claim(1);
    let one_rows = one.canonical_rows().expect("one claim rows");
    let one_claim = one_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .expect("changed accepted claim row");
    assert_ne!(
        accepted_claim.canonical_bytes(),
        one_claim.canonical_bytes(),
        "changing an accepted guarantee must change its trust-bearing row",
    );
    assert_ne!(
        zero.canonical_review_bytes().expect("zero claim encoding"),
        one.canonical_review_bytes().expect("one claim encoding"),
        "changing an accepted guarantee must change exact review evidence",
    );
}

#[test]
fn review_projects_exact_outcome_specific_guarantees() {
    let compile = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("outcome-specific package fixture should check")
    };
    let source = |selector: &str, groups: &str| {
        format!(
            r#"pub trait Evidence {{
    machine witness();
}}
pub proposition ready() evidence Evidence;
pub data Outcome {{ case Success; case Failure; }}
pub machine choose(flag: bool) -> Outcome
requires input_proof: ready()
ensures
{groups}
{{
    {selector} = input_proof;
    Outcome::Success
}}
"#,
        )
    };
    let ordered_groups = r#"    Outcome::Success -> {
        selected: ready();
        true;
    }
ensures
    Outcome::Failure -> {
        true;
    }"#;
    let reordered_groups = r#"    Outcome::Failure -> {
        true;
    }
ensures
    Outcome::Success -> {
        true;
        selected: ready();
    }"#;
    let moved_group = r#"    Outcome::Success -> {
        selected: ready();
    }
ensures
    Outcome::Failure -> {
        true;
        true;
    }"#;

    let checked = compile(&source("selected", ordered_groups));
    let review = project_checked_package_review(&checked)
        .expect("checked outcome-specific carriers should rejoin review rows");
    let choose = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("choose"))
        .expect("public choose callable");
    let guarded = choose
        .contracts()
        .iter()
        .filter(|contract| contract.result_case().is_some())
        .collect::<Vec<_>>();
    assert_eq!(guarded.len(), 3);
    assert!(
        guarded
            .iter()
            .all(|contract| contract.kind() == PackageReviewContractKind::Ensures)
    );
    assert!(guarded.iter().all(|contract| {
        contract
            .result_case()
            .is_some_and(|guard| guard.result_data().path() == "Outcome")
    }));
    let selected = guarded
        .iter()
        .find(|contract| contract.binding() == Some("selected"))
        .expect("named guarded guarantee");
    assert_eq!(selected.evidence_lane_position(), Some(0));
    assert!(
        selected
            .result_case()
            .is_some_and(|guard| guard.result_case().path().contains("Success"))
    );

    let reordered = project_checked_package_review(&compile(&source("selected", reordered_groups)))
        .expect("reordered guarded rows should project");
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "group and row ordering must not affect guarded contract identity",
    );

    let moved = project_checked_package_review(&compile(&source("selected", moved_group)))
        .expect("moved guarded fact should project");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        moved.canonical_review_bytes().unwrap(),
        "moving a guarantee to another result case must change review identity",
    );
    let renamed_source = source("approved", &ordered_groups.replace("selected", "approved"));
    let renamed = project_checked_package_review(&compile(&renamed_source))
        .expect("renamed guarded selector should project");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming a public guarded selector must change review identity",
    );

    let mut missing = checked.clone();
    missing.facts.proof.outcome_specific_guarantees.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("0 exact checked carrier rows; expected one")
    }));

    let mut duplicate = checked.clone();
    let duplicate_row = duplicate
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .next()
        .map(|(_, row)| row.clone())
        .expect("one guarded carrier to duplicate");
    duplicate
        .facts
        .proof
        .outcome_specific_guarantees
        .append(duplicate_row);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("2 exact checked carrier rows; expected one")
    }));

    let mut mismatched = checked;
    let carrier = mismatched
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("one guarded carrier to alter");
    mismatched
        .facts
        .proof
        .outcome_specific_guarantees
        .get_mut(carrier)
        .public_selector = Some("spoofed-selector".to_owned());
    let diagnostics = project_checked_package_review(&mismatched)
        .expect_err("mismatched guarded carriers must reject review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("0 exact checked carrier rows; expected one")
    }));
}

#[test]
fn claim_free_boundary_supply_does_not_collapse_into_an_accepted_claim() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary machine host_ping();\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("claim-free boundary fixture should check");
    let review =
        project_checked_package_review(&checked).expect("claim-free boundary review should close");
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "host_ping")
        .expect("claim-free boundary row");
    assert_eq!(boundary.supply(), PackageReviewCallableSupply::Boundary);
    assert!(boundary.contracts().is_empty());
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(review.dangerous_authority_slack().is_empty());
    assert!(
        review
            .canonical_rows()
            .expect("claim-free boundary rows")
            .iter()
            .all(|row| row.kind() != PackageReviewCanonicalRowKind::AcceptedClaim),
        "claim-free boundary supply must not emit accepted-claim evidence"
    );
}

#[test]
fn review_projects_exact_public_domain_membership_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u64::Trusted;

pub machine consume(value: u64)
requires value in u64::Trusted
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked public-domain membership requirement should check");
    let review = project_checked_package_review(&checked).expect("membership contract review");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one exact membership contract")
    };
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("exact membership row")
    };
    assert_eq!(*value, PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "u64::Trusted");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let membership_row = review
        .canonical_rows()
        .expect("membership canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("public consume callable row");
    assert!(
        membership_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"domain u64::Hidden;
pub machine consume(value: u64)
requires value in u64::Hidden
{ }
"#,
    );
    hidden.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `u64::Hidden`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_structural_propositions_and_alpha_normalizes_their_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub proposition equivalent<Element>(left: Element, right: Element);
pub machine compare<Value>(left: Value, right: Value)
requires equivalent<Value>(left, right)
{ }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub proposition equivalent<Item>(left: Item, right: Item);
pub machine compare<Compared>(left: Compared, right: Compared)
requires equivalent<Compared>(left, right)
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);

    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic proposition fixture should check");
        project_checked_package_review(&checked).expect("generic proposition review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("compare"))
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one proposition contract")
    };
    assert_eq!(contract.evidence_lane_position(), None);
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("exact proposition application")
    };
    assert_eq!(application.declaration().path(), "equivalent");
    let [binder] = application.binders() else {
        panic!("one proposition binder")
    };
    assert_eq!(binder.kind(), &PackageReviewPropositionBinderKind::Type);
    let [argument] = application.binder_arguments() else {
        panic!("one proposition binder argument")
    };
    assert_eq!(
        argument.value(),
        &PackageReviewPropositionBinderValue::GenericBinder(0)
    );
    assert_eq!(application.parameter_types().len(), 2);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(0),
            PackageReviewContractExpression::Parameter(1),
        ]
    );
    assert_eq!(
        application.evidence(),
        &PackageReviewPropositionEvidence::FactOnly
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming callable and proposition binders must not alter package evidence",
    );
}

#[test]
fn review_projects_unused_public_proposition_declarations_without_granting_facts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub proposition ready();
pub proposition reflexive(value: i32) = value == value;
proposition hidden();
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("public proposition declarations should check");
    assert!(
        checked
            .facts
            .proof
            .proposition_vocabulary
            .applications
            .is_empty(),
        "publishing a bodyless proposition declaration must not manufacture an application fact"
    );

    let review = project_checked_package_review(&checked).expect("public proposition review");
    assert_eq!(review.public_propositions().len(), 2);
    let ready = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "ready")
        .expect("unused public primitive proposition row");
    assert_eq!(ready.body(), &PackageReviewPublicPropositionBody::Primitive);
    let reflexive = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "reflexive")
        .expect("public transparent proposition row");
    assert!(matches!(
        reflexive.body(),
        PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary { .. }
        ))
    ));
    let proposition_rows = review
        .canonical_rows()
        .expect("canonical public proposition rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicProposition)
        .count();
    assert_eq!(
        proposition_rows, 2,
        "private propositions stay out of public API rows"
    );
}

#[test]
fn review_projects_unused_public_consts_with_exact_type_and_value_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let project = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public const declaration should check");
        project_checked_package_review(&checked).expect("public const review")
    };

    let original = project("pub const LIMIT: u64 = 4;\nconst HIDDEN_LIMIT: u64 = 2;\n");
    let changed_value = project("pub const LIMIT: u64 = 5;\n");
    let changed_type = project("pub const LIMIT: u32 = 4;\n");

    let [limit] = original.public_consts() else {
        panic!("private consts must stay out of public compatibility rows");
    };
    assert_eq!(limit.identity().path(), "LIMIT");
    assert!(limit.declared_type().canonical().contains("u64"));
    assert!(!limit.canonical_value_encoding().is_empty());
    let rows = original
        .canonical_rows()
        .expect("canonical public const rows");
    let const_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
        .collect::<Vec<_>>();
    assert_eq!(const_rows.len(), 1);
    assert_eq!(
        const_rows[0].risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(const_rows[0].source().authored_locations().is_some());
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a public const value must change package compatibility",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "changing a public const declared type must change package compatibility",
    );
}

#[test]
fn review_projects_unused_public_operator_overloads_and_exact_contract_meaning() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token [copy] { value: u64; }
pub operator < Token::less(left: Token, right: Token) -> bool;
pub operator Token::ordered(left: Token, right: Token) -> bool
ensures result == (left < right)
crashes Trap
    left < right
    left < right
crashes Abort;
operator Token::hidden(value: Token) -> bool;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("unused public operators should check");
    let review = project_checked_package_review(&checked)
        .expect("unused public operators should project directly from declarations");
    assert_eq!(review.public_operators().len(), 2);
    let less = review
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::less")
        .expect("fixed-token overload row");
    assert_eq!(
        less.spelling(),
        Some(psi_language_core::OperatorSpelling::Less)
    );
    assert_eq!(less.parameters().len(), 2);
    assert!(less.coordinate().result_dispatch().is_empty());
    assert!(less.published_crash().is_empty());

    let ordered = review
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::ordered")
        .expect("named operator row");
    let [contract] = ordered.contracts() else {
        panic!("one exact public operator contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("outer equality uses one compiler-owned builtin meaning")
    };
    let PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Declared(selected),
        operator: PackageReviewContractBinaryOperator::Less,
        ..
    } = right.as_ref()
    else {
        panic!("inner comparison retains one exact declared overload")
    };
    assert_eq!(selected, less.coordinate());
    let [trap, abort] = ordered.published_crash() else {
        panic!("one guarded Trap and one unconditional Abort bucket")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    let [
        PackageReviewCrashRouteGuard::Expression(PackageReviewContractExpression::Binary {
            meaning: PackageReviewContractOperatorMeaning::Declared(selected_crash_operator),
            operator: PackageReviewContractBinaryOperator::Less,
            ..
        }),
    ] = trap.alternative_guards()
    else {
        panic!("duplicate guarded routes canonicalize to one exact declared-operator expression")
    };
    assert_eq!(selected_crash_operator, less.coordinate());
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );

    let rows = review.canonical_rows().expect("public operator rows");
    let operator_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicOperator)
        .collect::<Vec<_>>();
    assert_eq!(operator_rows.len(), 2);
    assert!(operator_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::Blocking
            && row.source().authored_locations().is_some()
    }));

    let ordered_symbol = checked
        .operators()
        .iter()
        .find(|operator| operator.is_public && !operator.contracts.is_empty())
        .map(|operator| operator.symbol)
        .expect("checked ordered operator declaration");
    let owner = psi_checked_trees::ContractProofFactOwner::OperatorDeclaration {
        operator_symbol: ordered_symbol,
    };
    let (checked_contract_handle, checked_contract) = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .find(|(_, fact)| fact.owner == owner)
        .map(|(handle, fact)| (handle, fact.clone()))
        .expect("one checked operator-declaration contract row");

    let assert_owner_row_rejects = |checked: &CheckedCompilation, count: usize| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("malformed operator-declaration custody must reject review");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "contract fact has {count} checked owner rows; expected one"
            ))
        }));
    };

    let mut missing = checked.clone();
    assert!(
        missing
            .facts
            .proof
            .contract_facts
            .free(checked_contract_handle)
    );
    assert_owner_row_rejects(&missing, 0);

    let mut duplicate = checked.clone();
    duplicate
        .facts
        .proof
        .contract_facts
        .append(checked_contract.clone());
    assert_owner_row_rejects(&duplicate, 2);

    let mut wrong_owner = checked.clone();
    wrong_owner
        .facts
        .proof
        .contract_facts
        .get_mut(checked_contract_handle)
        .owner = psi_checked_trees::ContractProofFactOwner::Unknown;
    assert_owner_row_rejects(&wrong_owner, 0);
}

#[test]
fn public_operator_crash_routes_are_canonical_sensitive_and_checked() {
    let compile = |routes: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Token [copy] {{ value: u64; }}
pub operator / divide(left: Token, right: Token) -> Token
crashes Trap;
pub operator Token::checked(value: Token, flag: bool) -> bool
{routes};
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public operator crash fixture should check")
    };
    let project = |routes: &str| {
        project_checked_package_review(&compile(routes))
            .expect("public operator crash routes should project")
    };

    let original_routes = r#"crashes Trap
    flag
    flag
crashes Abort"#;
    let reordered_routes = r#"crashes Abort
crashes Trap
    flag"#;
    let original = project(original_routes);
    let reordered = project(reordered_routes);
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "clause order and duplicate guards must not change operator crash identity",
    );
    let checked_operator = original
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::checked")
        .expect("domain-homed checked operator");
    let [trap, abort] = checked_operator.published_crash() else {
        panic!("one guarded Trap and one unconditional Abort bucket")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    assert_eq!(
        trap.alternative_guards(),
        [PackageReviewCrashRouteGuard::Expression(
            PackageReviewContractExpression::Parameter(1)
        )]
    );
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let divide = original
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "divide")
        .expect("root fixed-token operator");
    assert_eq!(divide.published_crash().len(), 1);

    let changed_guard = project(
        r#"crashes Trap
    !flag
crashes Abort"#,
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_guard.canonical_review_bytes().unwrap(),
        "changing a guarded route must change operator review identity",
    );
    let changed_cause = project(
        r#"crashes Abort
    flag"#,
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_cause.canonical_review_bytes().unwrap(),
        "changing the crash cause must change operator review identity",
    );

    let operator_row = original
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicOperator
                && row
                    .key_bytes()
                    .windows("Token::checked".len())
                    .any(|window| window == "Token::checked".as_bytes())
        })
        .expect("checked operator canonical row");
    let encoded = encode_package_review_canonical_row(&operator_row).unwrap();
    let decoded = decode_package_review_canonical_row(&encoded).unwrap();
    assert_eq!(decoded.kind(), operator_row.kind());
    assert_eq!(decoded.risk(), operator_row.risk());
    assert_eq!(decoded.key_bytes(), operator_row.key_bytes());
    assert_eq!(decoded.canonical_bytes(), operator_row.canonical_bytes());
    assert_eq!(decoded.source(), operator_row.source());

    let checked = compile(original_routes);
    let mut missing = checked.clone();
    missing.facts.operators.operator_crash_contracts.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing checked operator crash rows must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-crash evidence does not equal compiler rederivation")
    }));

    let mut duplicate = checked;
    let duplicate_row = duplicate.facts.operators.operator_crash_contracts[0].clone();
    duplicate
        .facts
        .operators
        .operator_crash_contracts
        .push(duplicate_row);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate checked operator crash rows must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-crash evidence does not equal compiler rederivation")
    }));
}

#[test]
fn review_projects_named_witness_interfaces_through_transparent_aliases() {
    let Some(target) = host_target_name() else {
        return;
    };
    let direct = TempPackage::new();
    let aliased = TempPackage::new();
    let direct_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub machine consume()
requires proof: carries<i32>(1)
{ }
"#;
    let aliased_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub proposition forwarded<Item>(value: Item) = carries<Item>(value);
pub machine consume()
requires evidence: forwarded<i32>(1)
{ }
"#;
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    direct.write("main.omg", direct_source);
    direct.write("build.omg", build);
    aliased.write("main.omg", aliased_source);
    aliased.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named witness fixture should check")
    };
    let direct_checked = compile(&direct);
    let direct_review =
        project_checked_package_review(&direct_checked).expect("direct witness review");
    let aliased_review =
        project_checked_package_review(&compile(&aliased)).expect("aliased witness review");
    let consume = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public consumer");
    let [contract] = consume.contracts() else {
        panic!("one named witness contract")
    };
    let consume_row = direct_review
        .canonical_rows()
        .expect("named witness canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("named witness callable row");
    assert!(
        consume_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::ContractClause
                    && &direct_source[start..end] == "requires"
            }))
    );
    assert_eq!(
        contract.binding(),
        None,
        "a named requires spelling is a callee-local alias"
    );
    assert_eq!(contract.evidence_lane_position(), Some(0));
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("witness proposition application")
    };
    assert_eq!(application.declaration().path(), "carries");
    let [binder_argument] = application.binder_arguments() else {
        panic!("one witness proposition type argument")
    };
    let PackageReviewPropositionBinderValue::Type(type_identity) = binder_argument.value() else {
        panic!("concrete proposition type argument must use structural type identity")
    };
    assert!(type_identity.canonical().contains("compiler-type"));
    let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
        panic!("witness interface")
    };
    assert_eq!(interface.trait_identity().path(), "Evidence");
    assert_eq!(interface.arguments().len(), 1);
    assert_eq!(interface.requirements().len(), 2);
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "Evidence"
            && requirement.requirement().path().contains("witness")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "EvidenceBase"
            && requirement.requirement().path().contains("inherited")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert_ne!(
        direct_review
            .canonical_review_bytes()
            .expect("direct witness encoding"),
        aliased_review
            .canonical_review_bytes()
            .expect("aliased witness encoding"),
        "a published transparent alias is a distinct source API row even though contract semantic identity expands through it",
    );
    let direct_contract = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("direct public consumer")
        .contracts();
    let aliased_contract = aliased_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("aliased public consumer")
        .contracts();
    assert_eq!(
        direct_contract, aliased_contract,
        "transparent alias expansion must preserve the consuming contract's semantic row"
    );

    let mut diagnostic_spoof = compile(&direct);
    let term_handles = diagnostic_spoof
        .facts
        .proof
        .evidence_terms
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in term_handles {
        let term = diagnostic_spoof.facts.proof.evidence_terms.get_mut(handle);
        term.evidence_type = "spoofed diagnostic evidence".to_owned();
        term.proposition
            .arguments
            .fill("spoofed argument".to_owned());
        for argument in &mut term.proposition.binder_arguments {
            argument.identity = "spoofed binder".to_owned();
        }
        if let Some(interface) = term.evidence_interface.as_mut() {
            interface.arguments.fill("spoofed interface".to_owned());
            for requirement in &mut interface.requirements {
                requirement
                    .declaring_trait_arguments
                    .fill("spoofed requirement".to_owned());
            }
        }
    }
    let spoofed_review = project_checked_package_review(&diagnostic_spoof)
        .expect("diagnostic strings are not review identity");
    assert_eq!(
        direct_review
            .canonical_review_bytes()
            .expect("structural witness encoding"),
        spoofed_review
            .canonical_review_bytes()
            .expect("spoofed diagnostic witness encoding"),
        "checked diagnostic strings must not influence package evidence",
    );
}

#[test]
fn named_evidence_lane_order_changes_canonical_review_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let first = TempPackage::new();
    let second = TempPackage::new();
    let prefix = r#"pub trait Evidence {}
pub proposition left_fact() evidence Evidence;
pub proposition right_fact() evidence Evidence;
"#;
    first.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires left: left_fact()\nrequires right: right_fact()\n{{ }}\n"
        ),
    );
    second.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires right: right_fact()\nrequires left: left_fact()\n{{ }}\n"
        ),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);
    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named evidence lane fixture should check");
        project_checked_package_review(&checked)
            .expect("named evidence lane review")
            .canonical_review_bytes()
            .expect("named evidence lane encoding")
    };
    assert_ne!(
        encode(&first),
        encode(&second),
        "reordering positional erased proof inputs must change package evidence",
    );
}

#[test]
fn review_projects_proof_static_evidence_members_by_lane_and_requirement() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let source = |binding: &str| {
        format!(
            r#"pub trait EvidenceBase<Element> {{
    machine modulus() -> Element;
}}
pub trait Evidence<Element>: EvidenceBase<Element> {{
}}
pub proposition holds<Element>() evidence Evidence<Element>;
pub proposition selected<machine Witness>();
pub machine caller()
requires {binding}: holds<i32>()
requires selected<{binding}.modulus>()
{{ }}
"#
        )
    };
    original.write("main.omg", &source("proof"));
    renamed.write("main.omg", &source("evidence"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("proof-static projection fixture should check");
        project_checked_package_review(&checked).expect("proof-static projection review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let caller = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("caller"))
        .expect("public caller");
    let selected = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "selected").then_some(application)
        })
        .expect("selected proposition row");
    let holds = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "holds").then_some(application)
        })
        .expect("source witness proposition row");
    let [argument] = selected.binder_arguments() else {
        panic!("one projected static machine argument")
    };
    let PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind,
        source_lane_position,
        declaring_trait,
        declaring_trait_arguments,
        requirement,
    } = argument.value()
    else {
        panic!("exact proof-static evidence projection")
    };
    assert_eq!(*source_kind, PackageReviewContractKind::Requires);
    assert_eq!(*source_lane_position, 0);
    assert_eq!(declaring_trait.path(), "EvidenceBase");
    assert!(requirement.path().contains("modulus"));
    let PackageReviewPropositionEvidence::Witness(source_interface) = holds.evidence() else {
        panic!("source witness interface")
    };
    let source_requirement = source_interface
        .requirements()
        .iter()
        .find(|candidate| candidate.requirement() == requirement)
        .expect("inherited source requirement");
    assert_eq!(
        declaring_trait_arguments,
        source_requirement.declaring_trait_arguments(),
        "the projection must retain the exact inherited requirement template anchored by the source lane",
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original proof-static encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed proof-static encoding"),
        "renaming the local evidence term must not alter its lane-based package identity",
    );
}

#[test]
fn review_projects_exact_concrete_machine_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected: &str| {
        format!(
            r#"pub machine chosen(value: u64) -> u64 {{ value }}
pub machine alternate(value: u64) -> u64 {{ value }}
pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
boundary machine trusted_zero() -> u64
ensures result == apply<{selected}>(0);
"#,
        )
    };
    package.write("main.omg", &source("chosen"));
    changed.write("main.omg", &source("alternate"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static contract call should check");
        project_checked_package_review(&checked)
            .expect("an exact concrete machine argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("static apply call")
    };
    let [PackageReviewContractStaticArgument::ConcreteMachine(selected)] =
        static_arguments.as_slice()
    else {
        panic!("one exact concrete machine argument")
    };
    assert_eq!(selected.path(), "chosen::entry");
    assert_eq!(
        selected.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("chosen-machine contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("alternate-machine contract encoding"),
        "changing an exact concrete static-machine selection must change package-review identity",
    );
}

#[test]
fn review_projects_contract_machine_binders_by_canonical_static_ordinal() {
    let Some(target) = host_target_name() else {
        return;
    };
    let compile = |binder: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
pub machine trusted_apply<machine {binder}>(value: u64) -> u64
where machine {binder}(value: u64) -> u64;
requires apply<{binder}>(value) == apply<{binder}>(value)
{{
    0
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic public contract fixture should check");
        project_checked_package_review(&checked)
            .expect("a forwarded machine binder has a canonical contract row")
    };
    let original = compile("Operation");
    let renamed = compile("RenamedOperation");
    let trusted_apply = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_apply")
        .expect("trusted generic public callable");
    let [contract] = trusted_apply.contracts() else {
        panic!("one trusted-apply contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-apply equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic apply call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::GenericMachineBinder(0)]
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original generic contract encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed generic contract encoding"),
        "renaming a local machine binder must not alter package-review identity",
    );
}

#[test]
fn compiler_rejects_nested_machine_arguments_before_package_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine sample(value: u64) -> u64;
machine inspect<machine Operation>() -> u64
where machine Operation<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64;
{
    0
}
machine identity<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64;
{
    value
}
boundary machine trusted_identity() -> u64
ensures result == inspect<identity<sample>>();
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("nested machine applications must fail before checked lowering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nested machine application; recursive specialization identity")
    }));
}

#[test]
fn review_projects_exact_concrete_type_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_type: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_zero() -> u64
ensures result == tag<{selected_type}>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct concrete type argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [PackageReviewContractStaticArgument::Type(identity)] = static_arguments.as_slice() else {
        panic!("one exact concrete type argument")
    };
    assert!(identity.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("u64 static-type contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("i64 static-type contract encoding"),
        "changing an exact concrete type selection must change package-review identity",
    );
}

#[test]
fn review_projects_canonical_integer_const_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected_value: &str| {
        format!(
            r#"pub machine constant<const Value: u64>() -> u64 {{ 7 }}
boundary machine trusted_constant() -> u64
ensures result == constant<{selected_value}>();
"#,
        )
    };
    package.write("main.omg", &source("0x07"));
    changed.write("main.omg", &source("0x08"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free const-generic contract call should check");
        project_checked_package_review(&checked)
            .expect("a direct integer const argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_constant = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_constant")
        .expect("trusted const boundary callable");
    let [contract] = trusted_constant.contracts() else {
        panic!("one trusted-constant contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-constant equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("const-generic call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::ConstInteger(
            "0x7".to_owned()
        )]
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("0x7 static-const contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("0x8 static-const contract encoding"),
        "changing an exact const selection must change package-review identity",
    );
}

#[test]
fn review_alpha_normalizes_forwarded_type_and_const_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed_type = TempPackage::new();
    let changed_const = TempPackage::new();
    let source = |first: &str,
                  second: &str,
                  left: &str,
                  right: &str,
                  selected_type: &str,
                  selected_const: &str| {
        format!(
            r#"pub machine tag<Value>() -> u64 {{ 0 }}
pub machine constant<const Value: u64>() -> u64 {{ 0 }}
pub machine generic_type<{first}, {second}>() -> u64
requires tag<{selected_type}>() == tag<{selected_type}>()
{{
    0
}}
pub machine generic_const<const {left}: u64, const {right}: u64>() -> u64
requires constant<{selected_const}>() == constant<{selected_const}>()
{{
    0
}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Left"),
    );
    renamed.write(
        "main.omg",
        &source(
            "Primary",
            "Secondary",
            "Minimum",
            "Maximum",
            "Primary",
            "Minimum",
        ),
    );
    changed_type.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "Second", "Left"),
    );
    changed_const.write(
        "main.omg",
        &source("First", "Second", "Left", "Right", "First", "Right"),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed_type, &changed_const] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("forwarded type and const contract arguments should check");
        project_checked_package_review(&checked)
            .expect("forwarded type and const binders have canonical review rows")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed_type = project(&changed_type);
    let changed_const = project(&changed_const);
    let static_arguments = |name: &str| {
        let callable = original
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == name)
            .expect("generic callable");
        let [contract] = callable.contracts() else {
            panic!("one generic callable contract")
        };
        let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            right,
            ..
        }) = contract.fact()
        else {
            panic!("generic callable equality contract")
        };
        let PackageReviewContractExpression::Call {
            static_arguments, ..
        } = right.as_ref()
        else {
            panic!("generic callable contract call")
        };
        static_arguments.clone()
    };
    assert_eq!(
        static_arguments("generic_type"),
        [PackageReviewContractStaticArgument::GenericTypeBinder(0)]
    );
    assert_eq!(
        static_arguments("generic_const"),
        [PackageReviewContractStaticArgument::GenericConstBinder(0)]
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming forwarded type and const binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "selecting a different forwarded type binder must change review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_const.canonical_review_bytes().unwrap(),
        "selecting a different forwarded const binder must change review identity",
    );
}

#[test]
fn review_projects_recursive_generic_data_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |nested_type: &str| {
        format!(
            r#"pub data Wrapper<Value> {{ value: Value; }}
pub machine tag<Value>() -> u64 {{ 0 }}
boundary machine trusted_tag() -> u64
ensures result == tag<Wrapper<{nested_type}>>();
"#,
        )
    };
    package.write("main.omg", &source("u64"));
    changed.write("main.omg", &source("i64"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("nested static type contract call should check");
        project_checked_package_review(&checked)
            .expect("a recursive generic data argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_tag = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_tag")
        .expect("trusted tag boundary callable");
    let [contract] = trusted_tag.contracts() else {
        panic!("one trusted-tag contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-tag equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one generic data static argument")
    };
    assert!(base.canonical().contains("Wrapper"));
    assert!(lifetime_arguments.is_empty());
    let [PackageReviewContractStaticArgument::Type(nested)] = arguments.as_slice() else {
        panic!("one nested concrete type argument")
    };
    assert!(nested.canonical().contains("u64"));
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("Wrapper<u64> contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("Wrapper<i64> contract encoding"),
        "changing a nested concrete type must change package-review identity",
    );
}

#[test]
fn review_alpha_normalizes_lifetime_bearing_nested_type_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let source = |view_lifetime: &str, left: &str, right: &str, selected: &str| {
        format!(
            r#"pub data View<'{view_lifetime}, Value> {{ value: &'{view_lifetime} Value; }}
pub machine tag<Value>() -> u64 {{ 0 }}
pub machine generic_tag<'{left}, '{right}>(
    first: &'{left} u64,
    second: &'{right} u64
) -> u64
requires tag<View<'{selected}, u64>>() == tag<View<'{selected}, u64>>()
{{
    0
}}
"#,
        )
    };
    original.write("main.omg", &source("slot", "left", "right", "left"));
    renamed.write(
        "main.omg",
        &source("renamed_slot", "primary", "secondary", "primary"),
    );
    changed.write("main.omg", &source("slot", "left", "right", "right"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-bearing nested type contract call should check");
        project_checked_package_review(&checked)
            .expect("nested lifetime arguments have canonical binder ordinals")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed = project(&changed);
    let generic_tag = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "generic_tag")
        .expect("generic tag callable");
    let [contract] = generic_tag.contracts() else {
        panic!("one generic-tag contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("generic-tag equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic tag call")
    };
    let [
        PackageReviewContractStaticArgument::GenericType {
            lifetime_arguments, ..
        },
    ] = static_arguments.as_slice()
    else {
        panic!("one lifetime-bearing generic data argument")
    };
    assert_eq!(lifetime_arguments, &[0]);
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming caller and data lifetime binders must preserve package-review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "selecting a different caller lifetime must change package-review identity",
    );
}

#[test]
fn review_projects_contract_member_paths_with_exact_receivers_and_fields() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |left_receiver: &str, right_receiver: &str| {
        format!(
            r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine compare(first: Pair, second: Pair)
requires {left_receiver}.left == {right_receiver}.right
{{ }}
"#
        )
    };
    original.write("main.omg", &source("first", "second"));
    changed.write("main.omg", &source("second", "first"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("member-path contract fixture should check");
        project_checked_package_review(&checked).expect("member-path package review")
    };
    let original = project(&original);
    let changed = project(&changed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one member-path contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary member-path contract")
    };
    let PackageReviewContractExpression::Member {
        receiver: left_receiver,
        member: left_member,
        case_variant: left_variant,
    } = left.as_ref()
    else {
        panic!("left member path")
    };
    let PackageReviewContractExpression::Member {
        receiver: right_receiver,
        member: right_member,
        case_variant: right_variant,
    } = right.as_ref()
    else {
        panic!("right member path")
    };
    assert_eq!(
        left_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(
        right_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(1)
    );
    assert_eq!(left_member.path(), "Pair::left");
    assert_eq!(right_member.path(), "Pair::right");
    assert!(left_variant.is_none());
    assert!(right_variant.is_none());
    assert_eq!(
        left_member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original member-path encoding"),
        changed
            .canonical_review_bytes()
            .expect("changed member-path encoding"),
        "changing only the receiver coordinates must change package review identity",
    );
}

#[test]
fn review_projects_contract_casts_without_diagnostic_spelling() {
    let Some(target) = host_target_name() else {
        return;
    };
    let u16_cast = TempPackage::new();
    let u32_cast = TempPackage::new();
    let source = |target_type: &str| {
        format!(
            r#"pub machine compare(value: u8)
requires (value as {target_type}) == 1
{{ }}
"#
        )
    };
    u16_cast.write("main.omg", &source("u16"));
    u32_cast.write("main.omg", &source("u32"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    u16_cast.write("build.omg", build);
    u32_cast.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("exact widening cast contract should check");
        project_checked_package_review(&checked).expect("cast contract package review")
    };
    let u16_cast = project(&u16_cast);
    let u32_cast = project(&u32_cast);
    let compare = u16_cast
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary cast contract")
    };
    let PackageReviewContractExpression::Cast {
        value,
        target,
        arithmetic_domain,
        semantic_domain,
        semantic_domain_arguments,
        form,
    } = left.as_ref()
    else {
        panic!("structural cast expression")
    };
    assert_eq!(
        value.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert!(target.canonical().contains("u16"));
    assert_eq!(*arithmetic_domain, PackageReviewArithmeticDomain::Exact);
    assert!(semantic_domain.is_none());
    assert!(semantic_domain_arguments.is_empty());
    assert_eq!(*form, PackageReviewCastForm::Value);
    assert_ne!(
        u16_cast
            .canonical_review_bytes()
            .expect("u16 cast encoding"),
        u32_cast
            .canonical_review_bytes()
            .expect("u32 cast encoding"),
        "changing the exact cast target must change package review identity",
    );
}

#[test]
fn review_casts_retain_public_semantic_domains_and_reject_private_exposure() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let public = TempPackage::new();
    public.write(
        "main.omg",
        r#"pub domain u16::Tagged;
pub machine compare(value: u8)
requires (value as u16 in Tagged) == 1
{ }
"#,
    );
    public.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &public.0.join("main.omg"),
        Some(target),
        package_inputs(&public.0),
    )
    .expect("public semantic-domain cast contract should check");
    let review = project_checked_package_review(&checked).expect("public domain cast review");
    let compare = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one public-domain cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary public-domain cast contract")
    };
    let PackageReviewContractExpression::Cast {
        semantic_domain: Some(domain),
        ..
    } = left.as_ref()
    else {
        panic!("semantic domain cast identity")
    };
    assert_eq!(domain.path(), "u16::Tagged");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"domain u16::Hidden;
pub machine compare(value: u8)
requires (value as u16 in Hidden) == 1
{ }
"#,
    );
    private.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect("private semantic-domain cast contract should check before package review");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a public contract must not expose a private semantic domain");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public semantic domain")
    }));
}

#[test]
fn public_callable_signatures_are_exact_and_lifetime_alpha_normalized() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub machine borrow<'origin, 'scratch>(
    source: &'origin [u8],
    temporary: &'scratch [u8]
) -> &'origin [u8] { source }
pub machine identity<Value [copy]>(value: Value) -> Value { value }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'temporary [u8] { temporary }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public callable signature fixture should check");
        project_checked_package_review(&checked).expect("callable signature review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let borrow = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("borrow"))
        .expect("borrow callable row");
    assert_eq!(borrow.lifetime_parameter_count(), 2);
    assert_eq!(borrow.parameters().len(), 2);
    assert!(borrow.type_parameters().is_empty());
    assert!(!borrow.return_type().canonical().is_empty());
    assert!(
        borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("compiler-type"),
        "source-free builtin u8 must use a closed compiler atom: {}",
        borrow.parameters()[0].type_identity().canonical(),
    );
    assert!(
        !borrow.parameters()[0]
            .type_identity()
            .canonical()
            .contains("unresolved-owner"),
        "compiler builtins must not remain unresolved in package review",
    );
    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("generic identity callable row");
    assert_eq!(identity.type_parameters().len(), 1);
    assert_eq!(identity.parameters().len(), 1);

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming lifetime and type binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        changed.canonical_review_bytes().expect("changed encoding"),
        "changing the result's borrow relationship must alter canonical review evidence",
    );
}

#[test]
fn public_signatures_encode_closed_compiler_domains_and_exact_layout_schema() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Save {
    #1 value: u32;
}

pub machine inspect(
    number: f64 in Finite,
    token: u64 in Carry::AnyCpu,
    bytes: &[u8] in OmegaLayout<Save>
) { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("closed compiler-domain fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("closed compiler domains should project without textual fallback");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("inspect callable review row");
    let [number, token, bytes] = inspect.parameters() else {
        panic!("three inspect parameters")
    };
    assert!(
        number
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(number.type_identity().canonical().contains("finite"));
    assert!(
        !number
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(
        token
            .type_identity()
            .canonical()
            .contains("compiler-domain")
    );
    assert!(token.type_identity().canonical().contains("any-cpu"));
    assert!(
        !token
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
    assert!(bytes.type_identity().canonical().contains("omega-layout"));
    assert!(bytes.type_identity().canonical().contains("derived"));
    assert!(bytes.type_identity().canonical().contains("Save"));
    assert!(bytes.type_identity().canonical().contains("package-owner"));
    assert!(
        !bytes
            .type_identity()
            .canonical()
            .contains("unresolved-owner")
    );
}

#[test]
fn public_signatures_encode_structured_const_values_without_transport_or_display_text() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data UnitIndex { scale: u64; exponent: i32; }
data UnitIndices {}
const UnitIndices::Meters: UnitIndex = UnitIndex { scale: 1, exponent: 0 };

pub domain<Carrier, const Index: UnitIndex> Carrier::Quantity<Index>;
pub domain<Carrier, const Count: u64> Carrier::Counted<Count>;

pub data Reading {
    value: i64 in Quantity<UnitIndices::Meters>;
    count: i64 in Counted<7>;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("structured const package fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("structured const value should project through closed identity");
    let reading = review
        .public_data()
        .iter()
        .find(|data| data.identity().path().contains("Reading"))
        .expect("Reading review row");
    let field = |name| {
        reading
            .members()
            .iter()
            .find_map(|member| match member {
                PackageReviewDataMember::Field(field) if field.name() == name => Some(field),
                PackageReviewDataMember::Field(_) | PackageReviewDataMember::Variant { .. } => None,
            })
            .unwrap_or_else(|| panic!("Reading field `{name}`"))
    };
    let identity = field("value").type_identity().canonical();

    assert!(identity.contains("canonical-const"), "{identity}");
    assert!(identity.contains("encoding"), "{identity}");
    assert!(!identity.contains("#omega-const"), "{identity}");
    assert!(!identity.contains("UnitIndex {"), "{identity}");
    assert!(!identity.contains("unresolved-owner"), "{identity}");
    let integer = field("count").type_identity().canonical();
    assert!(integer.contains("integer-const"), "{integer}");
    assert!(integer.contains('7'), "{integer}");
    assert!(!integer.contains("unresolved-owner"), "{integer}");
}

#[test]
fn review_projects_exact_public_callable_conformances_and_static_machine_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let satisfying = TempPackage::new();
    satisfying.write(
        "main.omg",
        r#"pub trait Handler<Element> { machine handle(value: Element) -> Element; }
pub machine handle(value: u32) -> u32 satisfies Handler<u32>::handle { value }
"#,
    );
    satisfying.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &satisfying.0.join("main.omg"),
        Some(target),
        package_inputs(&satisfying.0),
    )
    .expect("public satisfier fixture should check");
    let review = project_checked_package_review(&checked).expect("public conformance review");
    let handle = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("handle"))
        .expect("public handle row");
    let [conformance] = handle.conformances() else {
        panic!("one exact public callable conformance")
    };
    assert_eq!(conformance.trait_identity().path(), "Handler");
    assert!(conformance.requirement_identity().path().contains("handle"));
    assert_eq!(conformance.arguments().len(), 1);
    assert_eq!(conformance.alias(), None);

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden { machine handle(); }
pub machine handle() satisfies Hidden::handle { }
"#,
    );
    hidden.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private-trait satisfier fixture should check");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes non-public trait `Hidden`")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub machine register<machine Selected>()
where machine Selected(value: bool) -> bool
requires value
crashes Abort
    value;
{ }
"#,
    );
    generic.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("public static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public static-machine contract should project exactly");
    let register = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("public register row");
    let [parameter] = register.type_parameters() else {
        panic!("one static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
        signature,
    )) = parameter.kind()
    else {
        panic!("register must retain its structural static-machine contract")
    };
    assert!(signature.type_parameters().is_empty());
    assert_eq!(signature.parameters().len(), 1);
    assert_eq!(signature.contracts().len(), 1);
    assert_eq!(signature.published_crash().len(), 1);
}

#[test]
fn review_projects_exact_checked_operator_realization() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{
    transition { _ -> input }
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked operator realization fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked operator realization should project exactly");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("public provider callable row");
    assert!(callable.conformances().is_empty());
    let [realization] = callable.operator_realizations() else {
        panic!("one exact checked operator realization")
    };
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::identity")
        .expect("public operator declaration row");
    assert_eq!(realization, declaration.coordinate());
}

#[test]
fn review_projects_fixed_token_checked_operator_realization_by_declaration_coordinate() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator - CheckedMath::subtract(left: i32, right: i32) -> i32;

pub machine provide_subtract(left: i32, right: i32) -> i32
satisfies CheckedMath::subtract
{
    transition { _ -> left }
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("fixed-token checked operator realization fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("fixed-token checked operator realization should project exactly");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_subtract")
        .expect("public provider callable row");
    let [realization] = callable.operator_realizations() else {
        panic!("one exact fixed-token operator realization")
    };
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::subtract")
        .expect("public fixed-token operator declaration row");
    assert_eq!(
        declaration.spelling(),
        Some(psi_language_core::OperatorSpelling::Subtract)
    );
    assert_eq!(realization, declaration.coordinate());
}

#[test]
fn review_joins_boundary_operator_realization_to_selected_provider() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub boundary operator CheckedMath::offset_zero(value: i32) -> i32;

pub data CheckedMathProvider {}
pub machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
satisfies CheckedMath::offset_zero
{
    transition { _ -> input }
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("boundary operator provider fixture should check and select uniquely");
    let review = project_checked_package_review(&checked)
        .expect("selected boundary operator provider should project exactly");
    let declaration = review
        .public_operators()
        .iter()
        .find(|shape| shape.coordinate().identity().path() == "CheckedMath::offset_zero")
        .expect("public boundary operator declaration row");
    assert!(declaration.is_boundary());
    assert_eq!(declaration.spelling(), None);
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "CheckedMathProvider::offset_zero_impl")
        .expect("public checked adapter callable row");
    let [realization] = callable.operator_realizations() else {
        panic!("one exact boundary operator realization")
    };
    assert_eq!(realization, declaration.coordinate());

    let [provider] = review.selected_providers() else {
        panic!("one selected boundary operator provider")
    };
    assert_eq!(
        provider.schema_declaration(),
        declaration.coordinate().identity()
    );
    let [provider_row] = provider.row_declarations() else {
        panic!("one selected boundary operator provider row")
    };
    assert_eq!(provider_row.realization(), callable.identity());
    assert_eq!(
        provider_row.requirement().owner(),
        declaration.coordinate().identity().owner()
    );
    assert_eq!(
        provider_row.requirement().path(),
        provider.schema().methods[0].requirement_identity
    );
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
    ));
}

#[test]
fn review_keeps_named_boundary_operator_overloads_and_private_supply_exact() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub boundary operator CheckedMath::convert(value: i32) -> i32;
pub boundary operator CheckedMath::convert(value: u64) -> u64;

pub data I32Provider {}
pub machine I32Provider::convert(input: i32) -> i32
satisfies CheckedMath::convert
{ input }

data U64Provider {}
machine U64Provider::convert(input: u64) -> u64
satisfies CheckedMath::convert
{ 0 }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("boundary operator overload providers should select independently");
    let review = project_checked_package_review(&checked)
        .expect("boundary operator overload providers should project exactly");
    let overloads = review
        .public_operators()
        .iter()
        .filter(|operator| operator.coordinate().identity().path() == "CheckedMath::convert")
        .collect::<Vec<_>>();
    assert_eq!(overloads.len(), 2);
    assert_ne!(
        overloads[0].coordinate().parameter_dispatch(),
        overloads[1].coordinate().parameter_dispatch()
    );
    assert!(
        overloads
            .iter()
            .all(|operator| operator.coordinate().result_dispatch().is_empty())
    );

    let public_adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "I32Provider::convert")
        .expect("public i32 adapter callable");
    let [public_realization] = public_adapter.operator_realizations() else {
        panic!("one exact public boundary operator realization")
    };
    assert!(overloads.iter().any(|operator| {
        operator.coordinate() == public_realization
            && operator.coordinate().parameter_dispatch().contains("i32")
    }));
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "U64Provider::convert"),
        "private selected adapters must not become public callable rows"
    );

    assert_eq!(review.selected_providers().len(), 2);
    let selected_realizations = review
        .selected_providers()
        .iter()
        .flat_map(|provider| provider.row_declarations())
        .map(|row| row.realization().path())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        selected_realizations,
        std::collections::BTreeSet::from(["I32Provider::convert", "U64Provider::convert"])
    );
}

#[test]
fn changing_checked_operator_realization_changes_only_the_callable_value() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data FirstMath {{}}
pub data OtherMath {{}}
pub operator FirstMath::identity(value: i32) -> i32;
pub operator OtherMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies {selected}::identity
{{
    input
}}
"#,
            ),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator selection fixture should check");
        project_checked_package_review(&checked).expect("operator selection should project")
    };

    let first = compile("FirstMath");
    let other = compile("OtherMath");
    assert_eq!(first.public_operators(), other.public_operators());
    let first_callable = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("first provider callable");
    let other_callable = other
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("other provider callable");
    assert_ne!(
        first_callable.operator_realizations(),
        other_callable.operator_realizations()
    );

    let first_rows = first
        .canonical_rows()
        .expect("first operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    let other_rows = other
        .canonical_rows()
        .expect("other operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    assert_eq!(first_rows.len(), other_rows.len());
    assert!(
        first_rows
            .iter()
            .zip(&other_rows)
            .all(|(left, right)| left.key_bytes() == right.key_bytes())
    );
    assert_eq!(
        first_rows
            .iter()
            .zip(&other_rows)
            .filter(|(left, right)| left.canonical_bytes() != right.canonical_bytes())
            .count(),
        1,
        "only the provider callable value should change"
    );
}

#[test]
fn operator_realization_rejects_post_check_reselection() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("weaker operator realization control fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let satisfies = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine")
        .satisfies;
    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check redirection to a stronger operator must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn operator_realization_rejects_coordinated_typed_contract_tampering() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("operator contract-custody fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let stronger_fact = checked.typed.operator_contracts(stronger)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(stronger_expression) =
        checked.typed.proof_facts.get(stronger_fact)
    else {
        panic!("stronger operator expression contract")
    };
    let stronger_expression_node = checked
        .typed
        .expression_table
        .expression(*stronger_expression)
        .clone();
    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine");
    let provider_fact = checked.typed.machine_contracts(provider)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(provider_expression) =
        checked.typed.proof_facts.get(provider_fact)
    else {
        panic!("provider expression contract")
    };
    let provider_expression = *provider_expression;
    let satisfies = provider.satisfies;

    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;
    *checked
        .typed
        .expression_table
        .expression_mut(provider_expression) = stronger_expression_node;

    let mutated_provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("mutated provider machine");
    let mutated_operator = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("mutated stronger operator selection");
    psi_validation::validate_checked_operator_realization_contract(
        &checked.typed,
        mutated_provider,
        mutated_operator,
    )
    .expect("coordinated mutable typed state would pass contract revalidation alone");

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("checked custody must reject coordinated typed contract tampering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn unsupported_checked_operator_realization_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let cases = [
        (
            "private",
            r#"data CheckedMath {}
operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
            "realizes non-public operator",
        ),
        (
            "aliased",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity as Selected
{ input }
"#,
            "through an alias not yet represented",
        ),
        (
            "external",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
via Binding::Syscall(60);
"#,
            "one exact boundary operator",
        ),
        (
            "bodyless",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub boundary machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity;
"#,
            "without one checked implementation body",
        ),
        (
            "crash-contract",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32
crashes Trap;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
            "outcome-specific or crash contracts outside checked operator refinement",
        ),
        (
            "provider-crash",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
crashes Trap
{ input }
"#,
            "nonempty checked crash behavior outside checked operator refinement",
        ),
        (
            "fixed-token-boundary",
            r#"pub data CheckedMath {}
pub boundary operator - CheckedMath::negate(value: i32) -> i32;
pub data CheckedMathProvider {}
pub machine CheckedMathProvider::negate_impl(input: i32) -> i32
satisfies CheckedMath::negate
{ input }
"#,
            "before checked-adapter token dispatch is represented",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }

    let compile_admission_control = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator admission-drift control fixture should check")
    };

    let mut visibility_drift = compile_admission_control(
        r#"data CheckedMath {}
operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let operator_roots = visibility_drift.typed.roots.operators;
    visibility_drift
        .typed
        .tables
        .operators
        .span_mut_or_empty(operator_roots)[0]
        .is_public = true;
    let diagnostics = project_checked_package_review(&visibility_drift)
        .expect_err("post-check private-to-public operator drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut alias_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity as Selected
{ input }
"#,
    );
    let provider = alias_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("aliased provider machine");
    let satisfies = provider.satisfies;
    alias_drift
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .alias = None;
    let diagnostics = project_checked_package_review(&alias_drift)
        .expect_err("post-check removal of an operator alias must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut signature_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine u64_helper(value: u64) { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let helper = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "u64_helper")
        .expect("u64 helper machine");
    let helper_state = &signature_drift.typed.machine_states(helper)[0];
    let u64_type = signature_drift.typed.state_parameters(helper_state)[0].type_reference;
    let u64_node = signature_drift
        .typed
        .type_reference_table
        .type_reference(u64_type)
        .clone();
    let provider = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("signature-drift provider");
    let provider_state = &signature_drift.typed.machine_states(provider)[0];
    let provider_type = signature_drift.typed.state_parameters(provider_state)[0].type_reference;
    let operator_type = signature_drift.typed.operator_parameters(
        signature_drift
            .typed
            .operators()
            .first()
            .expect("signature-drift operator"),
    )[0]
    .type_reference;
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(provider_type, u64_node.clone());
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(operator_type, u64_node);
    let diagnostics = project_checked_package_review(&signature_drift)
        .expect_err("coordinated post-check overload-shape drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut lifetime_drift = compile_admission_control(
        r#"pub data CheckedBorrow {}
pub operator CheckedBorrow::observe(first: &[u8], second: &[u8]);
pub machine provide_observe<'first, 'second>(
    first: &'first [u8],
    second: &'second [u8]
)
satisfies CheckedBorrow::observe
{ }
"#,
    );
    let provider = lifetime_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_observe")
        .expect("lifetime-drift provider");
    let state = &lifetime_drift.typed.machine_states(provider)[0];
    let parameters = lifetime_drift.typed.state_parameters(state);
    let first_type = parameters[0].type_reference;
    let second_type = parameters[1].type_reference;
    assert_ne!(
        first_type, second_type,
        "distinct lifetime-bearing type nodes"
    );
    let second_node = lifetime_drift
        .typed
        .type_reference_table
        .type_reference(second_type)
        .clone();
    lifetime_drift
        .typed
        .type_reference_table
        .substitute_node(first_type, second_node);
    let diagnostics = project_checked_package_review(&lifetime_drift)
        .expect_err("post-check lifetime-topology drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine generic<Element>() { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    generic.write("build.omg", build);
    let mut checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("generic-tamper control fixture should check");
    let type_parameters = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generic")
        .expect("generic helper machine")
        .type_parameters;
    let mut forged_type_parameter = checked.clone();
    let operators = forged_type_parameter.typed.roots.operators;
    forged_type_parameter
        .typed
        .tables
        .operators
        .span_mut_or_empty(operators)[0]
        .type_parameters = type_parameters;
    forged_type_parameter
        .facts
        .operators
        .operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &forged_type_parameter.typed,
        );
    let diagnostics = project_checked_package_review(&forged_type_parameter)
        .expect_err("post-check generic operator realization must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes generic or lifetime-parameterized operator")
    }));

    let operator = &checked.typed.operators()[0];
    let forged_lifetime = checked.typed.operator_path_members(operator.name)[0].clone();
    let operators = checked.typed.roots.operators;
    checked.typed.tables.operators.span_mut_or_empty(operators)[0]
        .lifetime_parameters
        .push(forged_lifetime);
    checked.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &checked.typed,
        );
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check generic operator realization must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes generic or lifetime-parameterized operator")
    }));

    let mut duplicate = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("duplicate-realization control fixture should check");
    let machine_index = duplicate
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine index");
    let machine_symbol = duplicate.typed.machines()[machine_index].symbol;
    let repeated = duplicate
        .typed
        .machine_trait_conformances(&duplicate.typed.machines()[machine_index])[0]
        .clone();
    let repeated_checked = duplicate
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .find(|row| row.machine_symbol() == machine_symbol)
        .expect("provider checked operator-realization contract")
        .clone();
    duplicate
        .facts
        .operators
        .operator_realization_contracts
        .push(repeated_checked);
    let machine_roots = duplicate.typed.roots.machines;
    let tables = &mut duplicate.typed.tables;
    let machine = &mut tables.machines.span_mut_or_empty(machine_roots)[machine_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut machine.satisfies, repeated);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate exact operator realizations must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicate exact operator realization")
    }));
}

#[test]
fn review_projects_trait_requirement_identity_machine_parameter() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "pub trait LocalSlot<machine Requirement> { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("public requirement-identity fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("closed requirement-identity parameter should project");
    let local_slot = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path().contains("LocalSlot"))
        .expect("LocalSlot review row");
    let [parameter] = local_slot.type_parameters() else {
        panic!("one requirement-identity machine parameter")
    };
    assert!(matches!(
        parameter.kind(),
        PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::RequirementIdentity
        )
    ));
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        project_checked_package_review(&checked)
            .unwrap()
            .canonical_review_bytes()
            .unwrap(),
    );
    assert_eq!(
        review.canonical_rows().unwrap(),
        project_checked_package_review(&checked)
            .unwrap()
            .canonical_rows()
            .unwrap(),
    );
}

#[test]
fn review_projects_alpha_normalized_public_conformance_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let original = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    original.write("build.omg", build);
    let renamed = TempPackage::new();
    renamed.write(
        "main.omg",
        r#"pub trait Ranked<Measure> { }
pub trait Alternate<Measure> { }
pub trait Ordering<Value, Unused, OrderingEvidence: Value satisfies Ranked<u32>> { }
pub machine identity<Value, Unused, IdentityEvidence: Value satisfies Ranked<u32>>(value: Value) -> Value {
    value
}
"#,
    );
    renamed.write("build.omg", build);
    let changed = TempPackage::new();
    changed.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Alternate<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Alternate<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed.write("build.omg", build);
    let changed_subject = TempPackage::new();
    changed_subject.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Other satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Other satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_subject.write("build.omg", build);
    let changed_argument = TempPackage::new();
    changed_argument.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u64>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u64>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_argument.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic conformance-binder fixture should check");
        project_checked_package_review(&checked)
            .expect("generic conformance-binder review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let changed_subject = review(&changed_subject);
    let changed_argument = review(&changed_argument);

    let ordering = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Ordering")
        .expect("public Ordering row");
    let [trait_bound] = ordering.conformance_bounds() else {
        panic!("one exact trait conformance binder")
    };
    assert_eq!(trait_bound.binder_ordinal(), Some(0));
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert_eq!(trait_bound.arguments().len(), 1);

    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [callable_bound] = identity.conformance_bounds() else {
        panic!("one exact callable conformance binder")
    };
    assert_eq!(callable_bound.binder_ordinal(), Some(0));
    assert_eq!(callable_bound.subject_parameter(), 0);
    assert_eq!(callable_bound.trait_identity().path(), "Ranked");

    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming conformance, type, and lifetime-free evidence binders must not change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing the exact conformance trait must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_subject.canonical_review_bytes().unwrap(),
        "changing the conformance subject parameter must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_argument.canonical_review_bytes().unwrap(),
        "changing a conformance trait argument must change review identity"
    );
}

#[test]
fn review_projects_alpha_normalized_trait_proposition_parameter_signatures() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let source = |carrier: &str, relation: &str, left: &str, right: &str, right_type: &str| {
        format!(
            r#"pub trait RelationShape<{carrier}, proposition {relation}>
where proposition {relation}({left}: {carrier}, {right}: {right_type});
{{}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source("Carrier", "Relation", "left", "right", "Carrier"),
    );
    renamed.write(
        "main.omg",
        &source("Value", "Equivalent", "first", "second", "Value"),
    );
    changed.write(
        "main.omg",
        &source("Carrier", "Relation", "left", "right", "u64"),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public proposition-parameter trait should check");
        project_checked_package_review(&checked)
            .expect("proposition-parameter signatures have canonical review rows")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed = project(&changed);
    let shape = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "RelationShape")
        .expect("RelationShape trait");
    let [_, relation] = shape.type_parameters() else {
        panic!("carrier and proposition parameters")
    };
    let PackageReviewTypeParameterKind::Proposition(signature) = relation.kind() else {
        panic!("proposition parameter signature")
    };
    let [left, right] = signature.parameters() else {
        panic!("two proposition value parameters")
    };
    assert!(
        left.type_identity()
            .canonical()
            .contains("type-parameter:0")
    );
    assert!(
        right
            .type_identity()
            .canonical()
            .contains("type-parameter:0")
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming trait, proposition, and proposition-value binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing a proposition parameter value type must change review identity",
    );
}

#[test]
fn review_rejects_uncertified_proposition_parameter_modes() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationShape<Carrier, proposition Relation>
where proposition Relation(const value: Carrier);
{}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("non-default proposition parameter mode currently reaches checked IR");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("non-default value-parameter mode not yet certified")
    }));
}

#[test]
fn review_projects_generic_proposition_contract_endpoints_by_static_ordinal() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed_endpoint = TempPackage::new();
    let changed_arguments = TempPackage::new();
    let source = |carrier: &str,
                  relation: &str,
                  alternate: &str,
                  left: &str,
                  right: &str,
                  selected: &str,
                  first_argument: &str,
                  second_argument: &str| {
        format!(
            r#"pub trait RelationLaw<{carrier}, proposition {relation}, proposition {alternate}>
where proposition {relation}(first: {carrier}, second: {carrier});
where proposition {alternate}(first: {carrier}, second: {carrier});
{{
    machine reverse({left}: {carrier}, {right}: {carrier})
    ensures {selected}({first_argument}, {second_argument});
}}
"#,
        )
    };
    original.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Relation",
            "right",
            "left",
        ),
    );
    renamed.write(
        "main.omg",
        &source(
            "Value",
            "Equivalent",
            "Other",
            "left",
            "right",
            "Equivalent",
            "right",
            "left",
        ),
    );
    changed_endpoint.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Alternate",
            "right",
            "left",
        ),
    );
    changed_arguments.write(
        "main.omg",
        &source(
            "Carrier",
            "Relation",
            "Alternate",
            "left",
            "right",
            "Relation",
            "left",
            "right",
        ),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    for package in [&original, &renamed, &changed_endpoint, &changed_arguments] {
        package.write("build.omg", build);
    }
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic proposition contract endpoint should check");
        project_checked_package_review(&checked)
            .expect("generic proposition contract endpoint should project exactly")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let changed_endpoint = project(&changed_endpoint);
    let changed_arguments = project(&changed_arguments);
    let law = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "RelationLaw")
        .expect("RelationLaw trait row");
    let [reverse] = law.requirements() else {
        panic!("one relation law")
    };
    let [contract] = reverse.contracts() else {
        panic!("one relation law contract")
    };
    let PackageReviewContractFact::PropositionParameter(application) = contract.fact() else {
        panic!("generic proposition-parameter application")
    };
    assert_eq!(application.binder_ordinal(), 1);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(1),
            PackageReviewContractExpression::Parameter(0),
        ]
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming trait and proposition-family binders must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_endpoint.canonical_review_bytes().unwrap(),
        "selecting a different proposition-family binder must change review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_arguments.canonical_review_bytes().unwrap(),
        "changing proposition value arguments must change review identity",
    );
}

#[test]
fn compiler_rejects_named_generic_proposition_evidence_before_package_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationLaw<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine use(value: Carrier)
    requires proof: Relation(value, value);
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("named generic proposition evidence must fail before checked lowering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not resolve to one nominal proposition endpoint")
    }));
}

#[test]
fn review_projects_composed_relation_laws_with_forwarded_proposition_family() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Reflexive<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine reflexive(value: Carrier)
    ensures Relation(value, value);
}

pub trait Symmetric<Carrier, proposition Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
    machine symmetric(left: Carrier, right: Carrier)
    requires Relation(left, right)
    ensures Relation(right, left);
}

pub trait Equivalence<Carrier, proposition Relation>:
    Reflexive<Carrier, Relation>
    + Symmetric<Carrier, Relation>
where proposition Relation(left: Carrier, right: Carrier);
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("composed relation laws should check");
    let review = project_checked_package_review(&checked)
        .expect("composed relation laws should have exact package-review rows");
    let equivalence = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Equivalence")
        .expect("Equivalence trait row");
    assert_eq!(equivalence.parents().len(), 2);
    for parent in equivalence.parents() {
        let [carrier, relation] = parent.arguments() else {
            panic!("forwarded carrier and proposition-family arguments")
        };
        assert!(carrier.canonical().contains("type-parameter:0"));
        assert!(relation.canonical().contains("type-parameter:1"));
    }
}

#[test]
fn review_rejects_generic_proposition_endpoint_and_value_symbol_spoofs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait RelationLaw<Carrier, proposition Relation>
where proposition Relation(value: Carrier);
{
    machine use(value: Carrier)
    ensures Relation(value);
}

trait OtherLaw<Carrier, proposition OtherRelation>
where proposition OtherRelation(value: Carrier);
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("generic proposition spoof fixture should check before mutation");
    project_checked_package_review(&checked).expect("unmodified generic proposition review");

    let law = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "RelationLaw")
        .expect("RelationLaw definition");
    let [signature] = checked.trait_machine_signatures(law) else {
        panic!("one RelationLaw requirement")
    };
    let [contract] = checked.state_signature_contracts(signature) else {
        panic!("one RelationLaw contract")
    };
    let fact_handle = contract.facts.start();
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        checked.proof_facts.get(fact_handle)
    else {
        panic!("generic proposition fact")
    };
    let [argument_handle] = checked
        .expression_table
        .expression_handles(application.arguments)
    else {
        panic!("one generic proposition argument")
    };
    let argument_handle = *argument_handle;

    let other = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "OtherLaw")
        .expect("OtherLaw definition");
    let [_, other_relation] = checked.trait_type_parameters(other) else {
        panic!("OtherLaw carrier and proposition parameters")
    };
    let other_relation_symbol = other_relation.symbol;
    let psi_typed_trees::data::TypeParameterKind::Proposition { contract } = &other_relation.kind
    else {
        panic!("OtherRelation signature")
    };
    let [other_value] = checked.state_parameters.span_or_empty(contract.parameters) else {
        panic!("one OtherRelation value parameter")
    };
    let other_value_symbol = other_value.symbol;

    let mut endpoint_spoof = checked.clone();
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        endpoint_spoof.typed.proof_facts.get_mut(fact_handle)
    else {
        panic!("generic proposition fact")
    };
    application.proposition = other_relation_symbol;
    let diagnostics = project_checked_package_review(&endpoint_spoof)
        .expect_err("a foreign generic proposition binder must not rejoin by category");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("generic proposition endpoint rejoins 0 callable static binders")
    }));

    let mut value_spoof = checked;
    let psi_typed_trees::expression::ExpressionNode::Name(path) = value_spoof
        .typed
        .expression_table
        .expression_mut(argument_handle)
    else {
        panic!("generic proposition name argument")
    };
    path.head_symbol = other_value_symbol;
    path.symbol = other_value_symbol;
    let diagnostics = project_checked_package_review(&value_spoof)
        .expect_err("a same-spelled foreign value symbol must not rejoin a callable parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contract parameter spelling does not match its exact resolved symbol")
    }));
}

#[test]
fn review_static_machine_contracts_are_recursive_alpha_stable_and_shape_sensitive() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    let original_source = r#"pub machine register<machine Schema>()
where machine Schema<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64
requires value == value;
{ }
"#;
    original.write("main.omg", original_source);
    renamed.write(
        "main.omg",
        r#"pub machine register<machine Operation>()
where machine Operation<machine Callback>(value: u64) -> u64
where machine Callback(value: u64) -> u64
requires value == value;
{ }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine register<machine Operation>()
where machine Operation<machine Callback>(value: u64) -> u64
where machine Callback(value: i64) -> u64
requires value == value;
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("higher-order static-machine fixture should check");
        project_checked_package_review(&checked)
            .expect("higher-order static-machine contract should project")
    };
    let original = review(&original);
    let register = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("register callable row");
    let [schema] = register.type_parameters() else {
        panic!("one outer static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
        signature,
    )) = schema.kind()
    else {
        panic!("outer structural contract")
    };
    let [inner] = signature.type_parameters() else {
        panic!("one nested static-machine parameter")
    };
    assert!(matches!(
        inner.kind(),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            _
        ))
    ));
    let register_row = original
        .canonical_rows()
        .expect("static-machine canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("register".len())
                    .any(|window| window == b"register")
        })
        .expect("register callable canonical row");
    assert!(
        register_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::ContractClause
                    && &original_source[start..end] == "requires"
            }))
    );

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original static-machine encoding"),
        review(&renamed)
            .canonical_review_bytes()
            .expect("renamed static-machine encoding"),
        "renaming nested static-machine binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original static-machine encoding"),
        review(&changed)
            .canonical_review_bytes()
            .expect("changed static-machine encoding"),
        "changing a nested static-machine contract must alter canonical review evidence",
    );
}

#[test]
fn review_static_machine_nominal_contracts_require_exact_public_requirements() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Handler {
    machine call(value: i32) -> i32;
}
pub machine register<machine Selected>()
where machine Selected satisfies Handler::call;
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("nominal static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public nominal static-machine contract should project");
    let register = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("register"))
        .expect("register callable row");
    let [selected] = register.type_parameters() else {
        panic!("one nominal static-machine parameter")
    };
    let PackageReviewTypeParameterKind::Machine(contract) = selected.kind() else {
        panic!("nominal static-machine parameter")
    };
    let Some((trait_identity, requirement_identity)) = contract.nominal() else {
        panic!("exact nominal requirement contract")
    };
    assert_eq!(trait_identity.path(), "Handler");
    assert!(requirement_identity.path().contains("Handler::call"));

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden {
    machine call(value: i32) -> i32;
}
pub machine register<machine Selected>()
where machine Selected satisfies Hidden::call;
{ }
"#,
    );
    hidden.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private nominal static-machine fixture should check before package review");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public trait `Hidden` through a static-machine contract")
    }));
}

#[test]
fn review_static_machine_contracts_cover_public_proof_data() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Stream<machine Sample>
where machine Sample(index: u64) -> u64;
{
    case Empty;
    case More(tail: Stream<Sample>);
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("public proof-data static-machine fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public proof-data static-machine contract should project");
    let [stream] = review.public_data() else {
        panic!("one public proof-data row")
    };
    let [sample] = stream.type_parameters() else {
        panic!("one proof-data static-machine parameter")
    };
    assert!(matches!(
        sample.kind(),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            _
        ))
    ));
}

#[test]
fn review_projects_binder_free_conformance_requirements_without_fabricating_evidence() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked { }
pub trait Constraint<Element>
where Element satisfies Ranked
{ }
pub machine identity<Element>(value: Element) -> Element
where Element satisfies Ranked
{
    value
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("unbound conformance-requirement fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("binder-free conformance requirement must project exactly");
    let identity = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [bound] = identity.conformance_bounds() else {
        panic!("one exact binder-free conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(bound.trait_identity().path(), "Ranked");
    let constraint = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Constraint")
        .expect("public Constraint row");
    let [trait_bound] = constraint.conformance_bounds() else {
        panic!("one exact trait binder-free conformance requirement")
    };
    assert_eq!(trait_bound.binder_ordinal(), None);
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_exact_selected_conformance_carrier_trait_and_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Marker<Tag> { }
pub data Tag { }
pub data Good { }
pub Primary: Good satisfies Marker<Tag> { }
pub machine accept<Element>(value: &Element)
where Element satisfies Good::Primary
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("selected-conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("exact non-generic selected conformance should project");
    let [conformance] = review.public_conformances() else {
        panic!("one package-owned public conformance row")
    };
    assert_eq!(conformance.identity().path(), "Primary");
    assert_eq!(conformance.lifetime_parameter_count(), 0);
    assert!(conformance.type_parameters().is_empty());
    let PackageReviewConformanceSubject::Nominal(subject) = conformance.subject() else {
        panic!("the public conformance has one nominal carrier")
    };
    assert_eq!(subject.path(), "Good");
    assert_eq!(conformance.interface().trait_identity().path(), "Marker");
    let [argument] = conformance.interface().arguments() else {
        panic!("one exact trait argument")
    };
    assert!(argument.canonical().contains("Tag"));
    assert!(conformance.interface().requirements().is_empty());
    assert!(review.canonical_rows().unwrap().iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConformance
            && row.risk() == PackageReviewCanonicalRowRisk::Blocking
    }));
    let accept = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("accept"))
        .expect("public accept row");
    let [bound] = accept.conformance_bounds() else {
        panic!("one exact selected conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance")
            .path(),
        "Primary"
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected conformance has one exact nominal subject")
    };
    assert!(subject.canonical().contains("Good"));
    assert!(bound.selected_lifetime_arguments().is_empty());
    assert!(bound.selected_arguments().is_empty());
    assert_eq!(bound.trait_identity().path(), "Marker");
    assert_eq!(bound.arguments().len(), 1);
    assert!(bound.arguments()[0].canonical().contains("Tag"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_complete_selected_generic_conformance_application() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub FullEncoding<'scope, Element, Output, const Rank: u64>:
    Element satisfies Encodes<Output>
{ }
pub machine inspect<'view, Element>(value: &'view Element)
where Element satisfies Card::FullEncoding<'view, Card, Message, 7>
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("selected generic conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("the complete selected conformance application must project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one exact selected generic conformance requirement")
    };
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance declaration")
            .path(),
        "FullEncoding"
    );
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [
        PackageReviewContractStaticArgument::Type(card),
        PackageReviewContractStaticArgument::Type(message),
        PackageReviewContractStaticArgument::ConstInteger(rank),
    ] = bound.selected_arguments()
    else {
        panic!("selected application retains its exact categorized telescope")
    };
    assert!(card.canonical().contains("Card"));
    assert!(message.canonical().contains("Message"));
    assert_eq!(rank, "7");
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("selected application retains its instantiated subject")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.trait_identity().path(), "Encodes");
    let [trait_argument] = bound.arguments() else {
        panic!("selected application retains its instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Message"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn selected_generic_conformance_rows_alpha_normalize_and_detect_application_changes() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |lifetime: &str, output: &str| {
        format!(
            r#"pub trait Encodes<Output> {{ }}
pub data Card {{ }}
pub data First {{ }}
pub data Second {{ }}
pub Scoped<'scope, Element, Output>:
    Element satisfies Encodes<Output>
{{ }}
pub machine inspect<'{lifetime}, Element>(value: &'{lifetime} Element)
where Element satisfies Card::Scoped<'{lifetime}, Card, {output}>
{{ }}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("selected generic conformance comparison fixture should check");
        project_checked_package_review(&checked)
            .expect("selected generic conformance comparison fixture should project")
            .canonical_review_bytes()
            .expect("selected generic conformance comparison bytes")
    };

    let first = project(source("view", "First"));
    let renamed = project(source("borrow", "First"));
    let changed = project(source("view", "Second"));
    assert_eq!(first, renamed);
    assert_ne!(first, changed);
}

#[test]
fn selected_generic_conformance_rows_substitute_lifetimes_into_trait_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |first: &str, second: &str, selected: &str| {
        format!(
            r#"pub trait Borrows<Source> {{ }}
pub data Card {{ }}
pub data Borrow<'scope, Element> {{ value: &'scope Element; }}
pub Scoped<'scope, Element>:
    Element satisfies Borrows<Borrow<'scope, Element>>
{{ }}
pub machine inspect<'{first}, '{second}, Element>(
    value: &'{first} Element,
    other: &'{second} Element
)
where Element satisfies Card::Scoped<'{selected}, Card>
{{ }}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-bearing selected conformance should check");
        project_checked_package_review(&checked)
            .expect("selected lifetime substitution should project")
    };

    let first = project(source("left", "right", "left"));
    let inspect = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one lifetime-bearing selected bound")
    };
    assert_eq!(bound.selected_lifetime_arguments(), [0]);
    let [trait_argument] = bound.arguments() else {
        panic!("one instantiated trait argument")
    };
    assert!(trait_argument.canonical().contains("Borrow"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("primary", "secondary", "primary"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("left", "right", "right"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());
}

#[test]
fn review_alpha_normalizes_forwarded_selected_conformance_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Encodes<Output> { }
pub data Card { }
pub data Message { }
pub Encoding<Output, const Rank: u64>:
    Card satisfies Encodes<Output>
{ }
pub machine inspect<Output, const Rank: u64, Element>(value: &Element)
where Element satisfies Card::Encoding<Output, Rank>
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("forwarded selected conformance arguments should check");
    let review = project_checked_package_review(&checked)
        .expect("forwarded selected conformance arguments should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public inspect row");
    let [bound] = inspect.conformance_bounds() else {
        panic!("one forwarded selected conformance bound")
    };
    assert_eq!(
        bound.selected_arguments(),
        [
            PackageReviewContractStaticArgument::GenericTypeBinder(0),
            PackageReviewContractStaticArgument::GenericConstBinder(1),
        ]
    );
    let Some(PackageReviewContractStaticArgument::Type(subject)) = bound.selected_subject() else {
        panic!("fixed selected subject is retained exactly")
    };
    assert!(subject.canonical().contains("Card"));
    assert_eq!(bound.arguments().len(), 1);
}

#[test]
fn review_projects_public_core_private_callback_slot_conformance() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::layout;

pub trait WindowProcedure {
    machine call();
}
pub data WndClassLayout { }

pub WndClassWindowProcedureSlot:
    WndClassLayout satisfies
        PrivateCallbackSlot<WindowProcedure::call>;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("public private-callback-slot fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("toolchain-owned requirement-identity conformance should project");
    let [conformance] = review.public_conformances() else {
        panic!("one public private-callback-slot conformance")
    };
    assert_eq!(conformance.identity().path(), "WndClassWindowProcedureSlot");
    let PackageReviewConformanceSubject::Nominal(subject) = conformance.subject() else {
        panic!("private callback slot must retain its nominal layout subject")
    };
    assert_eq!(subject.path(), "WndClassLayout");
    let interface = conformance.interface();
    assert_eq!(interface.trait_identity().path(), "PrivateCallbackSlot");
    assert!(matches!(
        interface.trait_identity().owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ));
    let [argument] = interface.arguments() else {
        panic!("one exact callback requirement identity argument")
    };
    assert!(argument.canonical().contains("WindowProcedure"));
    assert!(argument.canonical().contains("call"));
    assert!(interface.requirements().is_empty());
    assert!(review.canonical_rows().unwrap().iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConformance
            && row.risk() == PackageReviewCanonicalRowRisk::Blocking
    }));
}

#[test]
fn public_conformance_rows_are_alpha_normalized_and_exclude_private_realizations() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |binder: &str, value: i32| {
        format!(
            r#"pub trait Marker<Tag> {{
    machine Self::code(&self) -> i32;
}}
pub data Good {{ }}
pub Generic<{binder}>: {binder} satisfies Marker<{binder}> {{
    machine code(&self) -> i32 {{ {value} }}
}}
"#,
        )
    };
    package.write("main.omg", &source("Element", 1));
    let first = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("first generic public conformance should check");
    let first = project_checked_package_review(&first).expect("first row should project");
    let [shape] = first.public_conformances() else {
        panic!("one public generic conformance")
    };
    assert!(matches!(
        shape.subject(),
        PackageReviewConformanceSubject::TypeParameter(0)
    ));
    assert_eq!(shape.type_parameters().len(), 1);
    let [requirement] = shape.interface().requirements() else {
        panic!("one complete normalized requirement row")
    };
    assert!(requirement.requirement().path().contains("Marker::code"));
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("public conformance canonical row");

    package.write("main.omg", &source("Value", 2));
    let second = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("renamed telescope and changed private body should check");
    let second = project_checked_package_review(&second).expect("second row should project");
    let second_row = second
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("second public conformance canonical row");
    assert_eq!(first_row.key_bytes(), second_row.key_bytes());
    assert_eq!(first_row.canonical_bytes(), second_row.canonical_bytes());
}

#[test]
fn public_conformance_rows_alpha_normalize_lifetime_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |lifetime: &str| {
        format!(
            r#"pub trait Borrows<Source> {{ }}
pub data Borrow<'{lifetime}, Element> {{ value: &'{lifetime} Element; }}
pub Scoped<'{lifetime}, Element>:
    Element satisfies Borrows<Borrow<'{lifetime}, Element>>
{{ }}
"#
        )
    };

    package.write("main.omg", &source("scope"));
    let first = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("first lifetime-generic public conformance should check");
    let first = project_checked_package_review(&first)
        .expect("first lifetime-generic public conformance should project");
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("first lifetime-generic public conformance row");

    package.write("main.omg", &source("view"));
    let second = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("renamed lifetime-generic public conformance should check");
    let second = project_checked_package_review(&second)
        .expect("renamed lifetime-generic public conformance should project");
    let second_row = second
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("second lifetime-generic public conformance row");

    assert_eq!(first_row.key_bytes(), second_row.key_bytes());
    assert_eq!(first_row.canonical_bytes(), second_row.canonical_bytes());
}

#[test]
fn public_lifetime_conformances_project_inherited_requirement_substitutions() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |first: &str, second: &str, selected: &str, body: &str| {
        format!(
            r#"pub data Borrow<'{first}, Element> {{ value: &'{first} Element; }}
pub trait Parent<Source> {{
    machine absorb(value: Source);
}}
pub trait Child<Source>: Parent<Source> {{ }}
pub Scoped<'{first}, '{second}, Element>:
    Element satisfies Child<Borrow<'{selected}, Element>>
{{
    machine absorb(value: Borrow<'{selected}, Element>) {{ {body} }}
}}
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("lifetime-generic inherited conformance should check");
        project_checked_package_review(&checked)
            .expect("inherited lifetime substitution should project exactly")
    };

    let first = project(source("left", "right", "left", ""));
    let [shape] = first.public_conformances() else {
        panic!("one inherited lifetime conformance")
    };
    let [requirement] = shape.interface().requirements() else {
        panic!("one inherited requirement")
    };
    assert_eq!(requirement.declaring_trait().path(), "Parent");
    assert_eq!(
        requirement.declaring_trait_arguments(),
        shape.interface().arguments()
    );
    let first_row = first
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("first inherited lifetime conformance row");

    let renamed = project(source(
        "primary",
        "secondary",
        "primary",
        "let private_value: i32 = 1;",
    ));
    let renamed_row = renamed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("renamed inherited lifetime conformance row");
    assert_eq!(first_row.canonical_bytes(), renamed_row.canonical_bytes());

    let changed = project(source("left", "right", "right", ""));
    let changed_row = changed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("changed inherited lifetime conformance row");
    assert_ne!(first_row.canonical_bytes(), changed_row.canonical_bytes());
}

#[test]
fn public_conformance_identity_is_independent_of_bodyless_or_closed_realization_form() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = |implementation: &str| {
        format!(
            r#"pub trait Marker {{ machine Self::touch(&self); }}
pub data Good {{ }}
{implementation}
"#
        )
    };
    package.write(
        "main.omg",
        &source("pub Primary: Good satisfies Marker;\nmachine Good::touch(&self) { }"),
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let bodyless = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("bodyless public conformance is valid static language input");
    let bodyless = project_checked_package_review(&bodyless)
        .expect("checked bodyless public conformance should project");
    let bodyless_row = bodyless
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("bodyless public conformance canonical row");

    package.write(
        "main.omg",
        &source("pub Primary: Good satisfies Marker { machine touch(&self) { } }"),
    );
    let closed = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("closed public conformance is valid static language input");
    let closed = project_checked_package_review(&closed)
        .expect("checked closed public conformance should project");
    let closed_row = closed
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("closed public conformance canonical row");

    assert_eq!(bodyless_row.key_bytes(), closed_row.key_bytes());
    assert_eq!(bodyless_row.canonical_bytes(), closed_row.canonical_bytes());
}

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"pub boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"pub boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}

#[test]
fn public_data_and_numbered_wire_shape_changes_change_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u32; }\ndata Private { ignored: u32; }\n",
    );
    second.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u64; }\ndata Private { changed: i64; }\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-shape fixture should check");
        project_checked_package_review(&checked)
            .expect("public-shape review should close")
            .canonical_review_bytes()
            .expect("public-shape encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_quotient_identity_binds_carrier_and_relation_but_not_proof_implementation() {
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |carrier: &str, relation: &str, evidence: &str, reverse_relation: bool| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &public_quotient_source(carrier, relation, evidence, reverse_relation),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public quotient fixture should check");
        project_checked_package_review(&checked).expect("public quotient review should close")
    };
    let original = compile("Representative", "equivalent", "FirstEvidence", false);
    let different_evidence = compile("Representative", "equivalent", "SecondEvidence", false);
    let different_carrier = compile(
        "AlternateRepresentative",
        "equivalent",
        "FirstEvidence",
        false,
    );
    let different_relation = compile("Representative", "same_bucket", "FirstEvidence", false);
    let different_relation_body = compile("Representative", "equivalent", "FirstEvidence", true);

    let quotient = |review: &omega_compiler::CheckedPackageReviewProjection| {
        review
            .public_data()
            .iter()
            .find(|shape| shape.identity().path() == "EquivalenceClass")
            .cloned()
            .expect("public quotient row")
    };
    let original_quotient = quotient(&original);
    let PackageReviewDataKind::Quotient { carrier, relation } = original_quotient.kind() else {
        panic!("EquivalenceClass must project as quotient data")
    };
    assert!(!carrier.canonical().is_empty());
    assert_eq!(relation.path(), "equivalent");
    assert_eq!(
        original_quotient,
        quotient(&different_evidence),
        "switching one valid equivalence proof implementation must not change public quotient identity"
    );
    assert_ne!(original_quotient, quotient(&different_carrier));
    assert_ne!(original_quotient, quotient(&different_relation));
    assert_eq!(
        original_quotient,
        quotient(&different_relation_body),
        "the relation declaration row, rather than a duplicate body, belongs in the quotient row"
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original quotient review bytes"),
        different_relation_body
            .canonical_review_bytes()
            .expect("changed relation review bytes"),
        "the public proposition row must bind a changed relation body"
    );
}

#[test]
fn public_quotient_review_rederives_formation_instead_of_trusting_typed_metadata() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &public_quotient_source("Representative", "equivalent", "Evidence", false),
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public quotient fixture should check");
    let evidence_symbol = checked
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "Evidence")
        })
        .map(|conformance| conformance.symbol)
        .expect("selected quotient evidence conformance");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == evidence_symbol
            )
    }));
    checked
        .typed
        .tables
        .data_definitions
        .for_each_mut(|_, definition| {
            if definition.name.as_str() == "EquivalenceClass" {
                definition
                    .quotient
                    .as_mut()
                    .expect("quotient metadata")
                    .relation_symbol = psi_symbols::SymbolHandle::invalid();
            }
        });

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("malformed retained quotient metadata must fail independent formation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("must resolve to one exact proposition family")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_quotient_package_compilation_requires_a_public_relation() {
    let package = TempPackage::new();
    let source = public_quotient_source("Representative", "equivalent", "Evidence", false)
        .replacen("pub proposition equivalent", "proposition equivalent", 1);
    package.write("main.omg", &source);
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect_err("a public quotient cannot omit its relation semantics from review");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private proposition `equivalent`")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_domain_shape_changes_change_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Ready;\n",
    );
    second.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Prepared;\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("public-domain review should close")
            .canonical_review_bytes()
            .expect("public-domain encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_domain_semantic_roles_project_from_exact_typed_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain i32::Degrees;
pub domain i32::Radians;

pub operator + add(
    left: i32 in Degrees,
    right: i32 in Degrees
) -> i32 in Degrees;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("public semantic-role fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("exact typed semantic roles should project");
    let degrees = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "i32::Degrees")
        .expect("public Degrees domain row");
    assert_eq!(
        degrees.semantic_roles(),
        &[PackageReviewDomainSemanticRole::DenotationDimension]
    );
    let radians = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "i32::Radians")
        .expect("public Radians domain row");
    assert!(radians.semantic_roles().is_empty());
    assert!(review.public_operators().iter().any(|operator| {
        operator.coordinate().identity().path().ends_with("::add")
            || operator.coordinate().identity().path() == "add"
    }));

    let mut role_removed = checked.clone();
    role_removed
        .typed
        .domain_definitions
        .for_each_mut(|_, domain| {
            if domain.name.as_str() == "i32::Degrees" {
                domain.semantic_roles.denotation_dimension = None;
            }
        });
    let role_removed = project_checked_package_review(&role_removed)
        .expect("an absent semantic role remains a coherent distinct declaration");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        role_removed.canonical_review_bytes().unwrap(),
        "semantic-role presence must change canonical package-review identity"
    );

    let wrong_identity = checked
        .typed
        .domain_definitions
        .iter()
        .find(|(_, domain)| domain.name.as_str() == "i32::Radians")
        .expect("typed Radians declaration")
        .1
        .semantic_id;
    let mut spoofed = checked.clone();
    spoofed.typed.domain_definitions.for_each_mut(|_, domain| {
        if domain.name.as_str() == "i32::Degrees" {
            domain.semantic_roles.denotation_dimension = Some(wrong_identity);
        }
    });
    let diagnostics = project_checked_package_review(&spoofed)
        .expect_err("a semantic role pointing at another typed domain must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("semantic role does not name its exact typed semantic identity")
    }));
}

#[test]
fn public_domain_generic_binders_are_alpha_normalized() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Carrier, const Index: Unit> Carrier::Tagged<Index>;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Value, const Tag: Unit> Value::Tagged<Tag>;
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("generic public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("generic public-domain review should close")
            .canonical_review_bytes()
            .expect("generic public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_classification_and_establishment_routes_are_exact_review_rows() {
    let classified = TempPackage::new();
    let routed = TempPackage::new();
    classified.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    routed.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    classified.write("build.omg", build);
    routed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("routed public-domain fixture should check");
        project_checked_package_review(&checked).expect("routed public-domain review should close")
    };
    let classified_review = compile(&classified);
    let [domain] = classified_review.public_domains() else {
        panic!("one classified public domain row")
    };
    assert_eq!(
        domain.classification(),
        Some(PackageReviewDomainClassification::ProgressProfile)
    );
    let [route] = domain.establishment_routes() else {
        panic!("one exact establishment route")
    };
    assert_eq!(
        route.kind(),
        PackageReviewDomainEstablishmentKind::BoundaryRequirement
    );
    assert_eq!(route.trait_identity().path(), "SchedulerAdmission");
    assert!(
        route
            .requirement_identity()
            .path()
            .starts_with("named-callable(")
    );
    assert!(
        route
            .requirement_identity()
            .path()
            .contains("SchedulerAdmission::grant")
    );

    assert_ne!(
        classified_review
            .canonical_review_bytes()
            .expect("classified public-domain encoding"),
        compile(&routed)
            .canonical_review_bytes()
            .expect("unclassified routed public-domain encoding")
    );
}

#[test]
fn public_domain_establishment_route_order_is_canonical() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    let source = |routes: &str| {
        format!(
            r#"pub data SchedulerHandle {{ id: u64; }}
pub domain SchedulerHandle::Scheduled
established by {routes};
pub boundary trait PrimaryAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
pub boundary trait BackupAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
"#
        )
    };
    first.write(
        "main.omg",
        &source("PrimaryAdmission::grant, BackupAdmission::grant"),
    );
    second.write(
        "main.omg",
        &source("BackupAdmission::grant, PrimaryAdmission::grant"),
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("multi-route public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-route public-domain review should close")
            .canonical_review_bytes()
            .expect("multi-route public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_aliases_flatten_to_canonical_package_qualified_atoms() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Connected & Socket::Trusted;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Trusted & Socket::Connected;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-domain alias fixture should check");
        project_checked_package_review(&checked).expect("public-domain alias review should close")
    };
    let first_review = compile(&first);
    let usable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Socket::Usable")
        .expect("usable alias row");
    let usable_atoms = usable.alias_expansion().expect("usable alias expansion");
    assert_eq!(
        usable_atoms
            .iter()
            .map(|atom| match atom {
                PackageReviewDomainAliasAtom::Declared(identity) => identity.path(),
                PackageReviewDomainAliasAtom::Carry(_) => panic!("ordinary domain became carry"),
            })
            .collect::<Vec<_>>(),
        ["Socket::Authenticated", "Socket::Connected"]
    );
    assert!(usable_atoms.iter().all(|atom| {
        matches!(
            atom,
            PackageReviewDomainAliasAtom::Declared(identity)
                if identity.owner() == PackageReviewNominalOwner::Package(package_identity())
        )
    }));

    let portable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "u64::Portable")
        .expect("portable alias row");
    let portable_atoms = portable
        .alias_expansion()
        .expect("portable alias expansion");
    assert_eq!(
        portable_atoms,
        &psi_language_semantics::CarryPermission::ALL.map(PackageReviewDomainAliasAtom::Carry)
    );

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first alias encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("reordered alias encoding")
    );
}

#[test]
fn public_trait_shape_retains_boundary_parent_and_alpha_normalized_requirements() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    let first_source = r#"pub trait Parent<Element> {
    operator < compare(left: Element, right: Element) -> bool;
}
pub boundary trait Service<Element>: Parent<Element> {
    machine Self::exchange(&mut self, item: Element) -> Element;
}
"#;
    first.write("main.omg", first_source);
    second.write(
        "main.omg",
        r#"pub trait Parent<Value> {
    operator < compare(left: Value, right: Value) -> bool;
}
pub boundary trait Service<Value>: Parent<Value> {
    machine Self::exchange(&mut self, item: Value) -> Value;
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-trait fixture should check");
        project_checked_package_review(&checked).expect("public-trait review should close")
    };
    let first_review = compile(&first);
    let parent_shape = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    let [compare] = parent_shape.requirements() else {
        panic!("one fixed-operator requirement")
    };
    assert_eq!(
        compare.spelling(),
        Some(psi_language_core::OperatorSpelling::Less)
    );
    let service = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Service")
        .expect("service trait row");
    assert!(service.is_boundary());
    assert_eq!(service.type_parameters().len(), 1);
    let [parent] = service.parents() else {
        panic!("one exact parent edge")
    };
    assert_eq!(
        parent.kind(),
        psi_typed_trees::trait_definition::TraitCompositionKind::Policy
    );
    assert_eq!(parent.identity().path(), "Parent");
    assert_eq!(parent.arguments().len(), 1);
    let canonical_rows = first_review
        .canonical_rows()
        .expect("public-trait canonical rows");
    let service_row = canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .find(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    let start = usize::try_from(location.start_byte()).unwrap();
                    let end = usize::try_from(location.end_byte()).unwrap();
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && &first_source[start..end] == "Service"
                })
            })
        })
        .expect("service canonical row");
    let locations = service_row
        .source()
        .authored_locations()
        .expect("service declaration and parent source");
    assert!(locations.iter().any(|location| {
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        location.role() == PackageReviewSourceLocationRole::TraitParent
            && &first_source[start..end] == "Parent"
    }));
    let recovered_service_row = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(service_row).expect("encode service review row"),
    )
    .expect("recover service review row");
    assert!(
        recovered_service_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::TraitParent
                    && &first_source[start..end] == "Parent"
            }))
    );
    let [exchange] = service.requirements() else {
        panic!("one exact requirement row")
    };
    assert!(exchange.identity().path().starts_with("named-callable("));
    assert!(exchange.identity().path().contains("Service::exchange"));
    assert!(exchange.spelling().is_none());
    assert!(exchange.type_parameters().is_empty());
    let [receiver, item] = exchange.parameters() else {
        panic!("receiver and item parameters")
    };
    assert!(receiver.is_self());
    assert!(receiver.is_mutable());
    assert!(!receiver.is_const());
    assert!(receiver.type_identity().canonical().contains("trait-self"));
    assert_eq!(item.name(), "item");
    assert!(!item.is_self());
    assert_eq!(
        item.type_identity().canonical(),
        exchange.return_type().canonical()
    );

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first public-trait encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("renamed-binder public-trait encoding")
    );
}

#[test]
fn public_lifetime_contracts_are_alpha_normalized_and_relationship_sensitive() {
    let first = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'right [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'source [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub data View<'primary, 'secondary> {
    first: &'primary [u8];
    second: &'secondary [u8];
}
pub trait Parent<'origin> {
    machine borrow<'scratch>(source: &'origin [u8], temporary: &'scratch [u8]) -> &'origin [u8];
}
pub trait Child<'region>: Parent<'region> { }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'left [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'temporary [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public lifetime fixture should check");
        project_checked_package_review(&checked).expect("public lifetime review should close")
    };
    let first_review = compile(&first);
    let view = first_review
        .public_data()
        .iter()
        .find(|shape| shape.identity().path() == "View")
        .expect("view data row");
    assert_eq!(view.lifetime_parameter_count(), 2);
    let [
        PackageReviewDataMember::Field(first_field),
        PackageReviewDataMember::Field(second_field),
    ] = view.members()
    else {
        panic!("two view fields")
    };
    assert_ne!(
        first_field.type_identity().canonical(),
        second_field.type_identity().canonical()
    );

    let parent = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    assert_eq!(parent.lifetime_parameter_count(), 1);
    let [borrow] = parent.requirements() else {
        panic!("borrow requirement")
    };
    assert_eq!(borrow.lifetime_parameter_count(), 1);
    let [source, temporary] = borrow.parameters() else {
        panic!("borrow parameters")
    };
    assert_ne!(
        source.type_identity().canonical(),
        temporary.type_identity().canonical()
    );
    assert_eq!(
        source.type_identity().canonical(),
        borrow.return_type().canonical()
    );

    let child = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Child")
        .expect("child trait row");
    let [parent_edge] = child.parents() else {
        panic!("parent edge")
    };
    assert_eq!(parent_edge.lifetime_arguments(), &[0]);

    let first_bytes = first_review
        .canonical_review_bytes()
        .expect("first lifetime encoding");
    assert_eq!(
        first_bytes,
        compile(&renamed)
            .canonical_review_bytes()
            .expect("renamed lifetime encoding")
    );
    assert_ne!(
        first_bytes,
        compile(&changed)
            .canonical_review_bytes()
            .expect("changed lifetime encoding")
    );
}

#[test]
fn public_trait_lifetime_declarations_validate_before_review() {
    for (source, expected) in [
        (
            "pub trait Parent<'left, 'right> { }\npub trait Child<'child>: Parent<'child> { }\n",
            "expects 2 lifetime argument(s), got 1",
        ),
        (
            "pub trait Parent<'source> { }\npub trait Child<'child>: Parent<'ghost> { }\n",
            "uses undeclared lifetime argument `'ghost'",
        ),
        (
            "pub trait Parent<'source> { machine borrow<'source>(value: &'source [u8]) -> &'source [u8]; }\n",
            "redeclares inherited lifetime `'source'",
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect_err("invalid parent lifetime application must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    }
}

#[test]
fn review_projects_public_data_invariants_from_exact_checked_rows() {
    let compile = |facts: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Ledger
where
{facts}
{{
    len: u32;
    count: u32;
}}
"#
            ),
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public data invariant fixture should check");
        project_checked_package_review(&checked)
            .expect("review should project the checked public data invariant")
    };

    let review = compile("    count <= len,");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = data.invariants()
    else {
        panic!("one binary data invariant")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::LessOrEqual);
    for (expression, expected) in [
        (left.as_ref(), "Ledger::count"),
        (right.as_ref(), "Ledger::len"),
    ] {
        let PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } = expression
        else {
            panic!("data-subject field")
        };
        assert_eq!(
            receiver.as_ref(),
            &PackageReviewContractExpression::DomainSubject
        );
        assert_eq!(member.path(), expected);
        assert!(case_variant.is_none());
    }
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        compile("    count < len,")
            .canonical_review_bytes()
            .unwrap(),
        "changing a public data invariant must change canonical package identity"
    );
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        compile("    count <= len,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "duplicate invariant observations must normalize to one canonical fact"
    );
    assert_eq!(
        compile("    count <= len,\n    count <= 8,")
            .canonical_review_bytes()
            .unwrap(),
        compile("    count <= 8,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "authored invariant order must not change canonical package identity"
    );
}

#[test]
fn public_data_invariants_keep_generic_binders_distinct_from_fields() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("generic data invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("generic data invariant should retain its binder identity");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            left, ..
        }),
    ] = data.invariants()
    else {
        panic!("one generic data invariant")
    };
    assert_eq!(
        left.as_ref(),
        &PackageReviewContractExpression::GenericBinder(0)
    );
}

#[test]
fn public_data_membership_invariants_keep_exact_field_and_domain_identity() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u32::Small
requires self <= 8;

pub data Counter
where count in u32::Small,
{
    count: u32;
}
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("data membership invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("data membership invariant should retain exact identities");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [PackageReviewContractFact::Membership { value, domain }] = data.invariants() else {
        panic!("one membership invariant")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = value
    else {
        panic!("membership value projects the data field")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Counter::count");
    assert_eq!(domain.path(), "u32::Small");
}

#[test]
fn public_data_invariant_review_rejects_checked_ownership_spoofs() {
    let compile = || {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Ledger
where count <= len,
{
    len: u32;
    count: u32;
}
"#,
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public data ownership fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked data ownership must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    };

    let mut missing_owner = compile();
    let owner = missing_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .data_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "data invariant evidence");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "data invariant evidence");

    let mut unrelated_extra_owner = compile();
    let mut owner = unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    owner.semantic_fact = Default::default();
    unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&unrelated_extra_owner, "data invariant evidence");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = psi_facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "data invariant evidence");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "data invariant evidence");

    let mut extra_dependency = compile();
    let owner = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    let dependency = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get(owner)
        .dependencies[0];
    extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&extra_dependency, "data invariant evidence");

    let mut orphan_semantic_fact = compile();
    let semantic_fact = orphan_semantic_fact
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    let fact = *orphan_semantic_fact.facts.semantic.facts.get(semantic_fact);
    orphan_semantic_fact.facts.semantic.facts.append(fact);
    assert_rejects(&orphan_semantic_fact, "data invariant evidence");

    let mut orphan_ref = compile();
    let semantic_fact = orphan_ref
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    orphan_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: semantic_fact,
    });
    assert_rejects(&orphan_ref, "data invariant evidence");

    let mut dangling_ref = compile();
    dangling_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: psi_arena::Handle::from_parts(u32::MAX, 1),
    });
    assert_rejects(&dangling_ref, "data invariant evidence");

    let mut malformed_extra_context = compile();
    malformed_extra_context
        .facts
        .semantic
        .contexts
        .append(psi_facts::FactContext {
            point: psi_facts::ProgramPoint::Global,
            facts: psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1),
        });
    assert_rejects(&malformed_extra_context, "data invariant evidence");

    let mut missing_context = compile();
    let context = missing_context
        .facts
        .semantic
        .contexts
        .iter()
        .find_map(|(handle, context)| {
            matches!(context.point, psi_facts::ProgramPoint::Definition { .. }).then_some(handle)
        })
        .expect("data fact context");
    assert!(missing_context.facts.semantic.contexts.free(context));
    assert_rejects(&missing_context, "data invariant evidence");

    let mut missing_symbol_set = compile();
    let symbol_set = missing_symbol_set
        .facts
        .semantic
        .symbol_sets
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data symbol fact set");
    assert!(
        missing_symbol_set
            .facts
            .semantic
            .symbol_sets
            .free(symbol_set)
    );
    assert_rejects(&missing_symbol_set, "data invariant evidence");

    let mut malformed_empty_path = {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
"#,
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("generic data ownership fixture should check")
    };
    let binder_place = malformed_empty_path
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.dependencies[0].place)
        .expect("generic binder dependency");
    assert!(
        malformed_empty_path
            .facts
            .semantic
            .places
            .get(binder_place)
            .segments
            .is_empty()
    );
    malformed_empty_path
        .facts
        .semantic
        .places
        .get_mut(binder_place)
        .segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1);
    assert_rejects(&malformed_empty_path, "data invariant evidence");
}

#[test]
fn review_projects_public_domain_predicates_from_exact_checked_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Ready
    requires self.value == 0;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public domain fact fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("review should project the checked public domain predicate");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    assert_eq!(
        domain.predicate_body(),
        psi_language_semantics::DomainPredicateBody::Present
    );
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one binary domain predicate fact")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    let PackageReviewContractExpression::Member {
        receiver,
        member,
        case_variant,
    } = left.as_ref()
    else {
        panic!("domain-subject member path")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Packet::value");
    assert!(case_variant.is_none());
    assert_eq!(
        right.as_ref(),
        &PackageReviewContractExpression::Integer("0".to_owned())
    );
}

#[test]
fn review_projects_exact_compiler_byte_sequence_predicate_identity() {
    let project = |predicate: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub domain [u8]::ReviewedBytes\nrequires\n    {predicate}(self);\n"),
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("compiler-owned byte predicate should check");
        project_checked_package_review(&checked)
            .expect("compiler-owned byte predicate should have exact review identity")
    };

    let mut encodings = Vec::new();
    for (name, expected) in [
        ("valid_utf8", PackageReviewByteSequencePredicate::ValidUtf8),
        ("no_nul", PackageReviewByteSequencePredicate::NoNul),
        ("ascii_only", PackageReviewByteSequencePredicate::AsciiOnly),
        ("non_empty", PackageReviewByteSequencePredicate::NonEmpty),
    ] {
        let review = project(name);
        let [domain] = review.public_domains() else {
            panic!("one byte-domain row")
        };
        let [
            PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
                target,
                static_arguments,
                arguments,
                ..
            }),
        ] = domain.predicate_facts()
        else {
            panic!("one byte-predicate call")
        };
        assert_eq!(target.byte_sequence_predicate(), Some(expected));
        assert!(static_arguments.is_empty());
        assert_eq!(arguments, &[PackageReviewContractExpression::DomainSubject]);
        encodings.push(review.canonical_review_bytes().unwrap());
    }
    encodings.sort();
    encodings.dedup();
    assert_eq!(
        encodings.len(),
        4,
        "each exact compiler predicate must have distinct package-review identity"
    );
}

#[test]
fn review_projects_exact_raw_byte_literals_in_public_contracts() {
    let project = |literal: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("pub domain [u8]::LiteralCheck\nrequires\n    no_nul({literal});\n"),
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("raw-byte contract literal should check");
        let review = project_checked_package_review(&checked)
            .expect("raw-byte contract literal should project exactly");
        let [domain] = review.public_domains() else {
            panic!("one raw-byte domain row")
        };
        let [
            PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
                target,
                arguments,
                ..
            }),
        ] = domain.predicate_facts()
        else {
            panic!("one raw-byte predicate call")
        };
        assert_eq!(
            target.byte_sequence_predicate(),
            Some(PackageReviewByteSequencePredicate::NoNul)
        );
        let [argument] = arguments.as_slice() else {
            panic!("one exact raw-byte argument")
        };
        let row = review
            .canonical_rows()
            .expect("canonical raw-byte rows")
            .into_iter()
            .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicDomain)
            .expect("public raw-byte domain row");
        (argument.clone(), row.canonical_bytes().to_vec())
    };

    let escaped_ascii = project(r#""\x41""#);
    let direct_ascii = project(r#""A""#);
    let opaque_octet = project(r#""\xFF""#);

    assert_eq!(
        escaped_ascii.0,
        PackageReviewContractExpression::ByteSequence(vec![b'A'])
    );
    assert_eq!(escaped_ascii, direct_ascii);
    assert_eq!(
        opaque_octet.0,
        PackageReviewContractExpression::ByteSequence(vec![0xff])
    );
    assert_ne!(escaped_ascii.1, opaque_octet.1);
}

#[test]
fn review_projects_ordered_nested_array_contract_expressions() {
    let project = |literal: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub proposition matrix(values: [[i32; 2]; 2]);
pub machine consume()
requires matrix({literal})
{{ }}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("nested array contract fixture should check");
        project_checked_package_review(&checked)
            .expect("nested array contract expression should project in order")
    };

    let original = project("[[1, 2], [3, 4]]");
    let consume = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public array consumer");
    let [contract] = consume.contracts() else {
        panic!("one array-bearing requirement")
    };
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("one proposition application")
    };
    assert_eq!(
        application.arguments(),
        [PackageReviewContractExpression::Array(vec![
            PackageReviewContractExpression::Array(vec![
                PackageReviewContractExpression::Integer("1".to_owned()),
                PackageReviewContractExpression::Integer("2".to_owned()),
            ]),
            PackageReviewContractExpression::Array(vec![
                PackageReviewContractExpression::Integer("3".to_owned()),
                PackageReviewContractExpression::Integer("4".to_owned()),
            ]),
        ])]
    );
    let reordered = project("[[2, 1], [3, 4]]");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "array element order is semantic contract identity",
    );

    let nested = TempPackage::new();
    nested.write(
        "main.omg",
        r#"pub proposition values(items: [i32; 1]);
pub machine consume(source: [i32; 1])
requires values([source[0]])
{ }
"#,
    );
    nested.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &nested.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&nested.0),
    )
    .expect("array containing an indexed expression should check");
    let nested_review = project_checked_package_review(&checked)
        .expect("array containing an indexed expression should project");
    let consume = nested_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public nested-index consumer");
    let [contract] = consume.contracts() else {
        panic!("one nested-index requirement")
    };
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("one nested-index proposition application")
    };
    assert_eq!(
        application.arguments(),
        [PackageReviewContractExpression::Array(vec![
            PackageReviewContractExpression::Indexed {
                meaning: PackageReviewContractOperatorMeaning::Builtin,
                collection: Box::new(PackageReviewContractExpression::Parameter(0)),
                index: Box::new(PackageReviewContractExpression::Integer("0".to_owned())),
            },
        ])]
    );
}

#[test]
fn review_projects_exact_nominal_record_and_case_constructors() {
    let project = |point_fields: &str, case: &str, case_fields: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Point [copy] {{ x: i32; y: i32; }}
pub data Outcome [copy] {{
    code: u64;
    case Success(value: u64);
    case Failure(value: u64);
}}
pub proposition has_point(value: Point);
pub proposition has_outcome(value: Outcome);
pub machine consume()
requires has_point(Point {{ {point_fields} }})
requires has_outcome(Outcome::{case} {{ {case_fields} }})
{{ }}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("nominal constructor contract fixture should check");
        project_checked_package_review(&checked)
            .expect("nominal constructors should project by exact declaration identity")
    };

    let original = project("x: 1, y: 2", "Success", "code: 3u64, value: 4u64");
    let consume = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public constructor consumer");
    assert_eq!(consume.contracts().len(), 2);
    let point = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_point" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("record constructor argument");
    let PackageReviewContractExpression::Constructor { data, case, fields } = point else {
        panic!("one exact record constructor")
    };
    assert_eq!(data.path(), "Point");
    assert_eq!(
        data.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(case.is_none());
    assert_eq!(fields.len(), 2);
    assert!(fields[0].field().path() < fields[1].field().path());

    let outcome = consume
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Proposition(application)
                if application.declaration().path() == "has_outcome" =>
            {
                application.arguments().first()
            }
            _ => None,
        })
        .expect("case constructor argument");
    let PackageReviewContractExpression::Constructor {
        data,
        case: Some(case),
        fields,
    } = outcome
    else {
        panic!("one exact sum-case constructor")
    };
    assert_eq!(data.path(), "Outcome");
    assert!(case.path().contains("Success"));
    assert_eq!(fields.len(), 2, "record and selected-case payload fields");

    let reordered = project("y: 2, x: 1", "Success", "value: 4u64, code: 3u64");
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "constructor field spelling order must canonicalize by exact field identity",
    );
    let changed_case = project("x: 1, y: 2", "Failure", "code: 3u64, value: 4u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_case.canonical_review_bytes().unwrap(),
        "changing the exact selected case must change review identity",
    );
    let changed_value = project("x: 1, y: 2", "Success", "code: 3u64, value: 5u64");
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a constructor field value must change review identity",
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"data Hidden [copy] { value: u64; }
pub proposition hidden(value: Hidden);
pub machine consume()
requires hidden(Hidden { value: 1u64 })
{ }
"#,
    );
    private.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect_err("a public contract must reject a private constructor before review");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("public interface selects private data `Hidden`")
    }));
}

#[test]
fn review_projects_checked_index_and_range_contract_expressions() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[0]),
    window(values[0..1])
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("indexed public contract fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked index and range expressions should project");
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("inspect"))
        .expect("public indexed-contract callable");
    let [selected, window] = inspect.contracts() else {
        panic!("two indexed requirements")
    };

    let PackageReviewContractFact::Proposition(selected) = selected.fact() else {
        panic!("selected proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = selected.arguments()
    else {
        panic!("selected argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Integer("0".to_owned())
    );

    let PackageReviewContractFact::Proposition(window) = window.fact() else {
        panic!("window proposition application")
    };
    let [
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        },
    ] = window.arguments()
    else {
        panic!("window argument is one indexed expression")
    };
    assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(**collection, PackageReviewContractExpression::Parameter(0));
    assert_eq!(
        **index,
        PackageReviewContractExpression::Range {
            start: Some(Box::new(PackageReviewContractExpression::Integer(
                "0".to_owned(),
            ))),
            end: Some(Box::new(PackageReviewContractExpression::Integer(
                "1".to_owned(),
            ))),
            end_inclusive: false,
        }
    );
    let baseline_bytes = review
        .canonical_review_bytes()
        .expect("indexed contract review must encode canonically");

    package.write(
        "main.omg",
        r#"pub proposition selected(value: i32);
pub proposition window(values: &[i32]);
pub machine inspect(values: [i32; 2])
requires
    selected(values[1]),
    window(values[0..=1])
{ }
"#,
    );
    let changed = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("changed indexed public contract fixture should check");
    let changed_bytes = project_checked_package_review(&changed)
        .expect("changed checked index and range expressions should project")
        .canonical_review_bytes()
        .expect("changed indexed contract review must encode canonically");
    assert_ne!(baseline_bytes, changed_bytes);
}

#[test]
fn review_projects_exact_zero_value_targets_in_public_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let source = |binder: &str, family: &str| {
        format!(
            r#"pub data Optional<Element> {{ case #0 None; }}
pub data Alternate<Element> {{ case #0 None; }}
pub proposition zero_is_none<{binder}>() =
    zero_value<{family}<{binder}>>() == zero_value<{family}<{binder}>>();
"#
        )
    };
    let project = |source: String| {
        package.write("main.omg", &source);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("zero-value public proposition should check");
        project_checked_package_review(&checked)
            .expect("zero-value public proposition should project exactly")
    };

    let first = project(source("Item", "Optional"));
    let proposition = first
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "zero_is_none")
        .expect("public zero-value proposition row");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("one transparent zero-value equality")
    };
    let PackageReviewContractExpression::ZeroValue(target_type) = left.as_ref() else {
        panic!("the proof-only observation retains its exact target type")
    };
    assert!(target_type.canonical().contains("Optional"));
    let first_bytes = first.canonical_review_bytes().unwrap();

    let renamed = project(source("Value", "Optional"));
    assert_eq!(first_bytes, renamed.canonical_review_bytes().unwrap());
    let changed = project(source("Item", "Alternate"));
    assert_ne!(first_bytes, changed.canonical_review_bytes().unwrap());
}

#[test]
fn package_callable_wins_over_compiler_byte_predicate_spelling_in_review() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine valid_utf8(value: &[u8]) -> bool { true }
pub domain [u8]::ReviewedBytes
requires
    valid_utf8(self);
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("package callable lookalike should check as an ordinary call");
    let review = project_checked_package_review(&checked)
        .expect("package callable lookalike should retain nominal identity");
    let [domain] = review.public_domains() else {
        panic!("one byte-domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            target, ..
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one nominal predicate call")
    };
    let nominal = target
        .nominal()
        .expect("package declaration must remain nominal");
    assert_eq!(nominal.path(), "valid_utf8::entry");
    assert_eq!(
        nominal.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_domain_predicate_review_rejects_checked_owner_and_dependency_spoofs() {
    let compile = || {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Packet { value: u32; }
pub domain Packet::Ready
    requires self.value == 0;
"#,
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public domain spoof fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked domain ownership must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    };

    let mut missing_owner = compile();
    let owner = missing_owner
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .domain_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "0 exact checked ownership records");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("domain ownership record");
    duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "2 exact checked ownership records");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = psi_facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "0 exact checked definition rows");

    let mut false_evidence = compile();
    let semantic_fact = false_evidence
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    false_evidence
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .evidence
        .receipt_identity = 1;
    assert_rejects(&false_evidence, "0 exact checked definition rows");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "0 exact checked dependency records");

    let mut duplicate_dependency = compile();
    let owner = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    let dependency = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get(owner)
        .dependencies[0];
    duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&duplicate_dependency, "2 exact checked dependency records");

    let mut wrong_member = compile();
    let dependency = wrong_member
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .and_then(|(_, record)| record.dependencies.first().copied())
        .expect("domain member dependency");
    let segment = wrong_member
        .facts
        .semantic
        .places
        .get(dependency.place)
        .segments
        .start();
    wrong_member
        .facts
        .semantic
        .place_segments
        .get_mut(segment)
        .clone_from(&psi_facts::PlaceSegment::Field {
            symbol: psi_symbols::SymbolHandle::invalid(),
        });
    assert_rejects(&wrong_member, "0 exact checked dependency records");
}

#[test]
fn review_projects_public_domain_membership_predicates_and_rejects_private_targets() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public domain membership fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public domain membership review should close");
    let ready = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Packet::Ready")
        .expect("ready domain row");
    let [PackageReviewContractFact::Membership { value, domain }] = ready.predicate_facts() else {
        panic!("one exact membership predicate")
    };
    assert_eq!(value, &PackageReviewContractExpression::DomainSubject);
    assert_eq!(domain.path(), "Packet::Base");

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
    );
    private.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public predicate");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `Packet::Base`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_domain_predicate_fact_order_is_canonical_but_content_changes_encoding() {
    let first = TempPackage::new();
    let reordered = TempPackage::new();
    let changed = TempPackage::new();
    let source = |facts: &str| {
        format!(
            "pub data Packet {{ value: u32; }}\npub domain Packet::Ready\nrequires\n    {facts}\n"
        )
    };
    first.write("main.omg", &source("self.value == 0; self.value <= 1;"));
    reordered.write("main.omg", &source("self.value <= 1; self.value == 0;"));
    changed.write("main.omg", &source("self.value == 0; self.value <= 2;"));
    let build = "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n";
    first.write("build.omg", build);
    reordered.write("build.omg", build);
    changed.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("multi-fact public domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-fact public domain review should close")
            .canonical_review_bytes()
            .expect("multi-fact public domain encoding")
    };
    assert_eq!(encode(&first), encode(&reordered));
    assert_ne!(encode(&first), encode(&changed));
}

#[test]
fn review_projects_callable_domain_predicates_through_exact_checked_selection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading {
    value: i64;
    minimum: i64;
    maximum: i64;
}
pub machine within_calibration(reading: Reading) -> bool {
    reading.value >= reading.minimum && reading.value <= reading.maximum
}
pub domain Reading::Calibrated
requires
    within_calibration(self);
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("callable domain predicate fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("simple checked callable predicates have an exact review row");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            arguments,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one callable domain predicate")
    };
    assert!(receiver.is_none());
    assert_eq!(
        target.nominal().expect("ordinary callable target").path(),
        "within_calibration::entry"
    );
    assert!(static_arguments.is_empty());
    assert_eq!(arguments, &[PackageReviewContractExpression::DomainSubject]);
}

#[test]
fn callable_domain_predicate_review_rejects_checked_target_spoof() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading { value: i64; }
pub machine is_zero(reading: Reading) -> bool { reading.value == 0 }
pub domain Reading::Zero requires is_zero(self);
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("callable predicate spoof fixture should check");
    let call_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Call(_))
                .then_some(expression)
        })
        .expect("domain predicate call expression");
    let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        panic!("call expression")
    };
    call.target_symbol = psi_symbols::SymbolHandle::invalid();

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a typed call target cannot diverge from checked selection custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target disagrees with its exact checked call-selection row")
    }));
}

#[test]
fn public_trait_operational_envelope_is_exact_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Console { }
pub boundary trait Worker {
    machine wait(handler: &mut Console)
    reaches <= Console
    invokes handler;
    invokes Console;
    suspends;
    blocks;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait suspension fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait operational review should close");
    let worker = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [wait] = worker.requirements() else {
        panic!("one worker requirement")
    };
    let [console] = wait.service_reach() else {
        panic!("one exact service-reach row")
    };
    assert_eq!(console.path(), "Console");
    assert_eq!(
        console.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(wait.service_reach_is_installation_bound());
    assert_eq!(wait.synchronous_invocations().len(), 2);
    assert_eq!(wait.synchronous_invocations()[0].parameter(), Some(0));
    assert_eq!(
        wait.synchronous_invocations()[1]
            .service()
            .expect("service invocation")
            .path(),
        "Console"
    );
    assert!(wait.suspends());
    assert!(wait.blocks());
}

#[test]
fn public_trait_termination_is_parameter_rooted_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait SchedulerRuntime {
    machine wait(&self, scheduler: SchedulerRuntime)
    requires self in WeakFair
    requires scheduler in WeakFair
    terminates;
}
pub domain SchedulerRuntime::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait termination fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait termination review should close");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| {
            requirement
                .identity()
                .path()
                .contains("SchedulerRuntime::wait")
        })
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    assert_eq!(premises.len(), 2);
    for premise in premises {
        assert_eq!(premise.profile().path(), "SchedulerRuntime::WeakFair");
        assert_eq!(
            premise.profile().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(premise.projections().is_empty());
    }
    assert!(premises[0].subject().is_receiver());
    assert_eq!(premises[1].subject().parameter(), Some(0));
}

#[test]
fn public_trait_termination_rejects_a_non_public_progress_profile() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect_err("ordinary visibility must reject a private profile in a public trait contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `SchedulerHandle::WeakFair`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_trait_defaults_and_unnamed_contracts() {
    let default_package = TempPackage::new();
    default_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait() { }
}
"#,
    );
    default_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &default_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&default_package.0),
    )
    .expect("public trait default fixture should check");
    let default_review = project_checked_package_review(&checked)
        .expect("review should retain a public trait default realization");
    let default_requirement = &default_review.public_traits()[0].requirements()[0];
    assert!(default_requirement.has_default_realization());

    let abstract_package = TempPackage::new();
    abstract_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait();
}
"#,
    );
    abstract_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let abstract_checked = compile_to_checked_with_packages(
        &abstract_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&abstract_package.0),
    )
    .expect("abstract public trait fixture should check");
    let abstract_review = project_checked_package_review(&abstract_checked)
        .expect("review should retain an abstract public trait requirement");
    assert!(!abstract_review.public_traits()[0].requirements()[0].has_default_realization());
    assert_ne!(
        default_review.canonical_review_bytes().unwrap(),
        abstract_review.canonical_review_bytes().unwrap(),
    );

    let precondition_package = TempPackage::new();
    precondition_package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair;
}
"#,
    );
    precondition_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &precondition_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&precondition_package.0),
    )
    .expect("public progress precondition fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("an unnamed trait precondition should project exactly");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let [wait] = runtime.requirements() else {
        panic!("one scheduler requirement")
    };
    let [contract] = wait.contracts() else {
        panic!("one exact trait contract")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Requires);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("trait precondition must retain exact membership")
    };
    assert_eq!(value, &PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let runtime_row = review
        .canonical_rows()
        .expect("trait precondition canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicTrait
                && row
                    .key_bytes()
                    .windows("SchedulerRuntime".len())
                    .any(|window| window == b"SchedulerRuntime")
        })
        .expect("scheduler runtime trait row");
    assert!(
        runtime_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );
}

#[test]
fn public_trait_requires_and_ensures_change_comparison_identity() {
    let project = |minimum: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Bounds {{\n    machine clamp(value: u64) -> u64\n    requires value >= {minimum}\n    ensures result >= value;\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public trait contract fixture should check");
        project_checked_package_review(&checked).expect("public trait contracts should project")
    };

    let first = project(1);
    let changed = project(2);
    let [bounds] = first.public_traits() else {
        panic!("one public trait")
    };
    let [clamp] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    assert_eq!(clamp.contracts().len(), 2);
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Requires)
    );
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Ensures)
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing a public trait contract must change comparison identity"
    );
}

#[test]
fn public_trait_named_witness_contracts_retain_exact_lanes_and_selector_identity() {
    let project = |requires_binding: &str, ensures_binding: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub trait Evidence {{
    machine witness();
}}
pub proposition ready() evidence Evidence;
pub trait Worker {{
    machine relay(value: i32) -> i32
    requires {requires_binding}: ready()
    ensures {ensures_binding}: ready()
    {{
        {ensures_binding} = {requires_binding};
    }}
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("named public-trait witness fixture should check");
        project_checked_package_review(&checked)
            .expect("named public-trait witness contracts should project")
    };

    let original = project("input_proof", "output_proof");
    let renamed_requires = project("renamed_local_input", "output_proof");
    let renamed_ensures = project("input_proof", "renamed_public_output");
    let worker = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [relay] = worker.requirements() else {
        panic!("one worker requirement")
    };
    assert_eq!(relay.contracts().len(), 2);
    for contract in relay.contracts() {
        assert!(contract.evidence_lane_position().is_some());
        let PackageReviewContractFact::Proposition(application) = contract.fact() else {
            panic!("named witness contract must retain proposition identity")
        };
        assert_eq!(application.declaration().path(), "ready");
        let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
            panic!("named witness contract must retain its evidence interface")
        };
        assert_eq!(interface.trait_identity().path(), "Evidence");
        assert_eq!(interface.requirements().len(), 1);
    }
    let requires = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Requires)
        .expect("named requires row");
    let ensures = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Ensures)
        .expect("named ensures row");
    assert_eq!(requires.binding(), None, "requires binding is local");
    assert_eq!(ensures.binding(), Some("output_proof"));
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed_requires.canonical_review_bytes().unwrap(),
        "renaming a local requires evidence alias must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        renamed_ensures.canonical_review_bytes().unwrap(),
        "renaming a public ensures selector must change review identity",
    );
}

#[test]
fn public_trait_member_contracts_join_exact_state_signature_places() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Threshold { minimum: u64; }
pub trait Bounds {
    machine accepts(threshold: Threshold, value: u64)
    requires value >= threshold.minimum;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait member contract should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait member contract should join checked places");
    let [bounds] = review.public_traits() else {
        panic!("one public trait")
    };
    let [accepts] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    let [contract] = accepts.contracts() else {
        panic!("one public trait contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("one exact binary contract expression")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = right.as_ref()
    else {
        panic!("right operand must retain the exact field place")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(member.path(), "Threshold::minimum");
    assert_eq!(
        member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_trait_crash_ceilings_are_exact_canonical_checked_routes() {
    let project = |trap_guard: &str, stop_cause: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Worker {{\n    machine run(flag: bool)\n    crashes Trap {trap_guard};\n    machine stop() crashes {stop_cause};\n    machine idle();\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public trait crash ceiling should check");
        project_checked_package_review(&checked)
            .expect("public trait crash ceiling should project from its exact checked capsule")
    };

    let first = project("flag", "Abort");
    let guard_changed = project("!flag", "Abort");
    let cause_changed = project("flag", "Trap");
    let worker = first
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let run = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::run"))
        .expect("run requirement row");
    let [trap] = run.published_crash() else {
        panic!("one guarded trap route")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    let [PackageReviewCrashRouteGuard::Predicate(predicate)] = trap.alternative_guards() else {
        panic!("trap route must retain one canonical predicate guard")
    };
    assert!(!predicate.canonical_bytes().is_empty());
    let stop = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::stop"))
        .expect("stop requirement row");
    let [abort] = stop.published_crash() else {
        panic!("one unconditional abort route")
    };
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let idle = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path().contains("Worker::idle"))
        .expect("idle requirement row");
    assert!(idle.published_crash().is_empty());
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        guard_changed.canonical_review_bytes().unwrap(),
        "changing a crash guard must change package comparison identity"
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        cause_changed.canonical_review_bytes().unwrap(),
        "changing a crash cause must change package comparison identity"
    );
}

#[test]
fn public_trait_crash_projection_rejects_missing_or_duplicate_checked_capsules() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Worker {
    machine run() crashes Trap;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait crash fixture should check");

    let mut missing = checked.clone();
    missing.facts.contract_plans.crash_capsules.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing checked crash capsule must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact checked crash capsule")
    }));

    let mut duplicate = checked;
    let capsule = duplicate.facts.contract_plans.crash_capsules[0].clone();
    duplicate.facts.contract_plans.crash_capsules.push(capsule);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate checked crash capsules must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has duplicate checked crash capsules")
    }));
}

#[test]
fn public_contract_call_projection_requires_one_exact_checked_certificate() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Pair [copy] { left: u64; right: u64; }
pub machine make_pair(left: u64, right: u64) -> Pair terminates; {
    transition { _ -> (Pair { left: left, right: right }) }
}
pub proposition projected_left(pair: Pair, expected: u64) = pair.left == expected;
pub trait Worker {
    machine observe(left: u64, right: u64) -> u64
    ensures projected_left(make_pair(left, right), left);
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public fact-call projection fixture should check");
    project_checked_package_review(&checked)
        .expect("one exact fact-call projection certificate should rejoin");

    let mut missing = checked.clone();
    missing.facts.fact_call_projections.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing fact-call projection certificate must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("fact-call projection rejoins 0 exact eligibility certificates")
    }));

    let mut duplicate = checked;
    let certificate = duplicate.facts.fact_call_projections[0].clone();
    duplicate.facts.fact_call_projections.push(certificate);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate fact-call projection certificates must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("fact-call projection rejoins 2 exact eligibility certificates")
    }));
}

#[test]
fn public_trait_contract_calls_use_the_same_checked_projection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine computed_zero() -> u64 { 0 }
pub trait Worker {
    machine wait() -> u64
    ensures result == computed_zero();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait contract call fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait contract calls use the checked call row");
    let [trait_shape] = review.public_traits() else {
        panic!("one public trait")
    };
    let [requirement] = trait_shape.requirements() else {
        panic!("one trait requirement")
    };
    let [contract] = requirement.contracts() else {
        panic!("one trait requirement contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary trait guarantee")
    };
    assert!(matches!(
        right.as_ref(),
        PackageReviewContractExpression::Call { target, .. }
            if target.nominal().is_some_and(|target| target.path() == "computed_zero::entry")
    ));
}

#[test]
fn review_rejects_contract_entailment_stand_downs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    a >= 1
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary checking should retain the out-of-language stand-down");
    let [stand_down] = checked.contract_entailment_stand_downs() else {
        panic!("one exact contract-entailment stand-down")
    };
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("package review must fail closed on an unresolved stand-down");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects unresolved contract-entailment stand-down")
    }));
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
