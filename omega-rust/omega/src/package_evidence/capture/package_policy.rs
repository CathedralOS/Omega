//! Direct assembly of normalized checked package policy components.
//!
//! This does not construct a legacy review capsule, persist proof results, or
//! admit the package. Every component retains its own checked association owner.

mod authority;
mod external;
mod restricted_build;
mod semantic_dependencies;

use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::record::PackagePolicyBaseline;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub fn project_checked_package_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
    let compilation = &input.into();
    // The checked program is read-only for the whole projection; its
    // program-pure tables are shared across every declaration's projection.
    let _frozen_program = typed_trees_to_checked_trees::validation::enter_frozen_program_scope(
        &compilation.program.typed,
    );
    if compilation.custody.package_identity() != Some(package)
        || compilation.custody.selected_target_profile() != Some(target)
        || compilation.custody.selected_native_target() != Some(target.native_target())
    {
        return Err(rejected(
            "package or target differs from the exact checked root activation",
        ));
    }
    let public_api = super::api::policy::project(compilation, package)?;
    let callables = super::project_checked_callable_policy(compilation, target, package)?;
    let selected_providers =
        super::project_checked_selected_provider_policy(compilation, target, package)?;
    let terminal_permissions =
        super::project_checked_terminal_permission_policy(compilation, target, package)?;
    let representation = super::project_checked_representation_policy(compilation, package)?;
    let external_supplies = external::project(compilation, package)?;
    let (dangerous_capabilities, slack_uses) = authority::project(compilation, &callables)?;
    let semantic_dependencies = semantic_dependencies::project(compilation, package, &callables)?;
    let boundary_applications = super::providers::project_checked_boundary_application_policy(
        compilation,
        target,
        package,
    )?;
    let policy = PackagePolicyBaseline {
        package,
        target,
        public_api,
        callables,
        selected_providers,
        terminal_permissions,
        representation,
        external_supplies,
        dangerous_capabilities,
        slack_uses,
        semantic_dependencies,
        boundary_applications,
        restricted_build_requests: restricted_build::project(compilation),
    };
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package policy rejects {reason}"
    ))]
}
