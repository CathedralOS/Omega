/*

Invocation-scoped compiler and package resource lifetime (`BuildSession`).

A `BuildSession` pairs one in-memory `CompilationSession` with one
`PackageBuildSession` (staging directory custody and aggregate evaluation work/byte
budgets). Package policy lends staging and resource sponsors to admitted work,
but never owns a `CompilationSession` or decides when to invoke the compiler.

We retain the same `BuildSession` across source discovery, multi-target review
passes, and automatic `--dangerously-accept-all` resume rounds. Calling `close`
or `dispose` cleans up the staging directory without replenishing the
invocation's resource grant; if both the operation and staging cleanup fail,
`dispose` preserves the primary operation error.

*/

use compiler::CompilationSession;
use package_manager::PackageBuildSession;
use package_manager::review::CompileResolvedPackageReviewsError;
use std::path::Path;

pub struct BuildSession {
    pub(crate) compiler: CompilationSession,
    pub(crate) packages: PackageBuildSession,
}

impl BuildSession {
    pub fn create(workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
        let packages = PackageBuildSession::create(workspace)?;
        Ok(Self {
            compiler: compiler_session(),
            packages,
        })
    }

    pub fn create_retained(workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
        Self::create(workspace)
    }

    pub fn compiler(&mut self) -> &mut CompilationSession {
        &mut self.compiler
    }

    pub fn resources(&mut self) -> &mut PackageBuildSession {
        &mut self.packages
    }

    pub fn close(&mut self) -> Result<(), CompileResolvedPackageReviewsError> {
        self.packages.close()
    }

    /// These concrete boundaries join held work even when a later operation's
    /// primary error has no compiler receipts of its own.
    pub fn dispose_inspection<T>(
        self,
        result: Result<T, package_manager::operations::PackageInspectionError>,
    ) -> Result<T, package_manager::operations::PackageInspectionError> {
        let accounting = self.packages.source_completion_accounting().clone();
        self.dispose(result.map_err(|error| error.with_source_completion_accounting(accounting)))
    }

    pub fn dispose_command<T>(
        self,
        result: Result<T, package_manager::PackageCommandError>,
    ) -> Result<T, package_manager::PackageCommandError> {
        let accounting = self.packages.source_completion_accounting().clone();
        self.dispose(result.map_err(|error| error.with_source_completion_accounting(accounting)))
    }

    pub fn dispose<T, Error>(self, result: Result<T, Error>) -> Result<T, Error>
    where
        Error: std::fmt::Display + From<CompileResolvedPackageReviewsError>,
    {
        self.packages.dispose(result)
    }
}

pub(crate) fn compiler_session() -> CompilationSession {
    CompilationSession::default()
}
