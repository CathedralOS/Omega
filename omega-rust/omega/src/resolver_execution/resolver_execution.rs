//! Freeze the resolver executable, then prepare each phase at one exact working root.
//!
//! Phase selects the root's role, not an operating-system confinement policy.
//! Git command policy belongs to acquisition; shared process limits and cleanup
//! belong to bounded-process.

mod path_custody;

use crate::bounded_process::BoundedProcessLimits;
use crate::resolver_execution::ResolverPreparedExecution;
use path_custody::{
    require_absolute, require_lexically_canonical_bounded_path, require_outside_roots,
    require_regular_file,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One compiler-owned source-resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    /// Remote object-format and selector discovery through ordinary host Git.
    TransportDiscovery,
    /// Creation of a new local bare repository.
    RepositoryInitialization,
    /// Fetch into an existing quarantine through ordinary host Git.
    Fetch,
    /// Read-only object and tree inspection.
    RepositoryInspection,
}

/// The primary executable frozen before package input influences command setup.
#[derive(Debug)]
pub struct ResolverExecutionBackend {
    executable: PathBuf,
}

impl ResolverExecutionBackend {
    /// Freeze one exact executable path before package-controlled roots can
    /// influence launch construction.
    pub fn open(executable: &Path, package_controlled_roots: &[PathBuf]) -> io::Result<Self> {
        require_absolute(executable, "resolver executable")?;
        require_lexically_canonical_bounded_path(executable, "resolver executable")?;
        require_outside_roots(executable, package_controlled_roots, "resolver executable")?;
        let canonical = executable.canonicalize().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot resolve resolver executable: {error}"),
            )
        })?;
        require_regular_file(&canonical, "resolver executable")?;
        require_lexically_canonical_bounded_path(&canonical, "resolver executable")?;
        require_outside_roots(&canonical, package_controlled_roots, "resolver executable")?;
        Ok(Self {
            executable: canonical,
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Prepare one phase at its required working root using the frozen executable.
    /// Discovery and inspection read that root; initialization and fetch mutate it
    /// through the acquisition owner's command policy, not a filesystem sandbox.
    /// Missing or competing roots are not representable in this interface.
    ///
    /// ```compile_fail
    /// use crate::resolver_execution::{ResolverExecutionBackend, ResolverExecutionPhase};
    /// fn missing_root(backend: &ResolverExecutionBackend) {
    ///     backend.prepare(ResolverExecutionPhase::Fetch, None);
    /// }
    /// ```
    pub fn prepare(
        &self,
        phase: ResolverExecutionPhase,
        working_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        let root_name = match phase {
            ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
                "resolver mutable root"
            }
            ResolverExecutionPhase::RepositoryInspection => "resolver inspection read root",
            ResolverExecutionPhase::TransportDiscovery => "resolver discovery read root",
        };
        require_absolute(working_root, root_name)?;
        require_lexically_canonical_bounded_path(working_root, root_name)?;
        require_outside_roots(&self.executable, &[working_root], "resolver executable")?;
        let mut command = Command::new(&self.executable);
        command.current_dir(working_root);
        ResolverPreparedExecution::new(command, resolver_process_limits(), "resolver")
    }
}

fn resolver_process_limits() -> BoundedProcessLimits {
    BoundedProcessLimits::new(
        120,
        8 * 1024 * 1024 * 1024,
        1024 * 1024 * 1024,
        256,
        16,
        2 * 1024 * 1024 * 1024,
        4 * 1024 * 1024 * 1024,
    )
}

#[cfg(test)]
mod tests;
