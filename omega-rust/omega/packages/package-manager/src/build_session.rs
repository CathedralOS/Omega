//! Private staging and aggregate Build resource grants shared by compile and review.

mod component_publication;
pub(crate) use component_publication::publish_checked_component;
mod inputs;
mod order;
mod preparation;
pub(crate) use inputs::{OccurrenceBuildInputs, reconcile_build_inputs};
pub(crate) use order::{dependency_first_package_order, package_build_root};
pub use preparation::PackageSourcePreparation;
pub(crate) use preparation::PreparedPackageSource;

use crate::review::CompileResolvedPackageReviewsError;
use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};
use filesystem_authority::FilesystemSponsor;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static PACKAGE_BUILD_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Deterministic evaluator work granted to one complete package
/// closure. The sibling limits below bound retained compiler resources; none
/// claims to bound CPU time or process memory.
const PACKAGE_BUILD_FUEL_CEILING: u64 = 100_000_000;
/// Aggregate compiler-owned BuildLog bytes across initial evaluation and
/// replay for the complete package closure.
const PACKAGE_BUILD_LOG_CEILING: u64 = 16 * 1024 * 1024;
/// Aggregate canonical filesystem operation attempts across initial evaluation
/// and replay for the complete package closure.
const PACKAGE_FILESYSTEM_ATTEMPT_CEILING: u64 = 65_536;
/// Compiler-owned filesystem resources live concurrently across the package
/// closure. This does not count unrelated host-process descriptors.
const PACKAGE_LIVE_FILESYSTEM_HANDLE_CEILING: u64 = 4_096;
/// Interpreter semantic storage-cell allocations live concurrently across the
/// package closure. This is a cell count, not a memory-byte claim.
const PACKAGE_LIVE_CELL_CEILING: u64 = 1_048_576;
/// Logical bytes held by concurrently live interpreter Text backing buffers.
/// This is not Vec capacity, allocator overhead, or a process-memory ceiling.
const PACKAGE_LIVE_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;
/// Aggregate recursive value cells returned by successful initial and replay
/// evaluations across the closure.
const PACKAGE_RESULT_CELL_CEILING: u64 = 1_048_576;
/// Aggregate Text payload bytes retained by successful build results.
const PACKAGE_RESULT_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct PackageBuildSession {
    root: PathBuf,
    filesystem_sponsor: FilesystemSponsor,
    evaluation_sponsor: BuildEvaluationSponsor,
    active: bool,
}

impl PackageBuildSession {
    pub(crate) fn create(
        build_workspace: &Path,
    ) -> Result<Self, CompileResolvedPackageReviewsError> {
        fs::create_dir_all(build_workspace).map_err(|error| {
            CompileResolvedPackageReviewsError::BuildStagingCreate {
                path: build_workspace.to_path_buf(),
                error,
            }
        })?;
        let canonical_workspace = fs::canonicalize(build_workspace).map_err(|error| {
            CompileResolvedPackageReviewsError::BuildStagingCreate {
                path: build_workspace.to_path_buf(),
                error,
            }
        })?;

        for _ in 0..128 {
            let sequence = PACKAGE_BUILD_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = canonical_workspace.join(format!(
                ".omega-package-build-{}-{sequence}",
                std::process::id()
            ));
            match FilesystemSponsor::create_private(&root) {
                Ok(filesystem_sponsor) => {
                    let canonical_root = filesystem_sponsor.session_root().map_err(|error| {
                        CompileResolvedPackageReviewsError::BuildStagingSponsor {
                            path: root.clone(),
                            error,
                        }
                    })?;
                    let evaluation_limits = BuildEvaluationSponsorLimits::new(
                        PACKAGE_BUILD_FUEL_CEILING,
                        PACKAGE_BUILD_LOG_CEILING,
                        PACKAGE_FILESYSTEM_ATTEMPT_CEILING,
                        PACKAGE_LIVE_FILESYSTEM_HANDLE_CEILING,
                        PACKAGE_LIVE_CELL_CEILING,
                        PACKAGE_LIVE_TEXT_BYTE_CEILING,
                        PACKAGE_RESULT_CELL_CEILING,
                        PACKAGE_RESULT_TEXT_BYTE_CEILING,
                    )
                    .expect("package build ceilings are nonzero");
                    return Ok(Self {
                        root: canonical_root,
                        filesystem_sponsor,
                        evaluation_sponsor: BuildEvaluationSponsor::new(evaluation_limits),
                        active: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
                        path: root,
                        error,
                    });
                }
            }
        }

        Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
            path: canonical_workspace,
            error: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not reserve a unique package build session after 128 attempts",
            ),
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn filesystem_sponsor(&self) -> &FilesystemSponsor {
        &self.filesystem_sponsor
    }

    pub(crate) fn evaluation_sponsor(&self) -> &BuildEvaluationSponsor {
        &self.evaluation_sponsor
    }

    pub(crate) fn dispose<T>(
        mut self,
        result: Result<T, CompileResolvedPackageReviewsError>,
    ) -> Result<T, CompileResolvedPackageReviewsError> {
        match self.filesystem_sponsor.dispose_private_staging() {
            Ok(()) => {
                self.active = false;
                result
            }
            Err(error) => {
                let path = self.root.clone();
                let prior = result.err().map(Box::new);
                Err(CompileResolvedPackageReviewsError::BuildStagingCleanup { path, error, prior })
            }
        }
    }
}

impl Drop for PackageBuildSession {
    fn drop(&mut self) {
        if self.active {
            let _ = self.filesystem_sponsor.dispose_private_staging();
        }
    }
}
