//! Package commands schedule review work here; package management owns the
//! admitted requests, editable decisions and accepted-file transaction.

use crate::BuildSession;
use package_manager::{
    PackageCommand, PackageCommandError, PackageCommandOptions, PackageCommandOutcome,
    PackageCommandTransaction, discard_package_review,
};

/// Run all target reviews before returning findings to package publication.
/// The caller lends one sponsored session across automatic resume rounds;
/// pending decisions neither dispose it nor replenish its resource grant.
pub fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
    session: &mut BuildSession,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    if matches!(command, PackageCommand::DiscardReview) {
        return discard_package_review(&options.project_root);
    }
    let mut transaction = PackageCommandTransaction::open(options)?;
    let preparation = transaction.prepare(command, None)?;
    let (prepared, mut completed) = resolve_command_sources(preparation, session)?;
    let mut source_bindings = prepared.source_bindings();
    let mut reviews = Vec::new();
    for request in prepared.review_requests() {
        let review = request.review(
            &mut session.packages,
            &mut source_bindings,
            &mut |plan, bindings| {
                crate::package_review::execute_pass(
                    plan,
                    &mut session.compiler,
                    bindings,
                    &mut completed,
                )
            },
        )?;
        reviews.push(review);
    }
    prepared.complete(reviews, &mut session.packages)
}

/// Execute the staged candidate once before review selects target/purpose
/// contexts. Source graphs and proposal checks stay with package management.
fn resolve_command_sources<'workspace, 'storage>(
    preparation: package_manager::PackageCommandPreparation<'workspace, 'storage>,
    session: &mut BuildSession,
) -> Result<
    (
        package_manager::PreparedPackageCommand<'workspace, 'storage>,
        Vec<compiler::CompletedBuild>,
    ),
    PackageCommandError,
> {
    let profile = target::TargetProfile::host_if_supported().ok_or_else(|| {
        PackageCommandError::from("package Build execution requires a supported host".to_owned())
    })?;
    let acquired = preparation.acquire_root()?;
    let entry = acquired.entry_path();
    let mut acquisitions = preparation.source_acquisitions()?;
    let (resolved, builds) = crate::build_execution::source_resolution::resolve_acquired_sources(
        session,
        acquired,
        &entry,
        &mut acquisitions,
        profile,
        preparation.build_snapshot().cloned(),
        false,
    )
    .map_err(|error| PackageCommandError::from(error.to_string()))?;
    drop(acquisitions);
    let prepared = preparation.complete_source_resolution(resolved)?;
    let completed = crate::build_execution::source_resolution::settle_source_builds(
        builds,
        &mut session.compiler,
        profile,
    )
    .map_err(|error| PackageCommandError::from(error.to_string()))?;
    Ok((prepared, completed))
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
    fn update_and_resume_retain_the_application_session_until_publication() {
        use package_manager::{PackageCommandKind, PackageCommandStatus};

        let project = project();
        let workspace = project.path().join("build/package-manager");
        let mut baseline = BuildSession::create(&workspace).expect("baseline session");
        let accepted = execute_package_command(update(), options(project.path()), &mut baseline)
            .expect("accept the ordinary application without policy changes");
        assert_eq!(accepted.status, PackageCommandStatus::Published);
        let lock_before = std::fs::read(project.path().join("omega.lock")).expect("accepted lock");
        let declaration_path = project.path().join("build.omg");
        let declaration = std::fs::read_to_string(&declaration_path).expect("Build source");
        std::fs::write(
            &declaration_path,
            declaration.replace("Build::application(", "Build::package("),
        )
        .expect("a root role change requires an explicit decision");
        let mut session = BuildSession::create(&project.path().join("build/package-manager"))
            .expect("application session");
        let sponsor = session.resources().evaluation_sponsor().clone();
        let pending = execute_package_command(update(), options(project.path()), &mut session)
            .expect("initial review");
        assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
        assert!(!pending.review_paths.is_empty());
        assert_eq!(
            std::fs::read(project.path().join("omega.lock")).unwrap(),
            lock_before
        );
        let first_fuel = sponsor.consumed_fuel_units();
        assert!(first_fuel > 0);

        // A resume reads the saved target/input roster, not a fresh grant.
        let mut resume_options = options(project.path());
        resume_options.targets.clear();
        let resumed = execute_package_command(
            PackageCommand::Resume {
                kind: PackageCommandKind::Update,
            },
            resume_options.clone(),
            &mut session,
        )
        .expect("pending review resumes under the same compiler and sponsors");
        assert_eq!(resumed.status, PackageCommandStatus::ReviewRequired);
        assert!(sponsor.consumed_fuel_units() > first_fuel);
        for path in &resumed.review_paths {
            let document = std::fs::read_to_string(path).expect("review document");
            let mut accepted = String::with_capacity(document.len());
            for line in document.split_inclusive('\n') {
                let body = line.trim_end_matches('\n');
                if let Some(subject) = body.strip_suffix(" pending")
                    && subject.starts_with("decision ")
                {
                    accepted.push_str(subject);
                    accepted.push_str(" accept");
                    accepted.push_str(&line[body.len()..]);
                } else {
                    accepted.push_str(line);
                }
            }
            std::fs::write(path, accepted).expect("accept this test's reviewed code");
        }
        let published = execute_package_command(
            PackageCommand::Resume {
                kind: PackageCommandKind::Update,
            },
            resume_options,
            &mut session,
        )
        .expect("accepted review publishes through package policy");
        assert_eq!(published.status, PackageCommandStatus::Published);
        assert!(project.path().join("omega.lock").is_file());
        session
            .close()
            .expect("idempotent disposal after publication");
    }

    #[test]
    fn source_changes_after_review_cannot_publish() {
        let project = project();
        let mut transaction = PackageCommandTransaction::open(options(project.path())).unwrap();
        let mut session =
            BuildSession::create(&project.path().join("build/package-manager")).unwrap();
        let (prepared, mut completed) =
            resolve_command_sources(transaction.prepare(update(), None).unwrap(), &mut session)
                .unwrap();
        let mut bindings = prepared.source_bindings();
        let mut reviews = Vec::new();
        for request in prepared.review_requests() {
            reviews.push(
                request
                    .review(
                        &mut session.packages,
                        &mut bindings,
                        &mut |plan, bindings| {
                            crate::package_review::execute_pass(
                                plan,
                                &mut session.compiler,
                                bindings,
                                &mut completed,
                            )
                        },
                    )
                    .unwrap(),
            );
        }
        let source = project.path().join("main.omg");
        let changed = std::fs::read_to_string(&source).unwrap() + "\n// changed after review\n";
        std::fs::write(source, changed).unwrap();
        let error = prepared
            .complete(reviews, &mut session.packages)
            .unwrap_err();
        assert!(error.to_string().contains("changed"), "{error}");
        assert_unpublished(project.path());
        session.close().unwrap();
    }

    #[test]
    fn missing_reviews_leave_decisions_and_accepted_files_untouched() {
        let project = project();
        let mut transaction =
            PackageCommandTransaction::open(options(project.path())).expect("transaction");
        let mut session =
            BuildSession::create(&project.path().join("build/package-manager")).expect("session");
        let (prepared, _) =
            resolve_command_sources(transaction.prepare(update(), None).unwrap(), &mut session)
                .expect("admitted command");
        assert_eq!(prepared.review_requests().count(), 2);
        let error = prepared
            .complete(Vec::new(), &mut session.packages)
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
        let mut session =
            BuildSession::create(&project.path().join("build/package-manager")).expect("session");
        let (prepared, mut completed) =
            resolve_command_sources(transaction.prepare(update(), None).unwrap(), &mut session)
                .expect("admitted command");
        let mut bindings = prepared.source_bindings();
        let mut reviews = prepared
            .review_requests()
            .map(|request| {
                request
                    .review(
                        &mut session.packages,
                        &mut bindings,
                        &mut |plan, bindings| {
                            crate::package_review::execute_pass(
                                plan,
                                &mut session.compiler,
                                bindings,
                                &mut completed,
                            )
                        },
                    )
                    .expect("review target")
            })
            .collect::<Vec<_>>();
        reviews.reverse();
        let error = prepared
            .complete(reviews, &mut session.packages)
            .expect_err("wrong roster order");
        assert!(error.to_string().contains("wrong target"));
        assert_unpublished(project.path());
        session.close().expect("dispose staging");
    }

    #[test]
    fn another_commands_reviews_cannot_publish_even_with_identical_inputs() {
        let project = project();
        let mut session =
            BuildSession::create(&project.path().join("build/package-manager")).expect("session");
        let reviews = {
            let mut transaction = PackageCommandTransaction::open(options(project.path()))
                .expect("first transaction");
            let (prepared, mut completed) =
                resolve_command_sources(transaction.prepare(update(), None).unwrap(), &mut session)
                    .expect("first command");
            let mut bindings = prepared.source_bindings();
            prepared
                .review_requests()
                .map(|request| {
                    request
                        .review(
                            &mut session.packages,
                            &mut bindings,
                            &mut |plan, bindings| {
                                crate::package_review::execute_pass(
                                    plan,
                                    &mut session.compiler,
                                    bindings,
                                    &mut completed,
                                )
                            },
                        )
                        .expect("review target")
                })
                .collect::<Vec<_>>()
        };
        let mut transaction =
            PackageCommandTransaction::open(options(project.path())).expect("second transaction");
        let (prepared, _) =
            resolve_command_sources(transaction.prepare(update(), None).unwrap(), &mut session)
                .expect("second command");
        let error = prepared
            .complete(reviews, &mut session.packages)
            .expect_err("stale command receipt");
        assert!(error.to_string().contains("another command"));
        assert_unpublished(project.path());
        session.close().expect("dispose staging");
    }
}
