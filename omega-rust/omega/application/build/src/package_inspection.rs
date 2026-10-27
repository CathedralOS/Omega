//! Execute an observational target roster under one sponsored session.

use crate::BuildSession;
use package_manager::PackageSourceBindings;
use package_manager::operations::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionSources, PackageInspectionTransaction, PreparedPackageInspection,
};

/// Keep package custody alive through every target and staging disposal.
/// A target's unavailable findings do not hide its siblings; report failures
/// abort the invocation. Inspection never grants publication or host authority.
pub fn inspect_packages(
    options: PackageInspectionOptions,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction = PackageInspectionTransaction::open(options)?;
    let mut session =
        BuildSession::create(&transaction.project_root().join("build/package-manager"))?;
    let result = (|| {
        let mut inspection = transaction.prepare(None)?;
        let (sources, mut completed) = match resolve_inspection_sources(&inspection, &mut session) {
            Ok((sources, completed)) => (Ok(sources), completed),
            Err(error) => (Err(error), Vec::new()),
        };
        let mut source_bindings = PackageSourceBindings::default();
        for target in inspection.targets().to_vec() {
            inspection.review_target(
                target,
                sources.as_ref(),
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
        }
        inspection.finish()
    })();
    session.dispose(result)
}

/// Source execution is shared across target findings, including failed targets.
/// Failure stays report data; the transaction still verifies unchanged files.
fn resolve_inspection_sources(
    inspection: &PreparedPackageInspection<'_>,
    session: &mut BuildSession,
) -> Result<(PackageInspectionSources, Vec<compiler::CompletedBuild>), PackageInspectionError> {
    let profile = target::TargetProfile::host_if_supported().ok_or_else(|| {
        PackageInspectionError::from("package Build execution requires a supported host".to_owned())
    })?;
    let mut acquisitions = inspection.source_acquisitions()?;
    let acquired = inspection.acquire_root()?;
    let entry = acquired.entry_path();
    let (resolved, builds) = crate::build_execution::source_resolution::resolve_acquired_sources(
        session,
        acquired,
        &entry,
        &mut acquisitions,
        profile,
        package_compilation::BuildExecutionInputs {
            snapshot: inspection.build_snapshot().cloned(),
            ..Default::default()
        },
        false,
    )
    .map_err(|error| PackageInspectionError::from(error.to_string()))?;
    let sources = inspection.complete_source_resolution(resolved)?;
    let completed = crate::build_execution::source_resolution::settle_source_builds(
        builds,
        &mut session.compiler,
        profile,
    )
    .map_err(|error| PackageInspectionError::from(error.to_string()))?;
    Ok((sources, completed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_custody::temporary_directory::TemporaryDirectory;

    #[test]
    fn inspection_checks_receipt_roster_and_final_file_identity() {
        let project = TemporaryDirectory::create("audit-transaction", false).unwrap();
        std::fs::write(project.path().join("build.omg"),
            "machine build(builder: &mut Build) { Build::application(builder, \"audit_transaction\"); }\n").unwrap();
        std::fs::write(
            project.path().join("main.omg"),
            "machine main() -> u32 { 0u32 }\n",
        )
        .unwrap();
        let profile = target::TargetProfile::host_if_supported().unwrap();
        let transaction = PackageInspectionTransaction::open(PackageInspectionOptions {
            project_root: project.path().to_path_buf(),
            targets: vec![profile],
            details: false,
            offline: true,
            build_inputs: None,
        })
        .unwrap();
        let mut session =
            BuildSession::create(&project.path().join("build/package-manager")).unwrap();
        let first = transaction.prepare(None).unwrap();
        let (sources, _) = resolve_inspection_sources(&first, &mut session).unwrap();
        let mut second = transaction.prepare(None).unwrap();
        let mut bindings = PackageSourceBindings::default();
        fn unexpected_review(
            _: package_manager::review::PackageReviewPass<'_>,
            _: &mut PackageSourceBindings,
        ) -> Result<
            package_manager::review::RetainedPackageReviews,
            package_manager::review::CompileResolvedPackageReviewsError,
        > {
            panic!("invalid or unavailable sources must not reach review")
        }
        let error = second
            .review_target(
                profile,
                Ok(&sources),
                &mut session.packages,
                &mut bindings,
                &mut unexpected_review,
            )
            .unwrap_err();
        assert!(error.to_string().contains("another inspection"));
        assert!(
            first
                .finish()
                .unwrap_err()
                .to_string()
                .contains("every admitted target")
        );

        let unavailable = PackageInspectionError::from("source execution refused".to_owned());
        second
            .review_target(
                profile,
                Err(&unavailable),
                &mut session.packages,
                &mut bindings,
                &mut unexpected_review,
            )
            .unwrap();
        let error = second
            .review_target(
                profile,
                Err(&unavailable),
                &mut session.packages,
                &mut bindings,
                &mut unexpected_review,
            )
            .unwrap_err();
        assert!(error.to_string().contains("next admitted target"));
        std::fs::write(
            project.path().join("build.omg"),
            "// changed during inspection\n",
        )
        .unwrap();
        assert!(
            second
                .finish()
                .unwrap_err()
                .to_string()
                .contains("changed during inspection")
        );
        session.close().unwrap();
    }
}
