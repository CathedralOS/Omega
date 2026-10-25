//! Package commands schedule review work here; package management owns the
//! admitted requests, editable decisions and accepted-file transaction.

use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandError, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandTransaction, discard_package_review,
};

/// Run all target reviews before returning findings to package publication.
/// The caller lends one sponsored session across automatic resume rounds;
/// pending decisions neither dispose it nor replenish its resource grant.
pub fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
    session: &mut PackageBuildSession,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    if matches!(command, PackageCommand::DiscardReview) {
        return discard_package_review(&options.project_root);
    }
    let mut transaction = PackageCommandTransaction::open(options)?;
    let prepared = transaction.prepare(command, None)?;
    let mut source_bindings = prepared.source_bindings();
    let mut reviews = Vec::new();
    for request in prepared.review_requests() {
        let review = request.review(session, &mut source_bindings)?;
        reviews.push(review);
    }
    prepared.complete(reviews, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::path::Path;
    use target::TargetProfile;

    fn project() -> TemporaryDirectory {
        let project = TemporaryDirectory::create("command-review", false).expect("test directory");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../tests/omega/pass/calls/machine_self_call_recursion_compile");
        for name in ["main.omg", "build.omg"] {
            std::fs::copy(fixture.join(name), project.path().join(name)).expect("fixture copy");
        }
        project
    }

    fn options(project: &Path) -> PackageCommandOptions {
        PackageCommandOptions {
            project_root: project.to_path_buf(),
            targets: vec![
                TargetProfile::from_canonical_target_name("linux_x86_64").expect("Linux target"),
                TargetProfile::from_canonical_target_name("macos_arm64").expect("macOS target"),
            ],
            offline: true,
            build_inputs: None,
        }
    }

    fn update() -> PackageCommand {
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        }
    }

    fn assert_unpublished(project: &Path) {
        assert!(!project.join("omega.lock").exists());
        assert!(!project.join("build/package-manager/proposal").exists());
        for target in ["linux_x86_64", "macos_arm64"] {
            assert!(
                !project
                    .join(format!("build/package-manager/review-{target}.txt"))
                    .exists()
            );
        }
    }

    #[test]
    fn missing_reviews_leave_decisions_and_accepted_files_untouched() {
        let project = project();
        let mut transaction =
            PackageCommandTransaction::open(options(project.path())).expect("transaction");
        let prepared = transaction
            .prepare(update(), None)
            .expect("admitted command");
        assert_eq!(prepared.review_requests().count(), 2);
        let mut session =
            PackageBuildSession::create(&project.path().join("build/package-manager"))
                .expect("session");
        let error = prepared
            .complete(Vec::new(), &mut session)
            .expect_err("incomplete roster");
        assert!(error.to_string().contains("every requested target review"));
        assert_unpublished(project.path());
        session.close().expect("dispose staging");
    }

    #[test]
    fn reordered_reviews_cannot_publish() {
        let project = project();
        let mut transaction =
            PackageCommandTransaction::open(options(project.path())).expect("transaction");
        let prepared = transaction
            .prepare(update(), None)
            .expect("admitted command");
        let mut session =
            PackageBuildSession::create(&project.path().join("build/package-manager"))
                .expect("session");
        let mut bindings = prepared.source_bindings();
        let mut reviews = prepared
            .review_requests()
            .map(|request| {
                request
                    .review(&mut session, &mut bindings)
                    .expect("review target")
            })
            .collect::<Vec<_>>();
        reviews.reverse();
        let error = prepared
            .complete(reviews, &mut session)
            .expect_err("wrong roster order");
        assert!(error.to_string().contains("wrong target"));
        assert_unpublished(project.path());
        session.close().expect("dispose staging");
    }

    #[test]
    fn another_commands_reviews_cannot_publish_even_with_identical_inputs() {
        let project = project();
        let mut session =
            PackageBuildSession::create(&project.path().join("build/package-manager"))
                .expect("session");
        let reviews = {
            let mut transaction = PackageCommandTransaction::open(options(project.path()))
                .expect("first transaction");
            let prepared = transaction.prepare(update(), None).expect("first command");
            let mut bindings = prepared.source_bindings();
            prepared
                .review_requests()
                .map(|request| {
                    request
                        .review(&mut session, &mut bindings)
                        .expect("review target")
                })
                .collect::<Vec<_>>()
        };
        let mut transaction =
            PackageCommandTransaction::open(options(project.path())).expect("second transaction");
        let prepared = transaction.prepare(update(), None).expect("second command");
        let error = prepared
            .complete(reviews, &mut session)
            .expect_err("stale command receipt");
        assert!(error.to_string().contains("another command"));
        assert_unpublished(project.path());
        session.close().expect("dispose staging");
    }
}
