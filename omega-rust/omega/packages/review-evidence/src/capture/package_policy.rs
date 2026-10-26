//! Direct assembly of normalized checked package policy components.
//!
//! This does not construct a legacy review capsule, persist proof results, or
//! admit the package. Every component retains its own checked association owner.

mod assumed_bindings;
mod authority;
mod external;
mod restricted_build;
mod semantic_dependencies;

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyBaseline;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// One projection borrows both immutable namespaces for its whole lifetime.
/// Family heads rederive original declarations repeatedly; their normalization
/// belongs to custody, independently of the supplied candidate's query state.
struct PolicyCheckingContexts<'program> {
    candidate: validation::CheckingContext<'program>,
    custody: validation::CheckingContext<'program>,
}

impl<'program> PolicyCheckingContexts<'program> {
    fn new(
        candidate: &'program checked_trees::CheckedTrees,
        custody: &'program checked_trees::CheckedTrees,
    ) -> Self {
        Self {
            candidate: validation::CheckingContext::new(&candidate.typed)
                .with_authored_declaration_selections(candidate.authored_declaration_selections()),
            custody: validation::CheckingContext::new(&custody.typed)
                .with_authored_declaration_selections(custody.authored_declaration_selections()),
        }
    }
}

pub fn project_checked_package_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
    let input = input.into();
    let queries = PolicyCheckingContexts::new(input.program, input.custody);
    let compilation = &PackageReviewInput {
        checking_input: &queries.candidate,
        custody_checking_input: &queries.custody,
        ..input
    };
    if compilation.custody.sources().package_identity() != Some(package)
        || compilation.custody.execution().selected_target_profile != Some(target)
        || compilation.custody.execution().selected_native_target() != Some(target.native_target())
    {
        return Err(rejected(
            "package or target differs from the exact checked root activation",
        ));
    }
    let public_api = super::api::policy::project(compilation, package)?;
    let callables = super::project_checked_callable_policy(compilation, target, package)?;
    let (selected_providers, provider_indices) =
        super::project_checked_selected_provider_policy(compilation, target, package)?;
    let terminal_permissions =
        super::project_checked_terminal_permission_policy(compilation, target, package)?;
    let representation = super::project_checked_representation_policy(compilation, package)?;
    let external_supplies = external::project(compilation, package)?;
    let (dangerous_capabilities, slack_uses) = authority::project(compilation, &callables)?;
    let semantic_dependencies = semantic_dependencies::project(compilation, package, &callables)?;
    let boundary_applications = super::providers::project_checked_boundary_application_policy(
        compilation,
        package,
        &provider_indices,
    )?;
    let mut policy = PackagePolicyBaseline {
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
        assumed_bindings: assumed_bindings::project(compilation)?,
        service_audits: authority::registrations(compilation)?,
    };
    let services = policy.service_audits().to_vec();
    policy.apply_service_audit_policy(&services);
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package policy rejects {reason}"
    ))]
}
