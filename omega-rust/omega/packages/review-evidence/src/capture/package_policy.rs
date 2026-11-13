/*
Direct assembly of normalized checked package policy (`project_checked_package_policy`).

`PolicyCheckingContexts` borrows separate immutable `CheckingContext` instances
for the supplied candidate and compiler custody so family heads rederiving
original declarations never read the candidate's memoized query state. The
assembled `PackagePolicyBaseline` joins public API, callables, selected
providers, terminal permissions, representation, external supplies, native
bindings, dangerous capabilities, semantic dependencies, boundary applications,
restricted build requests, assumed bindings, and service audits.
*/

pub(crate) mod assumed_bindings;
mod authority;
mod external;
mod native_bindings;
mod restricted_build;
mod semantic_dependencies;

use super::providers::project_checked_boundary_application_policy;
use crate::capture::PackageReviewInput;
use crate::capture::refusal;
use crate::record::PackagePolicyBaseline;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

struct PolicyCheckingContexts<'program> {
    candidate: validation::CheckingContext<'program>,
    custody: validation::CheckingContext<'program>,
}

impl<'program> PolicyCheckingContexts<'program> {
    fn new(candidate: &'program CheckedTrees, custody: &'program CheckedTrees) -> Self {
        Self {
            candidate: validation::CheckingContext::new(&candidate.typed)
                .with_authored_declaration_selections(candidate.authored_declaration_selections()),
            custody: validation::CheckingContext::new(&custody.typed)
                .with_authored_declaration_selections(custody.authored_declaration_selections()),
        }
    }
}

pub fn project_checked_package_policy<'a>(
    input: impl crate::IntoPackageReviewInput<'a>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
    let input = input.into_review_input()?;
    let queries = PolicyCheckingContexts::new(input.program, input.original_program);
    let mathematical_calls =
        super::contracts::mathematical_calls::MathematicalCallQueries::new(&queries.candidate);
    let original_mathematical_calls =
        super::contracts::mathematical_calls::MathematicalCallQueries::new(&queries.custody);
    let external_providers = input.project_external_providers()?;
    let services = input.project_service_declarations()?;
    let compilation = &PackageReviewInput {
        service_declarations: Some(&services),
        external_providers: Some(&external_providers),
        source: super::SourceDeclarationInput {
            checking_input: &queries.candidate,
            original_checking_input: &queries.custody,
            mathematical_calls: Some(&mathematical_calls),
            original_mathematical_calls: Some(&original_mathematical_calls),
            ..input.source
        },
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
    let boundary_applications =
        project_checked_boundary_application_policy(compilation, package, &provider_indices)?;
    let mut policy = PackagePolicyBaseline {
        package,
        target,
        public_api,
        callables,
        selected_providers,
        terminal_permissions,
        representation,
        external_supplies,
        native_bindings: native_bindings::project(compilation, package)?,
        dangerous_capabilities,
        slack_uses,
        semantic_dependencies,
        boundary_applications,
        restricted_build_requests: restricted_build::project(compilation),
        assumed_bindings: assumed_bindings::project(compilation)?,
        service_audits: authority::registrations(compilation)?,
        component_assumptions: crate::record::PackagePolicyComponentAssumption::project(
            compilation.custody.providers(),
        ),
    };
    let services = policy.service_audits().to_vec();
    policy.apply_service_audit_policy(&services);
    Ok(policy)
}

refusal!(reason, "package policy rejects {reason}");
