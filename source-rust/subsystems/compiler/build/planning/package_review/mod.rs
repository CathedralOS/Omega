//! Collect the package findings established by one complete Product judgment and its selected target.
mod authority;
mod callables;
mod calling;
mod carried_types;
mod declarations;
mod hosted_services;
mod input;
mod native_bindings;
mod numeric;
mod policy;
mod preparations;
mod provider_grants;
mod provider_rows;
mod providers;
mod semantic_bindings;
mod services;
mod telescope;
mod terminal_permissions;
mod traits;

pub use preparations::ReviewedPreparation;

use diagnostics::Diagnostic;
use package_compilation::{
    BuildSourceCustody, CheckedSourceCustody, IndependentComponentDescription,
    RestrictedBuildRequest,
};
use review_evidence::record::{PackagePolicyBaseline, SemanticBindingReviewCandidate};
use target::TargetProfile;

#[non_exhaustive]
pub struct CheckedPackagePolicy {
    pub sources: CheckedSourceCustody,
    pub source_completion_accounting:
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    pub selected_build_machine_identity: String,
    pub preparations: Vec<ReviewedPreparation>,
    pub restricted_build_requests: Vec<RestrictedBuildRequest>,
    pub policy: PackagePolicyBaseline,
    pub semantic_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub component_description: Option<IndependentComponentDescription>,
}

impl crate::PlannedProduct {
    /// A single host Build can serve multiple deployments. Retain the request
    /// under the deployment being reviewed while preserving the host that ran it.
    pub fn reviewed_build_requests(
        &self,
        target: TargetProfile,
    ) -> Result<Vec<RestrictedBuildRequest>, Vec<Diagnostic>> {
        self.evaluated()
            .restricted_build_requests
            .iter()
            .map(|request| {
                if request
                    .selected_target_profile()
                    .is_some_and(|selected| selected != target)
                {
                    return Err(declarations::unavailable(
                        "a restricted Build request for a different deployment",
                    ));
                }
                Ok(request.clone().for_target(Some(target)))
            })
            .collect()
    }
}

impl crate::ProductStage<'_> {
    pub fn review_policy(
        &self,
        profile: TargetProfile,
    ) -> Result<CheckedPackagePolicy, build_evaluation_result::CompileFailure> {
        let mut accounting = Default::default();
        self.collect_policy(profile, &mut accounting)
            .map_err(|diagnostics| {
                build_evaluation_result::CompileFailure::source_completion(diagnostics, accounting)
            })
    }

    fn collect_policy(
        &self,
        profile: TargetProfile,
        accounting: &mut source_compiler::compile_time_invocations::SourceCompletionAccounting,
    ) -> Result<CheckedPackagePolicy, Vec<Diagnostic>> {
        let product = self.product;
        let inputs = product.package_inputs().ok_or_else(|| {
            vec![Diagnostic::error(
                "package review requires the acquired package graph",
            )]
        })?;
        let compiler::ReviewedPackage {
            declarations,
            providers: selected,
            component,
            component_assumptions,
        } = compiler::review_package(
            self.source(),
            self.native_target(profile, Vec::new())?,
            inputs,
            accounting,
        )?;
        let accepted = declarations.accepted()?;
        let source = product.source();
        let evaluation = product.evaluated();
        let sources = BuildSourceCustody {
            source_file_count: source.parsed.files.selected_files().count(),
            build_execution_profile: Some(product.build_profile()),
            generated_source_custody: source.generated_source_custody.clone(),
            base_source_consumption_commitment: evaluation.base_source_consumption_commitment,
        }
        .seal(accepted.source(), Some(inputs))?;
        let input =
            input::PolicySource::checked(&declarations, &sources, product, inputs, profile)?;
        let package = inputs.root();
        let services = services::identities(
            input.typed(),
            input.accepted().module(),
            input.sources().exact_toolchain_sources(),
            source_provenance::DependencyScope::Product,
        )?;
        let policy = policy::project(
            &services,
            &input,
            &selected,
            profile,
            package,
            component_assumptions,
        )?;
        let semantic_binding_candidates =
            semantic_bindings::candidates(&services, &input, &selected, &policy, profile)?;
        let selected_build_machine_identity = evaluation
            .configuration
            .selected_machine_identity(input.typed())?
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "package review lacks the exact selected Build machine",
                )]
            })?;
        Ok(CheckedPackagePolicy {
            preparations: product.reviewed_preparations()?,
            restricted_build_requests: input.restricted_build_requests,
            sources,
            policy,
            source_completion_accounting: std::mem::take(accounting),
            selected_build_machine_identity,
            semantic_binding_candidates,
            component_description: component,
        })
    }
}
