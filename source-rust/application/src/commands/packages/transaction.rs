//! Hold the project transaction through source discovery and every admitted target review.
use package_manager::operations::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionTransaction,
};
use package_manager::project::{RootBuildInputs, resolve_build_sources};
use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandAdmission, PackageCommandError,
    PackageCommandOptions, PackageCommandOutcome, PackageCommandTransaction, PackageSourceBindings,
};

pub(super) fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
    session: &mut PackageBuildSession,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let programs = options.programs.clone();
    let mut transaction = PackageCommandTransaction::open(options)?;
    let preparation = match transaction.prepare(command, None)? {
        PackageCommandAdmission::Published(outcome) => return Ok(outcome),
        PackageCommandAdmission::Review(preparation) => *preparation,
    };
    let prepared = match preparation.resolve_sources(session, programs)? {
        package_manager::transaction::CommandSourceReview::Pending(outcome) => return Ok(outcome),
        package_manager::transaction::CommandSourceReview::Resolved(prepared) => *prepared,
    };
    let mut bindings = prepared.source_bindings();
    let mut reviews = Vec::new();
    for request in prepared.review_requests() {
        reviews.push(request.review(session, &mut bindings)?);
    }
    prepared.complete(reviews, session)
}

pub(super) fn inspect_packages(
    options: PackageInspectionOptions,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let programs = options.programs.clone();
    let transaction = PackageInspectionTransaction::open(options)?;
    let mut session =
        PackageBuildSession::create(&transaction.project_root().join("build/package-manager"))?;
    let result = (|| {
        let mut inspection = transaction.prepare(None)?;
        let sources = (|| {
            let acquired = inspection.acquire_root()?;
            let sources = resolve_build_sources(
                &mut session,
                acquired,
                &mut inspection.source_acquisitions()?,
                RootBuildInputs { snapshot: inspection.build_snapshot().cloned(), programs },
            )?;
            inspection.complete_source_resolution(sources)
        })();
        let mut bindings = PackageSourceBindings::default();
        for target in inspection.targets().to_vec() {
            inspection.review_target(target, sources.as_ref(), &mut session, &mut bindings)?;
        }
        inspection.finish()
    })();
    session.dispose(result)
}
