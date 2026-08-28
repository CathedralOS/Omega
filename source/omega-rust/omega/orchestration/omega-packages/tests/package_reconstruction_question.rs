use omega_packages::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionLimits,
    LocalSourceLimits, PackageSourceClosureLimits, SourceLineage, WorkspaceMemberPath,
    compile_resolved_package_reviews, resolve_workspace_package_closure,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("omega-packages should live under the Omega workspace")
        .to_path_buf()
}

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-package-reconstruction-question-{label}-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

fn graph_workbench_question() -> (
    PathBuf,
    omega_packages::ResolvedPackageSourceClosure,
    omega_packages::CompilerIssuedPackageReviewSet,
    CanonicalPackageReconstructionQuestion,
) {
    let temporary = temporary_root("graph-workbench");
    std::fs::create_dir_all(&temporary).expect("create temporary root");
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure(
        &workspace_lineage,
        WorkspaceMemberPath::parse("graph-workbench").unwrap(),
        &fixture_root,
        temporary.join("cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve graph-workbench source closure");
    let reviews =
        compile_resolved_package_reviews(&closure, "windows_x64", &temporary.join("build"))
            .expect("compile graph-workbench package reviews");
    let question = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &closure,
        &reviews,
        CanonicalPackageReconstructionQuestionLimits::default(),
    )
    .expect("associate source and obligation closure");
    (temporary, closure, reviews, question)
}

#[test]
fn canonical_question_round_trips_and_freshly_reconstructs_complete_closure() {
    let (temporary, closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    assert_eq!(question.entries().len(), closure.graph().packages().len());
    assert_eq!(question.target_name(), "windows_x64");
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.package())
            .eq(question
                .source_closure()
                .packages()
                .iter()
                .map(|source| source.key()))
    );
    for entry in question.entries() {
        assert_eq!(
            entry.obligation_ledger().package(),
            entry.package().identity()
        );
        let expected_transitive_packages = match entry.package().name().as_str() {
            "graph-workbench" => 3,
            "arithmetic-kernels" | "file-journal" => 1,
            package => panic!("unexpected graph-workbench package `{package}`"),
        };
        assert_eq!(
            entry
                .obligation_ledger()
                .dependency_closure()
                .packages()
                .len(),
            expected_transitive_packages,
            "each ledger must retain its own exact transitive closure"
        );
    }

    let recovered =
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), limits)
            .expect("recover canonical reconstruction question");
    assert_eq!(recovered, question);
    assert_eq!(recovered.fingerprint(), question.fingerprint());
    assert!(
        recovered
            .matches_resolved_and_reviews(&closure, &reviews, limits)
            .expect("fresh source and review reconstruction should succeed")
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn exact_nested_source_request_changes_question_with_identical_ledgers_and_fresh_match_rejects() {
    let (temporary, _closure, reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let fixture_root = workspace_root().join("tests/fixtures/packages");
    let alternate_request_spelling = fixture_root.join(".");
    let workspace_lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let alternate_closure = resolve_workspace_package_closure(
        &workspace_lineage,
        WorkspaceMemberPath::parse("graph-workbench").unwrap(),
        &alternate_request_spelling,
        temporary.join("alternate-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve the same source through alternate exact request spelling");
    let alternate = CanonicalPackageReconstructionQuestion::from_resolved_and_reviews(
        &alternate_closure,
        &reviews,
        limits,
    )
    .expect("reuse identical locally reconstructed ledgers with alternate source request");

    assert_ne!(
        question.source_closure().canonical_bytes(),
        alternate.source_closure().canonical_bytes(),
        "exact caller request spelling belongs to the nested source subject"
    );
    assert!(
        question
            .entries()
            .iter()
            .map(|entry| entry.obligation_ledger())
            .eq(alternate
                .entries()
                .iter()
                .map(|entry| entry.obligation_ledger())),
        "the alternate question reuses byte-identical obligation ledgers"
    );
    assert_ne!(question.canonical_bytes(), alternate.canonical_bytes());
    assert_ne!(question.fingerprint(), alternate.fingerprint());
    assert!(
        !question
            .matches_resolved_and_reviews(&alternate_closure, &reviews, limits)
            .expect("alternate fresh reconstruction remains structurally valid"),
        "fresh match must reject a different exact source question"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_missing_duplicate_reordered_and_source_inconsistent_ledgers() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();
    let (version, source, ledgers) = split_question(question.canonical_bytes());
    assert_eq!(ledgers.len(), 3);

    let mut missing = ledgers.clone();
    missing.pop();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &missing),
            limits,
        )
        .is_err()
    );

    let mut duplicate = ledgers.clone();
    duplicate[2] = duplicate[0].clone();
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &duplicate),
            limits,
        )
        .is_err()
    );

    let mut reordered = ledgers.clone();
    reordered.swap(0, 1);
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            &join_question(version, &source, &reordered),
            limits,
        )
        .is_err()
    );

    let mut changed_alias = ledgers.clone();
    let graph_ledger = changed_alias
        .iter_mut()
        .find(|ledger| find_subslice(ledger, b"arithmetic_kernels").is_some())
        .expect("root ledger retains dependency alias");
    let alias_offset =
        find_subslice(graph_ledger, b"arithmetic_kernels").expect("root ledger alias offset");
    graph_ledger[alias_offset] = b'b';
    let error = CanonicalPackageReconstructionQuestion::recover(
        &join_question(version, &source, &changed_alias),
        limits,
    )
    .expect_err("source-inconsistent dependency alias must reject");
    assert_eq!(
        error.message(),
        "obligation ledger dependency edges do not match the source subject"
    );

    remove_temporary_tree(&temporary);
}

#[test]
fn recovery_rejects_unknown_version_trailing_bytes_and_resource_violations() {
    let (temporary, _closure, _reviews, question) = graph_workbench_question();
    let limits = CanonicalPackageReconstructionQuestionLimits::default();

    let mut unknown_version = question.canonical_bytes().to_vec();
    unknown_version[QUESTION_MAGIC.len()..QUESTION_MAGIC.len() + 2]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(CanonicalPackageReconstructionQuestion::recover(&unknown_version, limits).is_err());

    let mut trailing = question.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(CanonicalPackageReconstructionQuestion::recover(&trailing, limits).is_err());

    let record_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_record_bytes: question.canonical_bytes().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), record_bound,)
            .is_err()
    );

    let package_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_packages: question.entries().len() - 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), package_bound,)
            .is_err()
    );

    let ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(question.canonical_bytes(), ledger_bound,)
            .is_err()
    );

    let aggregate_ledger_bound = CanonicalPackageReconstructionQuestionLimits {
        maximum_total_ledger_bytes: 1,
        ..limits
    };
    assert!(
        CanonicalPackageReconstructionQuestion::recover(
            question.canonical_bytes(),
            aggregate_ledger_bound,
        )
        .is_err()
    );

    remove_temporary_tree(&temporary);
}

fn split_question(bytes: &[u8]) -> (u16, Vec<u8>, Vec<Vec<u8>>) {
    let mut offset = 0usize;
    assert_eq!(&bytes[..QUESTION_MAGIC.len()], QUESTION_MAGIC);
    offset += QUESTION_MAGIC.len();
    let version = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    offset += 2;
    let source = take_frame(bytes, &mut offset).to_vec();
    let ledger_count = take_u32(bytes, &mut offset) as usize;
    let ledgers = (0..ledger_count)
        .map(|_| take_frame(bytes, &mut offset).to_vec())
        .collect::<Vec<_>>();
    assert_eq!(offset, bytes.len());
    (version, source, ledgers)
}

fn join_question(version: u16, source: &[u8], ledgers: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(QUESTION_MAGIC);
    bytes.extend_from_slice(&version.to_le_bytes());
    push_frame(&mut bytes, source);
    bytes.extend_from_slice(&(ledgers.len() as u32).to_le_bytes());
    for ledger in ledgers {
        push_frame(&mut bytes, ledger);
    }
    bytes
}

fn take_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    value
}

fn take_frame<'bytes>(bytes: &'bytes [u8], offset: &mut usize) -> &'bytes [u8] {
    let length = take_u32(bytes, offset) as usize;
    let framed = &bytes[*offset..*offset + length];
    *offset += length;
    framed
}

fn push_frame(bytes: &mut Vec<u8>, framed: &[u8]) {
    bytes.extend_from_slice(&(framed.len() as u32).to_le_bytes());
    bytes.extend_from_slice(framed);
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|candidate| candidate == needle)
}

fn remove_temporary_tree(root: &std::path::Path) {
    make_tree_owner_writable(root);
    std::fs::remove_dir_all(root).expect("remove temporary root");
}

#[cfg(unix)]
fn make_tree_owner_writable(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    if !metadata.is_dir() {
        return;
    }
    let mode = metadata.permissions().mode() | 0o700;
    let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode));
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            make_tree_owner_writable(&entry.path());
        }
    }
}

#[cfg(windows)]
fn make_tree_owner_writable(root: &std::path::Path) {
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    let _ = std::fs::set_permissions(root, permissions);
    if metadata.is_dir() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                make_tree_owner_writable(&entry.path());
            }
        }
    }
}
