//! Execute an observational target roster under one sponsored session.

use crate::BuildSession;
use package_manager::PackageSourceBindings;
use package_manager::operations::{
    PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
    PackageInspectionTransaction,
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
        let mut source_bindings = PackageSourceBindings::default();
        for target in inspection.targets().to_vec() {
            inspection.review_target(
                target,
                &mut session.packages,
                &mut source_bindings,
                &mut |plan, packages, bindings| {
                    crate::package_review::execute_pass(
                        plan,
                        &mut session.compiler,
                        packages,
                        bindings,
                    )
                },
            )?;
        }
        inspection.finish()
    })();
    session.dispose(result)
}
