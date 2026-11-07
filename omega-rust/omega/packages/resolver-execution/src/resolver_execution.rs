// Freezes the resolver executable path outside package-controlled roots, then
// prepares each `ResolverExecutionPhase` at one exact absolute working root
// with fixed `BoundedProcessLimits` (120s CPU, 8 GiB address space, 1 GiB file
// size, 256 open files, 16 active processes, 2 GiB per-process memory, 4 GiB
// aggregate memory).
//
// Both the authored executable path and its canonicalized target are checked
// against `package_controlled_roots` so a symlink in a host directory cannot
// redirect execution into a package-controlled tree.

mod path_custody;

use crate::ResolverPreparedExecution;
use bounded_process::BoundedProcessLimits;
use path_custody::{
    require_absolute, require_lexically_canonical_bounded_path, require_outside_roots,
    require_regular_file,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    TransportDiscovery, // Remote object-format and selector discovery through host Git.
    RepositoryInitialization, // Creation of a new local bare repository.
    Fetch,              // Fetch into an existing quarantine through host Git.
    RepositoryInspection, // Read-only object and tree inspection.
}

#[derive(Debug)]
pub struct ResolverExecutionBackend {
    executable: PathBuf,
}

impl ResolverExecutionBackend {
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

    /// ```compile_fail
    /// use resolver_execution::{ResolverExecutionBackend, ResolverExecutionPhase};
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
