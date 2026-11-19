//! A Build declaration issued for the same captured source and returned activation.
use super::{CompileFailure, PreparedBuildEvaluation};
use build_declarations::BuildDeclaration;
use diagnostics::Diagnostic;
use package_compilation::CapturedLocalSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBuildDeclaration {
    capture: CapturedLocalSource,
    declaration: BuildDeclaration,
    settled: bool,
}

impl SourceBuildDeclaration {
    pub fn source_capture(&self) -> &CapturedLocalSource {
        &self.capture
    }
    pub fn declaration(&self) -> &BuildDeclaration {
        &self.declaration
    }
    pub fn is_settled(&self) -> bool {
        self.settled
    }
}

impl PreparedBuildEvaluation {
    fn require_catalog_acquisition(
        &self,
        returned: &crate::ReturnedBuild,
    ) -> Result<CapturedLocalSource, CompileFailure> {
        self.validate_return(returned)?;
        returned.original_acquisition().cloned().ok_or_else(|| {
            CompileFailure::source_completion(
                vec![Diagnostic::error(
                    "workspace catalog requires the Build's original unbound source acquisition",
                )],
                self.source_completion_accounting.clone(),
            )
        })
    }

    /// Observe the named declaration of the returned activation before source resolution.
    pub fn observe_source_declaration(
        &self,
        returned: &crate::ReturnedBuild,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        let capture = self.require_catalog_acquisition(returned)?;
        let declaration = returned.declaration().cloned().ok_or_else(|| {
            CompileFailure::source_completion(
                vec![Diagnostic::error(
                    "source Build returned no named declaration",
                )],
                self.source_completion_accounting.clone(),
            )
        })?;
        Ok(SourceBuildDeclaration {
            capture,
            declaration,
            settled: false,
        })
    }

    /// Finish the activation and its output obligations before issuing a workspace member roster.
    pub fn settle_workspace_declaration(
        self,
        returned: crate::ReturnedBuild,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        let capture = self.require_catalog_acquisition(&returned)?;
        self.validate_source_join(Some(&returned), None, true)?;
        let evaluated = self.complete(Some(returned))?;
        let Some(declaration @ BuildDeclaration::Workspace(_)) =
            evaluated.configuration.project_declaration.as_ref()
        else {
            return Err(CompileFailure::source_completion(
                vec![Diagnostic::error(
                    "source catalog requires a completed workspace declaration",
                )],
                evaluated.source_completion_accounting,
            ));
        };
        Ok(SourceBuildDeclaration {
            capture,
            declaration: declaration.clone(),
            settled: true,
        })
    }
}
