//! Hold the project transaction through source discovery and every admitted target review.
use crate::project_sources::resolve;
use package_manager::operations::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionTransaction,
};
use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandError, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandTransaction, PackageSourceBindings,
};

pub(super) fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
    session: &mut PackageBuildSession,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let mut transaction = PackageCommandTransaction::open(options)?;
    let preparation = transaction.prepare(command, None)?;
    let acquired = preparation.acquire_root()?;
    let sources = resolve(
        session,
        &acquired.entry_path(),
        acquired,
        &mut preparation.source_acquisitions()?,
        preparation.build_snapshot().cloned(),
    )?
    .settle(session)?;
    let prepared = preparation.complete_source_resolution(sources)?;
    let mut bindings = prepared.source_bindings();
    let mut reviews = Vec::new();
    for request in prepared.review_requests() {
        reviews.push(
            request.review(session, &mut bindings, &mut |pass, bindings| {
                pass.recover_cached(bindings)
            })?,
        );
    }
    prepared.complete(reviews, session)
}

pub(super) fn inspect_packages(
    options: PackageInspectionOptions,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction = PackageInspectionTransaction::open(options)?;
    let mut session =
        PackageBuildSession::create(&transaction.project_root().join("build/package-manager"))?;
    let result = (|| {
        let mut inspection = transaction.prepare(None)?;
        let sources = (|| {
            let acquired = inspection.acquire_root()?;
            let sources = resolve(
                &mut session,
                &acquired.entry_path(),
                acquired,
                &mut inspection.source_acquisitions()?,
                inspection.build_snapshot().cloned(),
            )?
            .settle(&mut session)?;
            inspection.complete_source_resolution(sources)
        })();
        let mut bindings = PackageSourceBindings::default();
        for target in inspection.targets().to_vec() {
            inspection.review_target(
                target,
                sources.as_ref(),
                &mut session,
                &mut bindings,
                &mut |pass, bindings| pass.recover_cached(bindings),
            )?;
        }
        inspection.finish()
    })();
    session.dispose(result)
}
