use super::request::validate_launch_request;
use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::process::limits;
use crate::request::{
    require_absolute, require_lexically_canonical_bounded_path, require_outside_roots,
    require_regular_file,
};
use crate::{ResolverExecutionPhase, ResolverPreparedExecution};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    /// Prepare a mutating phase rooted at its exact package-controlled
    /// repository directory.
    pub fn prepare(
        &self,
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_roots(
            phase,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: None,
                mutable_root,
            },
        )
    }

    pub fn prepare_inspection(
        &self,
        inspection_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_roots(
            ResolverExecutionPhase::RepositoryInspection,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: Some(inspection_read_root),
                mutable_root: None,
            },
        )
    }

    pub fn prepare_discovery(
        &self,
        discovery_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_roots(
            ResolverExecutionPhase::TransportDiscovery,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: Some(discovery_read_root),
                inspection_read_root: None,
                mutable_root: None,
            },
        )
    }

    pub(super) fn prepare_with_roots(
        &self,
        phase: ResolverExecutionPhase,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<ResolverPreparedExecution> {
        validate_launch_request(&self.executable, phase, roots)?;
        let working_root = roots
            .mutable_root
            .or(roots.inspection_read_root)
            .or(roots.discovery_read_root)
            .expect("every resolver phase has exactly one working root");
        let mut command = Command::new(&self.executable);
        command.current_dir(working_root);
        limits::configure_child_resource_limits(&mut command)?;
        Ok(ResolverPreparedExecution::new(command))
    }
}
