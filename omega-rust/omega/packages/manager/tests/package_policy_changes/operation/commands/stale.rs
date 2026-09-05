use super::*;

#[test]
fn changed_build_bytes_reject_resume_without_overwriting_the_concurrent_edit() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    accept(&initial);
    let path = tree.path("sources/root/build.omg");
    let changed = format!(
        "// concurrent declaration edit\n{}",
        fs::read_to_string(&path).unwrap()
    );
    fs::write(&path, &changed).unwrap();
    let before = accepted_files(&tree);
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("build.omg or omega.lock changed"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn changed_valid_lock_rejects_resume_without_replacing_the_accepted_pair() {
    let tree = fixture(ASSUMPTION);
    let baseline = execute(&tree, update(), vec![TARGET, TargetProfile::LinuxArm64]).unwrap();
    assert_eq!(baseline.status, PackageCommandStatus::Published);
    let accepted = lock(&tree);
    let initial = execute(&tree, install(), vec![TARGET]).unwrap();
    assert_eq!(initial.status, PackageCommandStatus::ReviewRequired);
    accept(&initial);
    // Another project writer landed a structurally valid, different lock.
    let changed =
        PackageLock::from_targets(vec![accepted.target(TARGET).unwrap().clone()]).unwrap();
    fs::write(
        tree.path("sources/root/omega.lock"),
        changed.canonical_text().unwrap(),
    )
    .unwrap();
    assert_eq!(lock(&tree), changed);
    let before = accepted_files(&tree);
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("build.omg or omega.lock changed"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn changed_main_source_rejects_resume_even_when_the_build_and_lock_are_unchanged() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    accept(&initial);
    let before = accepted_files(&tree);
    let path = tree.path("sources/root/main.omg");
    let changed = format!("// implementation edit\n{PURE}");
    fs::write(&path, &changed).unwrap();
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("project source changed"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn changed_local_dependency_rejects_resume_without_refreshing_the_pending_source_pin() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    accept(&initial);
    let before = accepted_files(&tree);
    let proposal = fs::read(proposal_path(&tree)).unwrap();
    let path = tree.path("sources/dependency/main.omg");
    let changed = format!("// dependency implementation edit\n{ASSUMPTION}");
    fs::write(&path, &changed).unwrap();
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("candidate sources or dependency graph changed"),
        "{error}"
    );
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
    assert_eq!(fs::read(proposal_path(&tree)).unwrap(), proposal);
}
