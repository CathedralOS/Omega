//! One application invocation's compiler and package resource lifetime.
//!
//! Package policy lends staging and aggregate sponsors to admitted work; it
//! neither owns a compiler nor chooses when to run it. The application retains
//! this session across source discovery, target reviews and automatic resume.

use compiler::CompilationSession;
use package_manager::PackageBuildSession;
use package_manager::review::CompileResolvedPackageReviewsError;
use std::path::Path;

/// Reusable compiler work and aggregate package grants for one invocation.
/// Closing staging does not replace either owner or authorize another Build.
pub struct BuildSession {
    pub(crate) compiler: CompilationSession,
    pub(crate) packages: PackageBuildSession,
}

impl BuildSession {
    /// Reserve one resource grant and one reusable compiler for the invocation.
    pub fn create(workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
        let packages = PackageBuildSession::create(workspace)?;
        Ok(Self {
            compiler: CompilationSession::default(),
            packages,
        })
    }

    /// Reuse the invocation's compiler without transferring it to package policy.
    pub fn compiler(&mut self) -> &mut CompilationSession {
        &mut self.compiler
    }

    /// Borrow package-owned admission, staging and budget accounting.
    pub fn resources(&mut self) -> &mut PackageBuildSession {
        &mut self.packages
    }

    /// Close staging without replenishing the invocation's resource grant.
    pub fn close(&mut self) -> Result<(), CompileResolvedPackageReviewsError> {
        self.packages.close()
    }

    /// Preserve the operation's error if resource disposal also fails.
    pub fn dispose<T, Error>(self, result: Result<T, Error>) -> Result<T, Error>
    where
        Error: std::fmt::Display + From<CompileResolvedPackageReviewsError>,
    {
        self.packages.dispose(result)
    }
}
