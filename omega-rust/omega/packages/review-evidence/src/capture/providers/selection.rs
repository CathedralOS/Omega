use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(crate) fn validate_selected_provider_declaration_owner(
    declaration: &PackageReviewNominalIdentity,
    expected_package: Option<PackageKeyIdentity>,
    plan_name: &str,
    role: &str,
) -> Result<(), Vec<Diagnostic>> {
    let matches = match (expected_package, declaration.owner) {
        (Some(expected), PackageReviewNominalOwner::Package(actual)) => expected == actual,
        (None, PackageReviewNominalOwner::ToolchainSource(_)) => true,
        (Some(_), PackageReviewNominalOwner::ToolchainSource(_))
        | (None, PackageReviewNominalOwner::Package(_))
        | (_, PackageReviewNominalOwner::Unresolved) => false,
    };
    if matches {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(format!(
            "selected provider plan `{plan_name}` {role} `{}` disagrees with its exact package/toolchain ownership",
            declaration.path,
        ))])
    }
}
