use std::fs;
use std::path::PathBuf;

use crate::repo_root;

use super::board_and_tasks::{
    DANGLING_BASELINE, TASK_BOARDS, board_task_ids, citing_documents, dangling_task_ids,
    dangling_task_violations, read_dangling_baseline,
};
use super::commit_metadata::{
    gate_violations, lanes, proof_violations, provenance_violations, spec_violations,
    subject_violations,
};
use super::diff_hygiene::diff_hygiene_violations;
use super::ratchets::{
    CHECKING_STATE_ROOTS, RATCHET_SPECS, checking_interior_state_violations,
    compute_ratchet_counts, count_place_transport, ratchet_violations, read_baseline,
    semantic_id_names,
};
use super::{git, pr_diff_base, show_git_file};

#[test]
fn interior_state_scan_reports_an_unreadable_configured_root() {
    let scratch = std::env::temp_dir().join(format!(
        "omega-interior-state-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock precedes the unix epoch")
            .as_nanos()
    ));
    fs::create_dir(&scratch).unwrap();
    let missing = checking_interior_state_violations(&scratch);

    for dir in CHECKING_STATE_ROOTS {
        let stage = scratch.join(dir);
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join("hidden.rs"), "let cell = RefCell::new(1);\n").unwrap();
    }
    let found = checking_interior_state_violations(&scratch);

    for dir in CHECKING_STATE_ROOTS {
        fs::write(scratch.join(dir).join("hidden.rs"), "fn plain() {}\n").unwrap();
    }
    let clean = checking_interior_state_violations(&scratch);
    let _ = fs::remove_dir_all(&scratch);

    assert_eq!(missing.len(), CHECKING_STATE_ROOTS.len());
    for dir in CHECKING_STATE_ROOTS {
        assert!(
            missing.iter().any(|violation| violation.starts_with(dir)
                && violation.contains("could not read this root")),
            "{dir} missing from {missing:?}"
        );
    }
    assert!(
        found
            .iter()
            .any(|violation| violation.contains("hidden.rs") && violation.contains("RefCell")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .all(|violation| !violation.contains("could not read this root")),
        "{found:?}"
    );
    assert!(clean.is_empty(), "{clean:?}");
}

#[test]
fn place_transport_counts_comparisons_rebasers_and_place_types() {
    let text = "\
pub(super) fn owner_path_overlaps(a: u8) -> bool { true }
fn rebase_place(a: u8) {}
pub struct CapturedPlaceSegment;
enum CheckedPath { A }
fn unrelated() {}
struct Ledger;
// fn commented_overlap() {}
";
    assert_eq!(count_place_transport(text), 4);
}

#[test]
fn ratchets_and_dangling_ids_match_checked_in_baselines() {
    let root = repo_root().expect("test runs in the Omega workspace");
    let semantic_ids = semantic_id_names(&root);
    for (idx, spec) in RATCHET_SPECS.iter().enumerate() {
        let counts = compute_ratchet_counts(&root, idx, &semantic_ids);
        let baseline_text = fs::read_to_string(root.join(spec.baseline)).unwrap();
        let baseline = read_baseline(&baseline_text);
        let violations = ratchet_violations(spec, &counts, &baseline);
        assert!(
            violations.is_empty(),
            "ratchet {} differed from baseline:\n{}",
            spec.baseline,
            violations.join("\n")
        );
    }
    let boards: Vec<String> = TASK_BOARDS
        .iter()
        .map(|b| fs::read_to_string(root.join(b)).unwrap_or_default())
        .collect();
    let dangling = dangling_task_ids(&citing_documents(&root), &board_task_ids(&boards));
    let dangling_baseline =
        read_dangling_baseline(&fs::read_to_string(root.join(DANGLING_BASELINE)).unwrap());
    let violations = dangling_task_violations(&dangling, &dangling_baseline);
    assert!(
        violations.is_empty(),
        "dangling task IDs differed:\n{}",
        violations.join("\n")
    );
}

struct ScratchRepository {
    root: PathBuf,
}

impl ScratchRepository {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("omega-check-rules-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "--quiet"]).unwrap();
        for (key, value) in [
            ("user.name", "Check rules test"),
            ("user.email", "check-rules@example.invalid"),
            ("commit.gpgSign", "false"),
            ("core.hooksPath", ".git/empty-hooks"),
        ] {
            git(&root, &["config", key, value]).unwrap();
        }
        Self { root }
    }

    fn commit(&self, files: &[(&str, Option<&str>)]) -> String {
        for (path, contents) in files {
            let path = self.root.join(path);
            if let Some(contents) = contents {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, contents).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
        git(&self.root, &["add", "--all"]).unwrap();
        git(&self.root, &["commit", "--quiet", "-m", "fixture"]).unwrap();
        git(&self.root, &["rev-parse", "HEAD"])
            .unwrap()
            .trim()
            .to_owned()
    }

    fn changes_since(&self, base: &str) -> Vec<(char, String)> {
        git(
            &self.root,
            &["diff", "--name-status", "--no-renames", base, "HEAD"],
        )
        .unwrap()
        .lines()
        .map(|line| {
            let (status, path) = line.split_once('\t').unwrap();
            (status.chars().next().unwrap(), path.to_owned())
        })
        .collect()
    }
}

impl Drop for ScratchRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn pr_scope_excludes_files_added_to_main_after_the_topic_forked() {
    let repo = ScratchRepository::new();
    let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
    let main = repo.commit(&[
        ("notes/new_on_main/main.md", Some("fixture\n")),
        (
            "notes/new_on_main/expected.md",
            Some("required diagnostic\n"),
        ),
    ]);
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/main", &main],
    )
    .unwrap();
    git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
    repo.commit(&[("topic.md", Some("independent topic change\n"))]);

    let base = pr_diff_base(&repo.root, Some("origin/main")).unwrap();
    let changes = repo.changes_since(&base);
    assert_eq!(changes, [('A', "topic.md".to_owned())]);
    assert_eq!(pr_diff_base(&repo.root, None).unwrap(), fork);
}

#[test]
fn pr_scope_preserves_ancestor_and_admission_merge_parent_diffs() {
    let repo = ScratchRepository::new();
    let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
    let topic = repo.commit(&[("topic.md", Some("topic change\n"))]);
    let ancestor = pr_diff_base(&repo.root, Some(&fork)).unwrap();
    assert_eq!(
        repo.changes_since(&ancestor),
        [('A', "topic.md".to_owned())]
    );
    git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
    let main = repo.commit(&[("main.md", Some("main change\n"))]);
    git(
        &repo.root,
        &[
            "merge",
            "--quiet",
            "--no-ff",
            &topic,
            "-m",
            "admission merge",
        ],
    )
    .unwrap();
    let parent = pr_diff_base(&repo.root, Some("HEAD^1")).unwrap();
    assert_eq!(repo.changes_since(&parent), [('A', "topic.md".to_owned())]);
    assert_eq!(show_git_file(&repo.root, &main, "main.md"), "main change\n");
    // Admission checks a shallow checkout of GitHub's synthetic merge.
    git(
        &repo.root,
        &[
            "clone",
            "--quiet",
            "--no-local",
            "--depth=2",
            ".",
            "shallow",
        ],
    )
    .unwrap();
    let shallow = repo.root.join("shallow");
    assert_eq!(
        git(&shallow, &["rev-parse", "--is-shallow-repository"])
            .unwrap()
            .trim(),
        "true"
    );
    let parent = pr_diff_base(&shallow, Some("HEAD^1")).unwrap();
    assert_eq!(
        git(&shallow, &["diff", "--name-only", &parent, "HEAD"]).unwrap(),
        "topic.md\n"
    );
}

#[test]
fn pr_scope_refuses_invalid_or_unrelated_bases_instead_of_empty_coverage() {
    let repo = ScratchRepository::new();
    repo.commit(&[("shared.md", Some("shared\n"))]);
    assert!(pr_diff_base(&repo.root, Some("missing-reference")).is_err());
    assert!(pr_diff_base(&repo.root, None).is_err());
    let tree = git(&repo.root, &["rev-parse", "HEAD^{tree}"]).unwrap();
    let unrelated = git(
        &repo.root,
        &["commit-tree", tree.trim(), "-m", "unrelated root"],
    )
    .unwrap();
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/main", unrelated.trim()],
    )
    .unwrap();
    assert!(pr_diff_base(&repo.root, None).is_err());
}

#[test]
fn pr_scope_requires_an_explicit_ancestor_when_history_has_two_merge_bases() {
    let repo = ScratchRepository::new();
    let fork = repo.commit(&[("shared.md", Some("shared\n"))]);
    let left = repo.commit(&[("left.md", Some("left\n"))]);
    git(&repo.root, &["checkout", "--quiet", "--detach", &fork]).unwrap();
    let right = repo.commit(&[("right.md", Some("right\n"))]);
    let tree = git(&repo.root, &["rev-parse", "HEAD^{tree}"]).unwrap();
    let cross = |first: &str, second: &str| {
        git(
            &repo.root,
            &[
                "commit-tree",
                tree.trim(),
                "-p",
                first,
                "-p",
                second,
                "-m",
                "cross merge",
            ],
        )
        .unwrap()
        .trim()
        .to_owned()
    };
    let main = cross(&left, &right);
    let topic = cross(&right, &left);
    git(
        &repo.root,
        &["update-ref", "refs/remotes/origin/main", &main],
    )
    .unwrap();
    git(&repo.root, &["checkout", "--quiet", "--detach", &topic]).unwrap();
    assert_eq!(
        git(&repo.root, &["merge-base", "--all", "origin/main", "HEAD"])
            .unwrap()
            .lines()
            .count(),
        2
    );
    assert!(pr_diff_base(&repo.root, Some("origin/main")).is_err());
    assert_eq!(pr_diff_base(&repo.root, Some(&left)).unwrap(), left);
}

#[test]
fn validates_commit_titles_and_provenance() {
    let root = repo_root().expect("test runs in the Omega workspace");
    let allowed = lanes(&fs::read_to_string(root.join("AGENTS.md")).unwrap());
    assert!(subject_violations("tools: port repository checks to rust", &allowed).is_empty());
    assert!(!subject_violations("unknownlane: bad", &allowed).is_empty());
    assert!(provenance_violations("Summary\n\nAgent: Antigravity | Model: Gemini 3.1 Pro | Effort: high | Tokens: 1.2M in (1.1M cached), 24k out | Steps: 12 | Time: 8m | Steers: 0\n").is_empty());
    assert!(provenance_violations("Summary\n\nAgent: Antigravity, main-rs\n").is_empty());
    assert!(!provenance_violations("Summary without agent line\n").is_empty());
}

#[test]
fn retired_corpus_and_frozen_facts_accept_no_new_files() {
    let added = |path: &str| diff_hygiene_violations(&[('A', path.to_owned())]);
    assert_eq!(added("tests/omega/pass/a/x/main.omg").len(), 1);
    assert_eq!(added("tests/omega/corpus_outcomes.txt").len(), 1);
    assert_eq!(added("samples/cli/facts/a/local/main.omg").len(), 1);
    assert!(added("tests/fixtures/pass/a/x/main.omg").is_empty());
    // Deleting from the retired tree is how it goes away.
    assert!(
        diff_hygiene_violations(&[('D', "tests/omega/pass/a/x/main.omg".to_owned())]).is_empty()
    );
    // A move counts as nothing added: the cap is on the net count.
    let mut moved = Vec::new();
    for index in 0..10 {
        moved.push(('D', format!("tests/omega/pass/a/x{index}/main.omg")));
        moved.push(('A', format!("samples/cli/a/x{index}/main.omg")));
    }
    assert!(diff_hygiene_violations(&moved).is_empty());
    moved.retain(|(status, _)| *status == 'A');
    assert_eq!(diff_hygiene_violations(&moved).len(), 1);
}

#[test]
fn frozen_facts_may_move_but_not_grow() {
    // A twin moved out of the program that contains it: one deletion
    // and one addition inside the set.
    let moved = [
        ('D', "samples/cli/facts/a/local/invalid/main.omg".to_owned()),
        ('A', "samples/cli/facts/a/invalid_local/main.omg".to_owned()),
    ];
    assert!(diff_hygiene_violations(&moved).is_empty());
    // An addition beside a move still grows the set.
    let grown = [
        moved[0].clone(),
        moved[1].clone(),
        ('A', "samples/cli/facts/a/extra/main.omg".to_owned()),
    ];
    assert_eq!(diff_hygiene_violations(&grown).len(), 2);
    // A deletion outside the set makes no room inside it.
    let elsewhere = [
        ('D', "tests/fixtures/pass/a/main.omg".to_owned()),
        ('A', "samples/cli/facts/a/extra/main.omg".to_owned()),
    ];
    assert_eq!(diff_hygiene_violations(&elsewhere).len(), 1);
}

#[test]
fn proof_line_states_the_tests_and_what_they_showed() {
    let check = |body: &str| proof_violations("psi: keep a bound", body);
    assert!(check("Proof: codec round trips and the two lowered-route tests | clean\n").is_empty());
    // A line written while a blast count was still required stays valid.
    assert!(check("Proof: unit tests | blast: 6 fixtures | clean\n").is_empty());
    // Missing and incomplete.
    assert!(!check("Summary\n").is_empty());
    assert!(!check("Proof: unit tests\n").is_empty());
    assert!(!check("Proof: unit tests | \n").is_empty());
    // Other lanes carry no proof line.
    assert!(proof_violations("docs: explain a page", "Summary\n").is_empty());
}

#[test]
fn spec_line_names_an_existing_page_or_says_why_none_governs() {
    let root = repo_root().expect("test runs in the Omega workspace");
    let check = |body: &str| spec_violations("psi: fix", body, &root);
    assert!(check("Spec: wiki/spec/language/domains.md#domain-only-casts\n").is_empty());
    assert!(
        check("Spec: wiki/spec/language/domains.md, wiki/spec/language/ownership.md\n").is_empty()
    );
    assert!(check("Spec: none | refusal placement is not specified\n").is_empty());
    assert!(!check("Spec: none\n").is_empty());
    assert!(!check("Spec: none | \n").is_empty());
    assert!(!check("Spec: wiki/spec/language/no_such_page.md\n").is_empty());
    assert!(!check("Spec: omega-rust/pipeline.md\n").is_empty());
    assert!(!check("No spec line\n").is_empty());
    assert!(spec_violations("docs: explain", "No spec line\n", &root).is_empty());
}

#[test]
fn gate_line_must_name_a_gate_and_what_the_change_makes_work() {
    let check = |body| gate_violations("psi: fix", body);
    assert!(check("Gate: RC-SOURCE-SEMANTICS | generic bounds carry through a call\n").is_empty());
    // A line written while a red-row count was still required stays valid.
    assert!(check("Gate: RC-NATIVE-MATRIX | red rows: 0 | unlocks: pass/a/x\n").is_empty());
    assert!(!check("Gate: RC-MADE-UP | something\n").is_empty());
    assert!(!check("Gate: RC-NATIVE-MATRIX\n").is_empty());
    assert!(!check("No gate line\n").is_empty());
    assert!(gate_violations("docs: explain", "No gate line\n").is_empty());
}
