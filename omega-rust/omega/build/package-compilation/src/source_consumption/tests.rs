//! Source consumption tests.

use super::{
    CheckedTrees, ConsumedSourceUnit, ConsumedSourceUnitKind, GeneratedSourceOrder,
    PackageCompilationSubject, PackageSourceConsumptionCommitment, Sha256, SourceFile,
    SourceOrigin, append_field, canonical_consumed_unit_bytes, canonical_source_entry,
    consumed_source_unit, derive_consumed_source_units, derive_source_consumption_commitment,
    toolchain_source_identity_digest, verify_current_files,
};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::Digest;
use source::SourceId;
use std::path::PathBuf;
use std::sync::Arc;

fn generated_fixture() -> Vec<(SourceId, build_output::PackageGeneratedSource)> {
    let tree = build_output::from_entries(&[
        build_output::OutputTreeEntry::regular_file(b"first.omg", b"data First {}", false),
        build_output::OutputTreeEntry::regular_file(b"second.omg", b"data Second {}", false),
    ])
    .expect("retained generated files");
    build_output::select_included_sources(&tree, &[b"first.omg".to_vec(), b"second.omg".to_vec()])
        .expect("included generated files")
        .into_iter()
        .enumerate()
        .map(|(position, generated)| (SourceId(position * 2 + 1), generated))
        .collect()
}

fn checked_sources(files: Vec<SourceFile>) -> CheckedTrees {
    let mut program = CheckedTrees::default();
    program.typed.symbols = program
        .typed
        .symbols
        .begin_extension(
            Some(Arc::new(source::SourceMap::from_files(files))),
            Vec::new(),
        )
        .finish();
    program
}

fn generated_files(
    custody: &[(SourceId, build_output::PackageGeneratedSource)],
) -> Vec<SourceFile> {
    custody
        .iter()
        .map(|(source_id, generated)| {
            let mut file = source(
                &format!(
                    "/package/.omega/generated/{}",
                    String::from_utf8_lossy(generated.relative_path())
                ),
                "/package",
                Some(PackageKeyIdentity::from_digest([7; 32]).expect("package identity")),
                SourceOrigin::User,
                std::str::from_utf8(generated.bytes()).expect("generated UTF-8"),
            );
            file.source_id = *source_id;
            file
        })
        .collect()
}

#[test]
fn generated_custody_order_preserves_arbitrary_ids_and_first_duplicate() {
    let custody = generated_fixture();
    let ordered = GeneratedSourceOrder::new(&custody);
    assert!(
        ordered.positions.is_none(),
        "producer order needs no permutation"
    );
    assert!(GeneratedSourceOrder::new(&[]).positions.is_none());
    assert!(ordered.find(SourceId(0)).is_none());
    assert!(ordered.find(SourceId(2)).is_none());
    assert!(ordered.find(SourceId(usize::MAX)).is_none());
    let reordered = vec![
        custody[1].clone(),
        custody[0].clone(),
        (custody[1].0, custody[0].1.clone()),
    ];
    let ordered = GeneratedSourceOrder::new(&reordered);
    assert!(ordered.positions.is_some());
    assert_eq!(ordered.find(custody[1].0).unwrap().1, &custody[1].1);
}

#[test]
fn generated_projection_preserves_order_independence_and_custody_errors() {
    let custody = generated_fixture();
    let files = generated_files(&custody);
    let baseline = derive_consumed_source_units(&checked_sources(files.clone()), &custody).unwrap();
    let mut reversed_files = files.clone();
    reversed_files.reverse();
    let reversed_custody = vec![custody[1].clone(), custody[0].clone()];
    assert_eq!(
        baseline,
        derive_consumed_source_units(&checked_sources(reversed_files), &reversed_custody).unwrap()
    );
    let mut duplicate = custody.clone();
    duplicate.push(custody[0].clone());
    assert!(
        derive_consumed_source_units(&CheckedTrees::default(), &duplicate).unwrap_err()[0]
            .message
            .contains("duplicate frontend source IDs")
    );
    let mut missing = custody.clone();
    missing.push((SourceId(usize::MAX), custody[0].1.clone()));
    assert!(
        derive_consumed_source_units(&checked_sources(files.clone()), &missing).unwrap_err()[0]
            .message
            .contains("absent from the final checked closure")
    );
    let mut changed_content = files.clone();
    changed_content[0].source = Arc::from("changed");
    let mut changed_path = files.clone();
    changed_path[0].path = PathBuf::from("/package/.omega/generated/other.omg");
    let mut changed_root = files.clone();
    changed_root[0].package_root = PathBuf::from("/other-package");
    for (changed, expected) in [
        (changed_content, "does not match final checked source"),
        (changed_path, "path does not match final checked source"),
        (changed_root, "outside its reconciled package root"),
    ] {
        assert!(
            derive_consumed_source_units(&checked_sources(changed), &custody).unwrap_err()[0]
                .message
                .contains(expected)
        );
    }
    // Two checked instances of one path consume identical bytes: the
    // byte-level projection keeps the canonical unit once rather than
    // rejecting a dual-purpose source (wiki/spec/build/scoped_execution.md).
    let mut repeated = files.clone();
    repeated.push(files[0].clone());
    assert_eq!(
        baseline,
        derive_consumed_source_units(&checked_sources(repeated.clone()), &custody).unwrap()
    );
    let package = PackageKeyIdentity::from_digest([9; 32]).expect("package identity");
    let authored = |text: &str, id| {
        let mut file = source(
            "/package/shared.omg",
            "/package",
            Some(package),
            SourceOrigin::User,
            text,
        );
        file.source_id = SourceId(id);
        file
    };
    let colliding = vec![authored("data A {}", 40), authored("data B {}", 41)];
    assert!(
        derive_consumed_source_units(&checked_sources(colliding), &[]).unwrap_err()[0]
            .message
            .contains("distinct sources at duplicate canonical coordinates")
    );
    assert!(
        derive_consumed_source_units(&checked_sources(repeated), &missing).unwrap_err()[0]
            .message
            .contains("absent from the final checked closure"),
        "repeated source IDs must not hide missing custody"
    );
    let mut wrong_origin = files;
    wrong_origin[0].origin = SourceOrigin::Toolchain;
    assert!(
        derive_consumed_source_units(&checked_sources(wrong_origin), &custody).unwrap_err()[0]
            .message
            .contains("does not match final checked source")
    );
}

#[test]
fn generated_coordinates_distinguish_scopes_but_preserve_collision_rejection() {
    use source::DependencyScope::{Build, Product};
    let generated = |bytes: &[u8]| {
        let tree = build_output::from_entries(&[build_output::OutputTreeEntry::regular_file(
            b"shared.omg",
            bytes,
            false,
        )])
        .unwrap();
        build_output::select_included_sources(&tree, &[b"shared.omg".to_vec()])
            .unwrap()
            .pop()
            .unwrap()
    };
    let custody = vec![
        (SourceId(1), generated(b"data Product {}")),
        (SourceId(2), generated(b"data Helper {}")),
    ];
    let mut files = generated_files(&custody);
    files[0].dependency_scope = Product;
    files[1].dependency_scope = Build;
    let units = derive_consumed_source_units(&checked_sources(files.clone()), &custody).unwrap();
    assert_eq!(units.len(), 2);
    assert_eq!(
        units[0].kind(),
        ConsumedSourceUnitKind::PackageGenerated(Product)
    );
    assert_eq!(
        units[1].kind(),
        ConsumedSourceUnitKind::PackageGenerated(Build)
    );
    assert_eq!(canonical_consumed_unit_bytes(&units[0])[0], 1);
    assert_eq!(canonical_consumed_unit_bytes(&units[1])[0], 4);
    verify_current_files(&checked_sources(files.clone()), &custody).unwrap();
    files[1].dependency_scope = Product;
    let diagnostics = derive_consumed_source_units(&checked_sources(files), &custody).unwrap_err();
    assert!(
        diagnostics[0]
            .message
            .contains("distinct sources at duplicate canonical coordinates")
    );

    let mut authored = source(
        "/package/shared.omg",
        "/package",
        Some(PackageKeyIdentity::from_digest([7; 32]).unwrap()),
        SourceOrigin::User,
        "data Shared {}",
    );
    authored.source_id = SourceId(3);
    let mut other_scope = authored.clone();
    other_scope.source_id = SourceId(4);
    other_scope.dependency_scope = Build;
    let baseline =
        derive_consumed_source_units(&checked_sources(vec![authored.clone()]), &[]).unwrap();
    assert_eq!(
        baseline,
        derive_consumed_source_units(&checked_sources(vec![authored, other_scope]), &[]).unwrap()
    );
}

#[test]
fn generated_verification_preserves_physical_rereads_and_custody_only_behavior() {
    let custody = generated_fixture();
    let mut files = generated_files(&custody);
    // Verification is also callable alone: projection, not this reread,
    // owns duplicate/missing ID and generated logical-path admission.
    let mut unchecked = vec![custody[1].clone(), custody[0].clone()];
    unchecked.push((custody[0].0, custody[1].1.clone()));
    unchecked.push((SourceId(usize::MAX), custody[0].1.clone()));
    verify_current_files(&checked_sources(files.clone()), &unchecked).unwrap();
    files[0].source = Arc::from("drifted");
    assert!(
        verify_current_files(&checked_sources(files.clone()), &unchecked).unwrap_err()[0]
            .message
            .contains("drifted from staged-output custody")
    );
    files = generated_files(&custody);
    let path = std::env::temp_dir().join(format!(
        "omega-source-consumption-reread-{}.omg",
        std::process::id()
    ));
    std::fs::write(&path, "physical").expect("write physical source");
    let mut physical = source("unused", "unused", None, SourceOrigin::User, "physical");
    physical.path = path.clone();
    physical.source_id = SourceId(0);
    files.insert(1, physical);
    let program = checked_sources(files);
    verify_current_files(&program, &unchecked).unwrap();
    std::fs::write(&path, "changed").expect("change physical source");
    assert!(
        verify_current_files(&program, &unchecked).unwrap_err()[0]
            .message
            .contains("changed after frontend loading")
    );
    std::fs::remove_file(&path).expect("remove physical source");
    assert!(
        verify_current_files(&program, &unchecked).unwrap_err()[0]
            .message
            .contains("cannot be re-read")
    );
}

fn canonical_row_fixture() -> Vec<ConsumedSourceUnit> {
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    [
        (
            ConsumedSourceUnitKind::PackageAuthored,
            Some(package),
            None,
            vec!["main.omg".to_owned()],
        ),
        (
            ConsumedSourceUnitKind::PackageGenerated(source::DependencyScope::Product),
            Some(package),
            None,
            vec!["generated".to_owned(), "λ.omg".to_owned()],
        ),
        (
            ConsumedSourceUnitKind::ToolchainVirtual,
            None,
            Some("std".to_owned()),
            vec!["<prelude>".to_owned()],
        ),
        (
            ConsumedSourceUnitKind::ToolchainOwned,
            None,
            Some("core".to_owned()),
            vec!["nested".to_owned(), "types.omg".to_owned()],
        ),
    ]
    .into_iter()
    .map(
        |(kind, package, toolchain_namespace, relative_path)| ConsumedSourceUnit {
            kind,
            package,
            toolchain_namespace,
            relative_path,
            byte_count: 123,
            content_digest: [13; 32],
        },
    )
    .collect()
}

#[test]
fn canonical_row_layout_and_source_commitment_are_stable() {
    let units = canonical_row_fixture();
    let mut identities = units
        .iter()
        .map(|unit| format!("{:x}", Sha256::digest(canonical_consumed_unit_bytes(unit))))
        .collect::<Vec<_>>();
    let package = units[0].package().expect("authored owner");
    let inputs = super::super::PackageCompilationInputs::new_package(
        package,
        vec![super::super::PackageSourceBinding::new(
            package,
            "canonical-row-fixture",
            std::env::current_dir().expect("package root"),
        )],
        Vec::new(),
    )
    .expect("single package graph");
    let commitment = derive_source_consumption_commitment(&units, &inputs).expect("commitment");
    identities.push(
        commitment
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    );
    // Product row framing is unchanged; the V4 commitment names scope-aware
    // generated-source coordinates.
    assert_eq!(
        identities,
        [
            "ae0e65b19e6af4b93dce85e0feb096a5858db96308d6c7e60d99915732e3a213",
            "519ca0511d1018331db0e01bb39405e40d0660985e099f787918737492c15671",
            "eb986861f4e7b8b2018605c9436f9aabf7e535182dd8ea9f774d90cdba4a9285",
            "24b8039fc620e9d7d4a69da5af6c2a203ecabb369fcf263a8acb134cd1021141",
            "a908b49af2d2319a97359c14fa6e5aa57af79f6c5cf90a304ef1781c5c27e01e",
        ]
    );
}

#[test]
fn canonical_source_rows_append_in_place_without_changing_framing() {
    let fixture = canonical_row_fixture();
    let package = fixture[0].package().expect("authored owner");
    for row_count in [0, 1, 4, 4096] {
        let mut units = (0..row_count)
            .map(|ordinal| {
                let mut unit = fixture[ordinal % fixture.len()].clone();
                unit.relative_path
                    .push(format!("{ordinal}-{}", "x".repeat(ordinal % 257)));
                unit
            })
            .collect::<Vec<_>>();
        units.sort();
        // Independent outer framing preserves the former manifest protocol:
        // count, then each raw row's length and bytes, in canonical order.
        let mut expected = b"manifest-prefix".to_vec();
        expected.extend_from_slice(&(row_count as u64).to_le_bytes());
        for unit in &units {
            append_field(&mut expected, &canonical_consumed_unit_bytes(unit));
        }
        expected.extend_from_slice(b"manifest-suffix");
        let subject = PackageCompilationSubject {
            root: package,
            dependency_closure: super::super::PackageDependencyClosure::from_canonical_parts(
                package,
                super::super::BuildDeclarationKind::Package,
                vec![package],
                Vec::new(),
            )
            .expect("single package closure"),
            source_consumption_commitment: PackageSourceConsumptionCommitment::for_test([1; 32]),
            consumed_units: units,
        };
        let mut actual = Vec::with_capacity(expected.len());
        actual.extend_from_slice(b"manifest-prefix");
        let storage = actual.as_ptr();
        subject.append_canonical_consumed_units(&mut actual);
        actual.extend_from_slice(b"manifest-suffix");
        assert_eq!(actual, expected, "{row_count} rows");
        assert_eq!(
            actual.as_ptr(),
            storage,
            "caller-supplied storage is retained"
        );
    }
}

fn source(
    path: &str,
    root: &str,
    package: Option<PackageKeyIdentity>,
    origin: SourceOrigin,
    text: &str,
) -> SourceFile {
    SourceFile {
        source_id: SourceId(0),
        path: PathBuf::from(path),
        package_root: PathBuf::from(root),
        package_identity: package,
        dependency_scope: source::DependencyScope::Product,
        origin,
        resolution_stratum: source::SourceResolutionStratum::Base,
        source: Arc::from(text),
    }
}

#[test]
fn canonical_entries_ignore_absolute_package_location() {
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let first = source(
        "/cache/one/pkg/main.omg",
        "/cache/one/pkg",
        Some(package),
        SourceOrigin::User,
        "machine main {}",
    );
    let second = source(
        "/different/cache/pkg/main.omg",
        "/different/cache/pkg",
        Some(package),
        SourceOrigin::User,
        "machine main {}",
    );
    assert_eq!(
        canonical_source_entry(&first).expect("first canonical entry"),
        canonical_source_entry(&second).expect("second canonical entry")
    );
}

#[test]
fn canonical_entries_bind_package_path_and_bytes() {
    let first_package = PackageKeyIdentity::from_digest([7; 32]).expect("first package");
    let second_package = PackageKeyIdentity::from_digest([8; 32]).expect("second package");
    let baseline = source(
        "/cache/pkg/main.omg",
        "/cache/pkg",
        Some(first_package),
        SourceOrigin::User,
        "machine main {}",
    );
    let other_package = source(
        "/cache/pkg/main.omg",
        "/cache/pkg",
        Some(second_package),
        SourceOrigin::User,
        "machine main {}",
    );
    let other_path = source(
        "/cache/pkg/lib.omg",
        "/cache/pkg",
        Some(first_package),
        SourceOrigin::User,
        "machine main {}",
    );
    let other_bytes = source(
        "/cache/pkg/main.omg",
        "/cache/pkg",
        Some(first_package),
        SourceOrigin::User,
        "machine changed {}",
    );
    let baseline = canonical_source_entry(&baseline).expect("baseline entry");
    assert_ne!(baseline, canonical_source_entry(&other_package).unwrap());
    assert_ne!(baseline, canonical_source_entry(&other_path).unwrap());
    assert_ne!(baseline, canonical_source_entry(&other_bytes).unwrap());
}

#[test]
fn consumed_units_are_logical_content_addressed_and_classified() {
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let authored = source(
        "/host/cache/pkg/main.omg",
        "/host/cache/pkg",
        Some(package),
        SourceOrigin::User,
        "machine main {}",
    );
    let relocated = source(
        "/other/root/pkg/main.omg",
        "/other/root/pkg",
        Some(package),
        SourceOrigin::User,
        "machine main {}",
    );
    let generated = consumed_source_unit(&authored, true).expect("generated row");
    let authored = consumed_source_unit(&authored, false).expect("authored row");
    let relocated = consumed_source_unit(&relocated, false).expect("relocated row");

    assert_eq!(authored, relocated);
    assert_eq!(authored.kind(), ConsumedSourceUnitKind::PackageAuthored);
    assert_eq!(
        generated.kind(),
        ConsumedSourceUnitKind::PackageGenerated(source::DependencyScope::Product)
    );
    assert_ne!(authored, generated);
    assert!(
        !canonical_consumed_unit_bytes(&authored)
            .windows(b"/host/cache".len())
            .any(|window| window == b"/host/cache")
    );

    let virtual_source = source(
        "<prelude>",
        "toolchain/std",
        None,
        SourceOrigin::Toolchain,
        "data Unit {}",
    );
    let owned_source = source(
        "toolchain/std/types.omg",
        "toolchain/std",
        None,
        SourceOrigin::Toolchain,
        "data Unit {}",
    );
    assert_eq!(
        consumed_source_unit(&virtual_source, false)
            .expect("virtual row")
            .kind(),
        ConsumedSourceUnitKind::ToolchainVirtual
    );
    assert_eq!(
        consumed_source_unit(&owned_source, false)
            .expect("owned row")
            .kind(),
        ConsumedSourceUnitKind::ToolchainOwned
    );
}

#[test]
fn toolchain_source_identity_binds_namespace_path_and_exact_bytes() {
    let baseline = source(
        "toolchain/std/types.omg",
        "toolchain/std",
        None,
        SourceOrigin::Toolchain,
        "data Packet {}",
    );
    let changed_namespace = source(
        "toolchain/core/types.omg",
        "toolchain/core",
        None,
        SourceOrigin::Toolchain,
        "data Packet {}",
    );
    let changed_path = source(
        "toolchain/std/other.omg",
        "toolchain/std",
        None,
        SourceOrigin::Toolchain,
        "data Packet {}",
    );
    let changed_bytes = source(
        "toolchain/std/types.omg",
        "toolchain/std",
        None,
        SourceOrigin::Toolchain,
        "data Packet { value: u8; }",
    );

    let baseline = toolchain_source_identity_digest(&baseline).expect("baseline identity");
    assert_ne!(
        baseline,
        toolchain_source_identity_digest(&changed_namespace).unwrap()
    );
    assert_ne!(
        baseline,
        toolchain_source_identity_digest(&changed_path).unwrap()
    );
    assert_ne!(
        baseline,
        toolchain_source_identity_digest(&changed_bytes).unwrap()
    );
}

/// One-field mutation matrix over the retained source-consumption receipt:
/// each `ConsumedSourceUnit` row field and each reconciled package-graph
/// axis the commitment hashes. Every representable substitution either
/// keeps the canonical roster order and recomputes a divergent commitment,
/// or cannot keep the order and rejects at derivation as non-canonical.
/// Build-purpose edges are compilation-local nameability, never bound by
/// this commitment -- the durable closure projection is product-only.
#[test]
fn source_consumption_commitment_rejects_every_one_field_substitution() {
    use super::super::{
        BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding,
        PackageSourceBinding,
    };
    use build_declarations::DependencyPurpose;

    let units = canonical_row_fixture();
    let package = units[0].package().expect("authored owner");
    let root = std::env::temp_dir().join(format!(
        "omega-source-consumption-substitution-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for name in ["root", "middle", "leaf"] {
        std::fs::create_dir_all(root.join(name)).expect("create package source root");
    }
    let binding = |identity: PackageKeyIdentity, name: &'static str| {
        PackageSourceBinding::new(identity, name, root.join(name))
    };
    let inputs = PackageCompilationInputs::new(
        package,
        BuildDeclarationKind::Package,
        vec![
            binding(package, "root"),
            binding(PackageKeyIdentity::from_digest([8; 32]).unwrap(), "middle"),
            binding(PackageKeyIdentity::from_digest([9; 32]).unwrap(), "leaf"),
        ],
        vec![
            PackageDependencyBinding::new(
                package,
                "middle",
                PackageKeyIdentity::from_digest([8; 32]).unwrap(),
            ),
            PackageDependencyBinding::new(
                PackageKeyIdentity::from_digest([8; 32]).unwrap(),
                "leaf",
                PackageKeyIdentity::from_digest([9; 32]).unwrap(),
            ),
        ],
    )
    .expect("three-package chain graph");
    let baseline =
        derive_source_consumption_commitment(&units, &inputs).expect("baseline commitment");
    let baseline_rows: Vec<Vec<u8>> = units.iter().map(canonical_consumed_unit_bytes).collect();

    // Every representable ConsumedSourceUnit field: the canonical row bytes
    // must diverge, and the recomputed commitment must diverge -- unless the
    // substitution cannot keep the strictly ordered roster, which rejects at
    // derivation as non-canonical.
    let other_package = PackageKeyIdentity::from_digest([77; 32]).expect("other package");
    let field_cases: Vec<(&'static str, usize, Box<dyn Fn(&mut ConsumedSourceUnit)>)> = vec![
        (
            "kind",
            0,
            Box::new(|unit| unit.kind = ConsumedSourceUnitKind::ToolchainOwned),
        ),
        (
            "package",
            0,
            Box::new(move |unit| unit.package = Some(other_package)),
        ),
        ("package::cleared", 1, Box::new(|unit| unit.package = None)),
        (
            "toolchain_namespace",
            3,
            Box::new(|unit| unit.toolchain_namespace = Some("std".to_owned())),
        ),
        (
            "toolchain_namespace::cleared",
            3,
            Box::new(|unit| unit.toolchain_namespace = None),
        ),
        (
            "relative_path component",
            0,
            Box::new(|unit| unit.relative_path[0] = "other.omg".to_owned()),
        ),
        (
            "relative_path::extend",
            0,
            Box::new(|unit| unit.relative_path.push("extra.omg".to_owned())),
        ),
        (
            "relative_path::drop",
            1,
            Box::new(|unit| {
                unit.relative_path.pop();
            }),
        ),
        ("byte_count", 2, Box::new(|unit| unit.byte_count += 1)),
        (
            "content_digest",
            0,
            Box::new(|unit| unit.content_digest = [14; 32]),
        ),
    ];
    for (name, index, mutate) in field_cases {
        let mut mutated = units.clone();
        mutate(&mut mutated[index]);
        assert_ne!(
            canonical_consumed_unit_bytes(&mutated[index]),
            baseline_rows[index],
            "{name}: the canonical row bytes must diverge"
        );
        match derive_source_consumption_commitment(&mutated, &inputs) {
            Ok(commitment) => assert_ne!(
                commitment, baseline,
                "{name}: an order-preserving substitution must diverge the commitment"
            ),
            Err(diagnostics) => {
                assert!(
                    mutated.windows(2).any(|pair| pair[0] >= pair[1]),
                    "{name}: derivation may only reject a non-canonical roster"
                );
                assert!(
                    diagnostics[0]
                        .message
                        .contains("strictly ordered unique consumed units"),
                    "{name}: expected the canonical-order rejection: {diagnostics:?}"
                );
            }
        }
    }

    // Roster mutations: drops and canonical-position inserts recompute a
    // divergent identity; duplicates and reorderings reject at derivation.
    let mut dropped = units.clone();
    dropped.remove(0);
    assert_ne!(
        derive_source_consumption_commitment(&dropped, &inputs).expect("dropped roster"),
        baseline,
        "a dropped row must diverge the commitment"
    );
    assert!(
        derive_source_consumption_commitment(&[], &inputs).is_err(),
        "an empty roster rejects at derivation"
    );
    let mut duplicated = units.clone();
    duplicated.insert(1, units[0].clone());
    assert!(
        derive_source_consumption_commitment(&duplicated, &inputs).is_err(),
        "a duplicated row rejects at derivation"
    );
    let mut swapped = units.clone();
    swapped.swap(0, 1);
    assert!(
        derive_source_consumption_commitment(&swapped, &inputs).is_err(),
        "a reordered roster rejects at derivation"
    );
    let reversed: Vec<ConsumedSourceUnit> = units.iter().cloned().rev().collect();
    assert!(
        derive_source_consumption_commitment(&reversed, &inputs).is_err(),
        "a reversed roster rejects at derivation"
    );
    let mut forged = units[0].clone();
    forged.relative_path = vec!["zzz.omg".to_owned()];
    let mut inserted = units.clone();
    inserted.insert(1, forged);
    assert_ne!(
        derive_source_consumption_commitment(&inserted, &inputs).expect("inserted roster"),
        baseline,
        "an inserted forged row must diverge the commitment"
    );

    // Every reconciled package-graph axis the commitment binds: root,
    // role, the package roster, and the product-edge coordinates.
    let middle = PackageKeyIdentity::from_digest([8; 32]).unwrap();
    let leaf = PackageKeyIdentity::from_digest([9; 32]).unwrap();
    let graph_cases: Vec<(&'static str, PackageCompilationInputs)> = vec![
        (
            "root identity",
            PackageCompilationInputs::new_package(
                middle,
                vec![binding(middle, "root"), binding(leaf, "leaf")],
                vec![PackageDependencyBinding::new(middle, "leaf", leaf)],
            )
            .expect("substituted root graph"),
        ),
        (
            "root role",
            PackageCompilationInputs::new(
                package,
                BuildDeclarationKind::Application,
                vec![
                    binding(package, "root"),
                    binding(middle, "middle"),
                    binding(leaf, "leaf"),
                ],
                vec![
                    PackageDependencyBinding::new(package, "middle", middle),
                    PackageDependencyBinding::new(middle, "leaf", leaf),
                ],
            )
            .expect("application-role graph"),
        ),
        (
            "package roster::drop",
            PackageCompilationInputs::new_package(
                package,
                vec![binding(package, "root"), binding(middle, "middle")],
                vec![PackageDependencyBinding::new(package, "middle", middle)],
            )
            .expect("leaf-dropped graph"),
        ),
        (
            "package identity",
            PackageCompilationInputs::new_package(
                package,
                vec![
                    binding(package, "root"),
                    binding(middle, "middle"),
                    binding(other_package, "leaf"),
                ],
                vec![
                    PackageDependencyBinding::new(package, "middle", middle),
                    PackageDependencyBinding::new(middle, "leaf", other_package),
                ],
            )
            .expect("substituted package graph"),
        ),
        (
            "edge alias",
            PackageCompilationInputs::new_package(
                package,
                vec![
                    binding(package, "root"),
                    binding(middle, "middle"),
                    binding(leaf, "leaf"),
                ],
                vec![
                    PackageDependencyBinding::new(package, "middle", middle),
                    PackageDependencyBinding::new(middle, "other", leaf),
                ],
            )
            .expect("renamed-alias graph"),
        ),
        (
            "edge requester",
            PackageCompilationInputs::new_package(
                package,
                vec![
                    binding(package, "root"),
                    binding(middle, "middle"),
                    binding(leaf, "leaf"),
                ],
                vec![
                    PackageDependencyBinding::new(package, "middle", middle),
                    PackageDependencyBinding::new(package, "leaf", leaf),
                ],
            )
            .expect("root-requested leaf graph"),
        ),
        (
            "edge roster::drop",
            PackageCompilationInputs::new_package(
                package,
                vec![
                    binding(package, "root"),
                    binding(middle, "middle"),
                    binding(leaf, "leaf"),
                ],
                vec![
                    PackageDependencyBinding::new(package, "middle", middle),
                    PackageDependencyBinding::new(package, "leaf", leaf),
                    PackageDependencyBinding::new(middle, "leaf", leaf),
                ],
            )
            .expect("extra-edge graph"),
        ),
    ];
    for (name, mutated_inputs) in graph_cases {
        assert_ne!(
            derive_source_consumption_commitment(&units, &mutated_inputs)
                .expect("mutated graph commitment"),
            baseline,
            "{name}: a substituted package-graph axis must diverge the commitment"
        );
    }

    // Bounded slack: a build-purpose edge is compilation-local nameability
    // for the root's build entry. It never joins the durable product
    // closure this commitment binds, so it cannot diverge the identity.
    let with_build_edge = PackageCompilationInputs::new_package(
        package,
        vec![
            binding(package, "root"),
            binding(middle, "middle"),
            binding(leaf, "leaf"),
        ],
        vec![
            PackageDependencyBinding::new(package, "middle", middle),
            PackageDependencyBinding::new(middle, "leaf", leaf),
            PackageDependencyBinding::for_purpose(
                package,
                "build_tools",
                leaf,
                DependencyPurpose::Build,
            ),
        ],
    )
    .expect("build-edge graph");
    assert_eq!(
        derive_source_consumption_commitment(&units, &with_build_edge)
            .expect("build-edge commitment"),
        baseline,
        "a build-purpose edge must not enter the product-consumption commitment"
    );

    let _ = std::fs::remove_dir_all(&root);
}
