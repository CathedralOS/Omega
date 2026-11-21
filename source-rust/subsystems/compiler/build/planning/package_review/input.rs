//! Owners retained only while deriving the policy of a checked occurrence.
use diagnostics::Diagnostic;
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::{
    TypedTrees, source_subjects::rejoin_build_activation,
};

pub(super) struct PolicySource<'a> {
    pub(super) declarations: &'a source_compiler::CheckedPackageDeclarations,
    pub(super) accepted: &'a AcceptedTerminalSource,
    pub(super) sources: &'a package_compilation::CheckedSourceCustody,
    pub(super) evaluation: &'a build_execution::EvaluatedBuild,
}

impl<'a> PolicySource<'a> {
    pub(super) fn checked(
        declarations: &'a source_compiler::CheckedPackageDeclarations,
        sources: &'a package_compilation::CheckedSourceCustody,
        evaluation: &'a build_execution::EvaluatedBuild,
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
            declarations,
            accepted,
            sources,
            evaluation,
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
            .selected_build_declaration
            .as_ref()
            .map(|declaration| {
                rejoin_build_activation(
                    self.typed(),
                    declaration.source().package_identity,
                    declaration,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])
            })
            .transpose()
    }
}
