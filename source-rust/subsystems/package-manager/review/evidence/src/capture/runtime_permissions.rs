//! A requested lock captures the activated package's own source policy before
//! publication. Consumption joins its declarations with the actual selected
//! native receipts and retains any complete-inventory refusal.

use diagnostics::Diagnostic;
use products::CheckedProductSource;
use source_compiler::provider_supply::{SourceProviderDemand, SourceProviderFacts};
use source_compiler::source_state::ProductSourceIdentity;
use std::sync::{Arc, Weak};

pub struct PreparedRuntimePolicy {
    owner: Result<ProductSourceIdentity, Diagnostic>,
    sources: Arc<package_compilation::CheckedSourceCustody>,
    publication: Weak<source_compiler::SourcePublication>,
    build: Weak<products::ComputedBuildConfig>,
    admission: products::BuildAdmission,
    target: Option<target::TargetProfile>,
    prepared: Result<
        (
            terminal_psi::TerminalPsiIdentity,
            Box<super::policy::PreparedPolicy>,
        ),
        Vec<Diagnostic>,
    >,
}

impl PreparedRuntimePolicy {
    pub fn capture(
        source: &CheckedProductSource,
        target: Option<target::TargetProfile>,
        demand: &SourceProviderFacts<SourceProviderDemand>,
    ) -> Self {
        let owner = source
            .sources
            .require_owner(&source.program)
            .map(|()| ProductSourceIdentity::new(&source.program));
        let prepared = (|| {
            let view = super::input::PolicySource::product(source)?;
            let package = view.sources().package_identity().ok_or_else(|| {
                super::declarations::unavailable(
                    "the accepted package's complete declaration subject",
                )
            })?;
            let chosen = demand.logical_provider_reach(
                &source.terminal_publication,
                target.map(|profile| profile.target_name()),
            )?;
            let (Some(target), Some(chosen)) = (target, chosen) else {
                return Err(vec![Diagnostic::error(
                    "policy package or target differs from the exact root activation",
                )]);
            };
            let choices = super::provider_rows::from_source(chosen, view.accepted().module());
            let prepared = super::policy::prepare(&view, target, package, choices)?;
            Ok((chosen.selection().program(), Box::new(prepared)))
        })();
        Self {
            owner,
            sources: Arc::clone(&source.sources),
            publication: Arc::downgrade(&source.terminal_publication),
            build: Arc::downgrade(&source.computed_build_config),
            admission: source.admission.clone(),
            target,
            prepared,
        }
    }

    pub(crate) fn rows(
        &self,
        input: &crate::permission_acceptance::RuntimePermissionInput<'_>,
        target: target::TargetProfile,
        package: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<Vec<crate::record::PackagePolicyRow>, Vec<Diagnostic>> {
        let owner = self.owner.as_ref().map_err(|error| vec![error.clone()])?;
        let checked = input.compilation;
        if !owner.same_source(checked.sources().source_identity())
            || self.sources.as_ref() != checked.sources()
            || !std::ptr::eq(self.publication.as_ptr(), checked.terminal_publication())
        {
            return Err(vec![Diagnostic::error(
                "saved permission declarations belong to another typed source owner",
            )]);
        }
        if !std::ptr::eq(self.build.as_ptr(), checked.build())
            || &self.admission != checked.build_admission()
        {
            return Err(super::declarations::unavailable(
                "the selected Build's exact permission policy custody",
            ));
        }
        if self.target != Some(target)
            || checked.sources().package_identity() != Some(package)
            || checked.execution().target_profile() != Some(target)
            || checked.execution().native_target() != Some(target.native_target())
        {
            return Err(vec![Diagnostic::error(
                "policy package or target differs from the exact root activation",
            )]);
        }
        let (original, prepared) = self.prepared.as_ref().map_err(Clone::clone)?;
        let published = checked.published_program().ok_or_else(|| {
            super::declarations::unavailable("the activated package's published program")
        })?;
        if *original != checked.execution().original_program()
            || *original != published.artifact().view().identity()
        {
            return Err(super::declarations::unavailable(
                "the activated package's original program",
            ));
        }
        let policy = prepared.finish(checked, || super::provider_rows::from_review(checked))?;
        let (rows, _) = policy
            .acceptance_rows_with_limits(Default::default())
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot encode the accepted runtime permission policy: {error:?}"
                ))]
            })?;
        Ok(rows)
    }
}
