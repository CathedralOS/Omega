//! Check the complete inventory of each dependency the Build explicitly selected.
use super::services::HandoffSources;
use build_execution::CompileFailure;
use diagnostics::Diagnostic;
use project_build::SettledDependency;
use target::TargetProfile;

impl HandoffSources<'_> {
    pub(super) fn validate_dependencies(
        &self,
        settled: &SettledDependency,
        target: TargetProfile,
    ) -> Result<(), CompileFailure> {
        for subject in &settled.evaluated().dependency_validation {
            let supplier = self
                .settled
                .iter()
                .find(|candidate| {
                    std::sync::Arc::ptr_eq(
                        &candidate.build.evaluated().configuration,
                        &subject.configuration,
                    ) && candidate.build.inputs() == &subject.inputs
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "validation lost the original Build of the selected package",
                    )]
                })?;
            let deployment = if subject.purpose.is_product() {
                target
            } else {
                self.profile
            };
            let mut dependencies = self.deployment_products(&supplier.build, deployment);
            let (validated, rechecks) = self.dependency_product(
                &supplier.build,
                subject.purpose,
                deployment,
                &mut dependencies,
                project_build::DependencyCheck::FullPackage,
                false,
            )?;
            rechecks.settle()?;
            let validated = validated.with_component_suppliers(dependencies.into_suppliers());
            let mut timings = artifacts::compile_timings::CompileTimings::default();
            let result = validated
                .product_stage(&[deployment])?
                .check(deployment, &mut timings);
            if let Err(mut diagnostics) = result {
                validated.attribute_diagnostics(&mut diagnostics);
                return Err(diagnostics.into());
            }
        }
        Ok(())
    }
}
