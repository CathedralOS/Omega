//! Command acceptance uses real staged source and compiler-generated findings.

use super::*;
use package_manager::operations::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandStatus, execute_package_command_with_storage,
};

#[path = "commands/review.rs"]
mod review;
#[path = "commands/source_diff.rs"]
mod source_diff;
#[path = "commands/stale.rs"]
mod stale;
#[path = "commands/targets.rs"]
mod targets;

fn fixture(dependency_source: &str) -> Tree {
    let tree = Tree::new();
    source(&tree, PURE, "");
    package(&tree.path("sources/dependency"), "command-dependency", "");
    fs::write(tree.path("sources/dependency/main.omg"), dependency_source).unwrap();
    tree
}

fn execute(
    tree: &Tree,
    command: PackageCommand,
    targets: Vec<TargetProfile>,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    execute_package_command_with_storage(
        command,
        PackageCommandOptions {
            project_root: tree.path("sources/root"),
            targets,
        },
        &tree.storage("command-cache"),
    )
}

fn install() -> PackageCommand {
    PackageCommand::Install {
        source: "../dependency".into(),
        revision: None,
        alias: Some("dependency".into()),
        package: None,
    }
}

fn update() -> PackageCommand {
    PackageCommand::Update {
        packages: Vec::new(),
        revision: None,
    }
}

fn resume(
    tree: &Tree,
    kind: PackageCommandKind,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    execute(tree, PackageCommand::Resume { kind }, Vec::new())
}

fn accepted_files(tree: &Tree) -> (Vec<u8>, Option<Vec<u8>>) {
    let build = fs::read(tree.path("sources/root/build.omg")).unwrap();
    let lock = match fs::read(tree.path("sources/root/omega.lock")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("read accepted lock: {error}"),
    };
    (build, lock)
}

fn lock(tree: &Tree) -> PackageLock {
    let text = fs::read_to_string(tree.path("sources/root/omega.lock")).unwrap();
    let lock = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(lock.canonical_text().unwrap(), text);
    lock
}

fn proposal_path(tree: &Tree) -> PathBuf {
    tree.path("sources/root/build/package-manager/proposal")
}

fn documents(outcome: &PackageCommandOutcome) -> Vec<String> {
    outcome
        .review_paths
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect()
}

fn edit_decisions(path: &Path, choice: &str) -> usize {
    assert!(matches!(choice, "pending" | "accept" | "reject"));
    let before = fs::read_to_string(path).unwrap();
    let mut count = 0;
    let after: String = before
        .split_inclusive('\n')
        .map(|line| {
            if line.starts_with("decision ") {
                let (prefix, previous) = line.strip_suffix('\n').unwrap().rsplit_once(' ').unwrap();
                assert!(matches!(previous, "pending" | "accept" | "reject"));
                count += 1;
                format!("{prefix} {choice}\n")
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_eq!(
        before
            .lines()
            .filter(|line| !line.starts_with("decision "))
            .collect::<Vec<_>>(),
        after
            .lines()
            .filter(|line| !line.starts_with("decision "))
            .collect::<Vec<_>>()
    );
    fs::write(path, after).unwrap();
    count
}

fn accept(outcome: &PackageCommandOutcome) {
    let count: usize = outcome
        .review_paths
        .iter()
        .map(|path| edit_decisions(path, "accept"))
        .sum();
    assert!(count > 0, "fixture must exercise actual required decisions");
}

fn pending_install(tree: &Tree) -> PackageCommandOutcome {
    let before = accepted_files(tree);
    let outcome = execute(tree, install(), vec![TARGET]).unwrap();
    assert_eq!(
        outcome.status,
        PackageCommandStatus::ReviewRequired,
        "{}",
        outcome.report
    );
    assert_eq!(outcome.review_paths.len(), 1);
    assert!(proposal_path(tree).is_file());
    assert_eq!(accepted_files(tree), before);
    outcome
}
