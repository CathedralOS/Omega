//! Owners retained only while deriving the policy of a checked occurrence.
use diagnostics::Diagnostic;
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::TypedTrees;

pub(super) struct PolicySource<'a> {
    pub(super) carried_types: super::carried_types::CarriedTypes,
    pub(super) declarations: &'a source_compiler::CheckedPackageDeclarations,
    pub(super) accepted: &'a AcceptedTerminalSource,
    pub(super) sources: &'a package_compilation::CheckedSourceCustody,
    pub(super) restricted_build_requests: Vec<package_compilation::RestrictedBuildRequest>,
    pub(super) evaluation: &'a build_execution::EvaluatedBuild,
}

impl<'a> PolicySource<'a> {
    pub(super) fn checked(
        declarations: &'a source_compiler::CheckedPackageDeclarations,
        sources: &'a package_compilation::CheckedSourceCustody,
        product: &'a crate::PlannedProduct,
        inputs: &package_compilation::PackageCompilationInputs,
        target: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let accepted = declarations.accepted()?;
        sources
            .require_owner(accepted.source())
            .map_err(|diagnostic| vec![diagnostic])?;
        if accepted.is_invocation() || sources.package_identity().is_none() {
            return Err(vec![Diagnostic::error(
                "package review requires a complete checked package",
            )]);
        }
        Ok(Self {
            carried_types: super::carried_types::CarriedTypes::new(inputs, target)?,
            declarations,
            accepted,
            sources,
            restricted_build_requests: product.reviewed_build_requests(target)?,
            evaluation: product.evaluated(),
        })
    }
    pub(super) fn typed(&self) -> &'a TypedTrees {
        self.accepted.source()
    }
    pub(super) fn sources(&self) -> &'a package_compilation::CheckedSourceCustody {
        self.sources
    }
    pub(super) fn accepted(&self) -> &'a AcceptedTerminalSource {
        self.accepted
    }
    pub(super) fn build(&self) -> &build_evaluation_result::ComputedBuildConfig {
        &self.evaluation.configuration
    }

    pub(super) fn selected_build_symbol(
        &self,
    ) -> Result<Option<symbols::SymbolHandle>, Vec<Diagnostic>> {
        self.build()
            .executed_entry
            .as_ref()
            .map(|entry| entry.rejoin(self.typed()))
            .transpose()
    }
}
