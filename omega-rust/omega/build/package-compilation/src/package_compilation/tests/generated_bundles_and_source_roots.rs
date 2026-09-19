use super::{
    TempTree, generated_bundle, generated_source, identity, three_package_generated_inputs,
};
use crate::package_compilation::{
    Arc, BTreeMap, BuildDeclarationKind, PackageCompilationInputError, PackageCompilationInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceBinding, PackageSourceConsumptionCommitment, PathBuf,
    append_overlapping_source_roots,
};
use std::fs;

#[test]
fn requester_local_aliases_may_name_different_targets() {
    let tree = TempTree::new();
    let packages = (1..=4)
        .map(|marker| {
            PackageSourceBinding::new(
                identity(marker),
                format!("package-{marker}"),
                tree.package(&marker.to_string()),
            )
        })
        .collect();
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        packages,
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::new(identity(2), "shared", identity(3)),
            PackageDependencyBinding::new(identity(3), "leaf", identity(4)),
        ],
    )
    .expect("requester-local aliases should reconcile");

    assert_eq!(
        inputs.dependency_target(identity(1), "shared"),
        Some(identity(2))
    );
    assert_eq!(
        inputs.dependency_target(identity(2), "shared"),
        Some(identity(3))
    );
    assert_eq!(inputs.package_name(identity(1)), Some("package-1"));
    assert!(inputs.allows_declaration_selection(identity(1), identity(1)));
    assert!(inputs.allows_declaration_selection(identity(1), identity(2)));
    assert!(!inputs.allows_declaration_selection(identity(1), identity(3)));
    assert!(inputs.allows_declaration_selection(identity(2), identity(3)));
    assert!(
        inputs
            .package_label(identity(1))
            .starts_with("`package-1` (")
    );
}

#[test]
fn noncanonical_package_names_reject_at_compiler_handoff() {
    let tree = TempTree::new();
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "not_canonical",
            tree.package("root"),
        )],
        Vec::new(),
    )
    .expect_err("compiler inputs must independently reject noncanonical package names");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::InvalidPackageName { identity: found, name }
            if *found == identity(1) && name == "not_canonical"
    )));
}

#[test]
fn duplicate_aliases_and_unreachable_rows_reject() {
    let tree = TempTree::new();
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("root")),
            PackageSourceBinding::new(identity(2), "first", tree.package("first")),
            PackageSourceBinding::new(identity(3), "second", tree.package("second")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "dep", identity(2)),
            PackageDependencyBinding::new(identity(1), "dep", identity(2)),
        ],
    )
    .expect_err("duplicate alias and unreachable package must reject");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::DuplicateAlias { alias, .. } if alias == "dep"
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::UnreachablePackage { identity: found }
            if *found == identity(3)
    )));
}

#[test]
fn package_records_retain_complete_custody_and_requester_local_edges() {
    let tree = TempTree::new();
    let root_path = tree.package("root");
    let leaf_path = tree.package("leaf");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root_path, fs::Permissions::from_mode(0o555))
            .expect("seal canonical metadata root");
    }
    let root = PackageSourceBinding::new(identity(1), "root", root_path.clone())
        .with_canonical_source_metadata()
        .expect("capture root custody");
    let metadata = root.canonical_source_metadata().cloned();
    let packages = vec![
        PackageSourceBinding::new(identity(2), "leaf", leaf_path.clone()),
        root,
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(2),
    )];
    let inputs =
        PackageCompilationInputs::new_package(identity(1), packages.clone(), dependencies.clone())
            .expect("complete records form a closed graph");
    let mut reordered = packages;
    reordered.reverse();
    let independent = PackageCompilationInputs::new_package(identity(1), reordered, dependencies)
        .expect("source record construction is input-order independent");
    assert_eq!(inputs.source_inputs(), independent.source_inputs());
    assert!(!Arc::ptr_eq(
        &inputs.source_inputs(),
        &independent.source_inputs()
    ));
    assert_eq!(inputs.source.packages.len(), 2);
    let root_record = &inputs.source.packages[&identity(1)];
    assert_eq!(root_record.source_root, root_path.canonicalize().unwrap());
    assert_eq!(root_record.canonical_name, "root");
    assert_eq!(root_record.canonical_source_metadata, metadata);
    assert_eq!(root_record.dependencies.get("leaf"), Some(&identity(2)));
    let leaf_record = &inputs.source.packages[&identity(2)];
    assert_eq!(leaf_record.source_root, leaf_path.canonicalize().unwrap());
    assert_eq!(leaf_record.canonical_name, "leaf");
    assert!(leaf_record.canonical_source_metadata.is_none());
    assert!(leaf_record.dependencies.is_empty());
    assert_eq!(
        inputs.dependency_closure_for(identity(2)).packages(),
        &[identity(2)]
    );
    assert!(
        inputs
            .dependency_closure_for(identity(2))
            .dependencies()
            .is_empty()
    );
    assert_eq!(inputs.dependency_target(identity(2), "leaf"), None);
}

#[test]
fn ancestor_root_validation_preserves_every_overlap_in_pairwise_order() {
    let tree = TempTree::new();
    let paths = [
        "a",
        "a/deep",
        "a/deep/leaf",
        "a/sibling",
        "a-sibling",
        "ab",
        "z",
    ];
    let mut roots = BTreeMap::new();
    let mut packages = Vec::new();
    let mut dependencies = Vec::new();
    for (position, relative) in paths.iter().enumerate() {
        let package = identity(u8::try_from(position + 1).unwrap());
        let path = tree.0.join(relative);
        fs::create_dir_all(&path).expect("create nested and disjoint package roots");
        roots.insert(path.canonicalize().unwrap(), package);
        packages.push(PackageSourceBinding::new(
            package,
            format!("package-{position}"),
            path,
        ));
        if position > 0 {
            dependencies.push(PackageDependencyBinding::new(
                identity(1),
                format!("child_{position}"),
                package,
            ));
        }
    }
    let rows = roots.iter().collect::<Vec<_>>();
    let mut expected = Vec::new();
    for (position, (first_root, first)) in rows.iter().enumerate() {
        for (second_root, second) in rows.iter().skip(position + 1) {
            if first_root.starts_with(second_root) || second_root.starts_with(first_root) {
                expected.push(PackageCompilationInputError::OverlappingSourceRoots {
                    first: **first,
                    first_root: (*first_root).clone(),
                    second: **second,
                    second_root: (*second_root).clone(),
                });
            }
        }
    }
    assert_eq!(
        expected.len(),
        4,
        "all nested ancestors, no textual-prefix siblings"
    );
    packages.reverse();
    let errors = PackageCompilationInputs::new_package(identity(1), packages, dependencies)
        .expect_err("nested source custody rejects");
    assert_eq!(errors, expected);
}

#[test]
fn ancestor_root_validation_accepts_many_component_disjoint_siblings() {
    let roots = (1..=200u8)
        .map(|marker| {
            (
                std::env::temp_dir().join(format!("package-{marker}")),
                identity(marker),
            )
        })
        .collect();
    let mut errors = Vec::new();
    append_overlapping_source_roots(&roots, &mut errors);
    assert!(errors.is_empty());
}

#[cfg(windows)]
#[test]
fn ancestor_root_validation_respects_windows_volume_and_share_prefixes() {
    let roots = [
        (r"\\?\C:\packages", identity(1)),
        (r"\\?\C:\packages\child", identity(2)),
        (r"\\?\D:\packages\child", identity(3)),
        (r"\\?\UNC\server\share\packages", identity(4)),
        (r"\\?\UNC\server\share\packages\child", identity(5)),
        (r"\\?\UNC\server\other\packages\child", identity(6)),
    ]
    .into_iter()
    .map(|(path, package)| (PathBuf::from(path), package))
    .collect();
    let mut errors = Vec::new();
    append_overlapping_source_roots(&roots, &mut errors);
    let mut pairs = errors
        .iter()
        .map(|error| match error {
            PackageCompilationInputError::OverlappingSourceRoots { first, second, .. } => {
                (*first, *second)
            }
            other => panic!("unexpected root validation error: {other:?}"),
        })
        .collect::<Vec<_>>();
    pairs.sort_unstable();
    assert_eq!(
        pairs,
        vec![(identity(1), identity(2)), (identity(4), identity(5))]
    );
}

#[test]
fn overlapping_roots_and_cycles_reject() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let nested = root.join("nested");
    fs::create_dir(&nested).expect("create nested package");
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root),
            PackageSourceBinding::new(identity(2), "nested", nested),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "child", identity(2)),
            PackageDependencyBinding::new(identity(2), "parent", identity(1)),
        ],
    )
    .expect_err("overlap and cycle must reject");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::OverlappingSourceRoots { .. }
    )));
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, PackageCompilationInputError::DependencyCycle { .. }))
    );
}

#[test]
fn canonical_path_free_closure_recovery_rejects_open_unreachable_and_cyclic_graphs() {
    let packages = vec![identity(1), identity(2)];
    let unreachable = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages.clone(),
        Vec::new(),
    )
    .expect_err("unreachable path-free closure package must reject");
    assert!(unreachable.contains("unreachable"));

    let open = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages.clone(),
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(3),
        )],
    )
    .expect_err("open path-free closure edge must reject");
    assert!(open.contains("open edge"));

    let cyclic = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages,
        vec![
            PackageDependencyBinding::new(identity(1), "dependency", identity(2)),
            PackageDependencyBinding::new(identity(2), "root", identity(1)),
        ],
    )
    .expect_err("cyclic path-free closure must reject");
    assert!(cyclic.contains("cycle"));
}

#[test]
fn generated_source_execution_profile_is_exact_even_for_empty_bundles() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let bundle = |package, profile| {
        PackageGeneratedSourceBundle::from_checked(
            package,
            target::TargetProfile::WindowsX64,
            profile,
            inputs.dependency_closure_for(package),
            PackageSourceConsumptionCommitment::for_test([19; 32]),
            Vec::new(),
        )
    };
    let linux = Some(target::TargetProfile::LinuxX64);
    let windows = Some(target::TargetProfile::WindowsX64);
    assert_ne!(bundle(identity(2), linux), bundle(identity(2), windows));
    assert_ne!(bundle(identity(2), linux), bundle(identity(2), None));
    let profiled = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![
            bundle(identity(2), linux),
            bundle(identity(3), linux),
        ])
        .unwrap();
    profiled
        .validate_dependency_generated_source_target(windows)
        .unwrap();
    profiled
        .validate_dependency_generated_source_execution_profile(linux)
        .unwrap();
    for requested in [windows, None] {
        let errors = profiled
            .validate_dependency_generated_source_execution_profile(requested)
            .unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|error| matches!(error,
            PackageCompilationInputError::GeneratedSourceBundleExecutionProfileMismatch { bundle_profile, execution_profile, .. }
            if *bundle_profile == linux && *execution_profile == requested)));
    }
    let unprofiled = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![
            bundle(identity(2), None),
            bundle(identity(3), None),
        ])
        .unwrap();
    unprofiled
        .validate_dependency_generated_source_execution_profile(None)
        .unwrap();
    assert!(
        unprofiled
            .validate_dependency_generated_source_execution_profile(linux)
            .is_err()
    );
}

#[test]
fn complete_generated_source_bundles_bind_owner_closure_target_and_bytes() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let generated = generated_source(
        b"generated_api.omg",
        b"pub machine generated_value() -> u64 { 17 }\n",
    );
    let generated_digest = generated.digest();
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        vec![generated],
    );
    let retained_middle = middle.clone();
    assert!(std::ptr::eq(
        middle.dependency_closure(),
        retained_middle.dependency_closure()
    ));
    assert_eq!(
        middle.sources().as_ptr(),
        retained_middle.sources().as_ptr()
    );
    let independently_built = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        retained_middle.sources().to_vec(),
    );
    assert_eq!(
        middle, independently_built,
        "equality observes contents, not allocation identity"
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );

    let inputs = inputs
        .with_complete_dependency_generated_sources(vec![leaf, middle])
        .expect("one exact bundle per dependency should attach");
    inputs
        .validate_dependency_generated_source_target(Some(target::TargetProfile::WindowsX64))
        .expect("matching generated-source targets should validate");
    let logical = inputs
        .generated_source_import_path(identity(2), &[PathBuf::from("generated_api.omg")])
        .expect("compiler-issued generated path should remain canonical")
        .expect("generated module should resolve from retained custody");
    assert_eq!(
        logical,
        inputs
            .package_root(identity(2))
            .unwrap()
            .join(".omega/generated/generated_api.omg")
    );
    let retained = inputs
        .dependency_generated_source_bundles()
        .find(|bundle| bundle.package() == identity(2))
        .and_then(|bundle| bundle.sources().first())
        .expect("exact package bundle retains its generated bytes");
    assert_eq!(retained.relative_path(), b"generated_api.omg");
    assert_eq!(
        retained.bytes(),
        b"pub machine generated_value() -> u64 { 17 }\n"
    );
    assert_eq!(retained.digest(), generated_digest);
    drop(inputs);
    assert_eq!(retained_middle.sources()[0].digest(), generated_digest);
}

#[test]
fn diamond_consumers_share_the_leaf_bundle_without_sharing_admission() {
    let tree = TempTree::new();
    let packages = (1..=4)
        .map(|marker| {
            PackageSourceBinding::new(
                identity(marker),
                format!("package-{marker}"),
                tree.package(&marker.to_string()),
            )
        })
        .collect::<Vec<_>>();
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        packages.clone(),
        vec![
            PackageDependencyBinding::new(identity(1), "left", identity(2)),
            PackageDependencyBinding::new(identity(1), "right", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(4)),
            PackageDependencyBinding::new(identity(3), "leaf", identity(4)),
        ],
    )
    .unwrap();
    let leaf = generated_bundle(
        &inputs,
        identity(4),
        target::TargetProfile::WindowsX64,
        14,
        vec![generated_source(
            b"leaf.omg",
            b"pub const VALUE: u64 = 17;\n",
        )],
    );
    let mut consumers = Vec::new();
    for root in [identity(2), identity(3)] {
        let consumer = PackageCompilationInputs::new_package(
            root,
            packages
                .iter()
                .filter(|package| [root, identity(4)].contains(&package.identity()))
                .cloned()
                .collect(),
            vec![PackageDependencyBinding::new(root, "leaf", identity(4))],
        )
        .unwrap()
        .with_complete_dependency_generated_sources(vec![leaf.clone()])
        .unwrap();
        consumer
            .validate_dependency_generated_source_target(Some(target::TargetProfile::WindowsX64))
            .unwrap();
        assert!(
            consumer
                .validate_dependency_generated_source_target(Some(target::TargetProfile::LinuxX64))
                .is_err()
        );
        let retained = consumer
            .dependency_generated_source_bundles()
            .next()
            .unwrap();
        assert!(std::ptr::eq(
            retained.dependency_closure(),
            leaf.dependency_closure()
        ));
        assert_eq!(retained.sources().as_ptr(), leaf.sources().as_ptr());
        consumers.push(consumer);
    }
    drop(leaf);
    drop(consumers.remove(0));
    assert_eq!(
        consumers[0]
            .dependency_generated_source_bundles()
            .next()
            .unwrap()
            .sources()[0]
            .relative_path(),
        b"leaf.omg"
    );
}

#[test]
fn generated_source_bundle_omission_duplicate_foreign_root_and_closure_substitution_reject() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        Vec::new(),
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );

    let missing = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone()])
        .expect_err("omitted explicit empty leaf bundle must reject");
    assert!(missing.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::MissingGeneratedSourceBundle { package }
            if *package == identity(3)
    )));

    let duplicate = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![
            middle.clone(),
            middle.clone(),
            leaf.clone(),
        ])
        .expect_err("duplicate package bundle must reject");
    assert!(duplicate.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::DuplicateGeneratedSourceBundle { package }
            if *package == identity(2)
    )));

    let foreign = PackageGeneratedSourceBundle::from_checked(
        identity(4),
        target::TargetProfile::WindowsX64,
        target::TargetProfile::host_if_supported(),
        inputs.dependency_closure_for(identity(3)),
        PackageSourceConsumptionCommitment::for_test([14; 32]),
        Vec::new(),
    );
    let foreign_errors = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), foreign])
        .expect_err("foreign bundle must reject");
    assert!(foreign_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::ForeignGeneratedSourceBundle { package }
            if *package == identity(4)
    )));

    let root = PackageGeneratedSourceBundle::from_checked(
        identity(1),
        target::TargetProfile::WindowsX64,
        target::TargetProfile::host_if_supported(),
        inputs.dependency_closure(),
        PackageSourceConsumptionCommitment::for_test([11; 32]),
        Vec::new(),
    );
    let root_errors = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), root])
        .expect_err("root self-injection must reject");
    assert!(root_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::RootGeneratedSourceBundle { package }
            if *package == identity(1)
    )));

    let wrong_closure = PackageGeneratedSourceBundle::from_checked(
        identity(2),
        target::TargetProfile::WindowsX64,
        target::TargetProfile::host_if_supported(),
        inputs.dependency_closure_for(identity(3)),
        PackageSourceConsumptionCommitment::for_test([12; 32]),
        Vec::new(),
    );
    let closure_errors = inputs
        .with_complete_dependency_generated_sources(vec![wrong_closure, leaf])
        .expect_err("bundle from another producer closure must reject");
    assert!(closure_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package }
            if *package == identity(2)
    )));
}

#[test]
fn generated_source_bundle_target_substitution_rejects_before_loading() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        Vec::new(),
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );
    let inputs = inputs
        .with_complete_dependency_generated_sources(vec![middle, leaf])
        .expect("complete generated-source bundles should attach");
    let errors = inputs
        .validate_dependency_generated_source_target(Some(target::TargetProfile::LinuxX64))
        .expect_err("cross-target generated-source substitution must reject");
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|error| matches!(
        error,
        PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
            bundle_target: target::TargetProfile::WindowsX64,
            selected_target: Some(target::TargetProfile::LinuxX64),
            ..
        }
    )));
}

#[test]
fn missing_and_symlink_source_roots_reject() {
    let tree = TempTree::new();
    let missing = tree.0.join("missing");
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", missing)],
        Vec::new(),
    )
    .expect_err("missing source root must reject");
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::InvalidSourceRoot { .. }
    )));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let actual = tree.package("actual");
        let linked = tree.0.join("linked");
        symlink(actual, &linked).expect("create source-root symlink");
        let errors = PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", linked)],
            Vec::new(),
        )
        .expect_err("symlink source root must reject");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));
    }
}
