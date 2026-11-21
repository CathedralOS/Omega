//! Collect the package findings established by one complete Product judgment and its selected target.
mod authority;
mod callables;
mod calling;
mod declarations;
mod hosted_services;
mod input;
mod native_bindings;
mod numeric;
mod policy;
mod provider_rows;
mod providers;
mod semantic_bindings;
mod telescope;
mod terminal_permissions;
mod traits;

use diagnostics::Diagnostic;
use package_compilation::{
    BuildSourceCustody, CheckedSourceCustody, IndependentComponentDescription,
};
use review_evidence::record::{PackagePolicyBaseline, SemanticBindingReviewCandidate};
use target::TargetProfile;

#[non_exhaustive]
pub struct CheckedPackagePolicy {
    pub sources: CheckedSourceCustody,
    pub source_completion_accounting:
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    pub selected_build_machine_identity: String,
    pub policy: PackagePolicyBaseline,
    pub semantic_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub target_entry_candidate: Option<SemanticBindingReviewCandidate>,
    pub component_description: Option<IndependentComponentDescription>,
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
        let (declarations, selected) = compiler::review_package(
            self.source(),
            self.native_target(profile, Vec::new())?,
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
        let input = input::PolicySource::checked(&declarations, &sources, evaluation)?;
        let package = inputs.root();
        let policy = policy::project(&input, &selected, profile, package)?;
        let (semantic_binding_candidates, target_entry_candidate) =
            semantic_bindings::candidates(&input, &selected, &policy, profile, package)?;
        if selected.realization().composition_modes().any(|mode| {
            mode == semantic_vocabulary::source_selections::CompositionMode::Independent
        }) || declarations.explicit_providers().iter().any(|supply| {
            supply.composition_mode
                == semantic_vocabulary::source_selections::CompositionMode::Independent
                && supply
                    .target
                    .is_none_or(|target| target.canonical_name() == profile.target_name())
        }) {
            return Err(vec![Diagnostic::error(
                "package review requires the separately verified description of its independently composed provider",
            )]);
        }
        let selected_build_machine_identity = evaluation
            .configuration
            .selected_machine_identity(input.typed())?
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "package review lacks the exact selected Build machine",
                )]
            })?;
        Ok(CheckedPackagePolicy {
            sources,
            policy,
            source_completion_accounting: std::mem::take(accounting),
            selected_build_machine_identity,
            semantic_binding_candidates,
            target_entry_candidate,
            component_description: None,
        })
    }
}
