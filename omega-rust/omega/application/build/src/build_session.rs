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
    /// Reserve one resource grant and one reusable in-memory compiler.
    /// Embedders own this lifetime and need no process-wide storage policy.
    pub fn create(workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
        let packages = phase_profile::scope("setup.package_grant", || {
            PackageBuildSession::create(workspace)
        })?;
        Ok(Self {
            compiler: CompilationSession::default(),
            packages,
        })
    }

    /// Application invocations retain their compiler inputs across processes.
    /// Select storage before source preparation, while the session is empty.
    pub fn create_retained(workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
        let mut session = Self::create(workspace)?;
        session.compiler = compiler_session();
        Ok(session)
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

/// Both standalone requests and package workflows configure the same optional
/// compiler-private store before source preparation. Cache setup never admits
/// source or prevents a cold compilation.
pub(crate) fn compiler_session() -> CompilationSession {
    match phase_profile::scope("setup.compiler_storage", || {
        package_manager::retain_compiler_source_storage(None)
    }) {
        Ok(storage) => phase_profile::scope("setup.compiler_graph", || {
            CompilationSession::default().with_source_retention(storage)
        }),
        Err(reason) => {
            if std::env::var_os("OMEGA_PHASE_PROFILE").is_some_and(|value| value != "0") {
                eprintln!("compiler source retention unavailable: {reason}");
            }
            CompilationSession::default()
        }
    }
}
