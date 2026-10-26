//! Invocation-owned private staging and aggregate Build resource grants.
//!
//! Review targets and passes borrow one session rather than replenishing its grants. Every
//! fresh compilation attempt receives an explicitly sponsored empty directory;
//! checked results retain immutable output custody after session disposal.

mod bindings;
mod order;
pub use bindings::PackageSourceBindings;
pub(crate) use order::{dependency_first_package_order, package_build_root};

use crate::review::CompileResolvedPackageReviewsError;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use filesystem_authority::FilesystemSponsor;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static PACKAGE_BUILD_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Deterministic Terminal work granted to one complete package
/// invocation. The sibling limits below bound retained compiler resources; none
/// claims to bound CPU time or process memory.
const PACKAGE_BUILD_FUEL_CEILING: u64 = 100_000_000;
/// Aggregate compiler-owned BuildLog bytes across all invocation passes.
const PACKAGE_BUILD_LOG_CEILING: u64 = 16 * 1024 * 1024;
/// Aggregate canonical filesystem operation attempts across all invocation passes.
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
/// Aggregate recursive value cells returned by successful evaluations across
/// all invocation passes.
const PACKAGE_RESULT_CELL_CEILING: u64 = 1_048_576;
/// Aggregate Text payload bytes retained by successful build results.
const PACKAGE_RESULT_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;

/// Resource owner for one invocation, including every review target and round.
/// Borrow this owner through candidate compilation; source preparation and
/// target-specific authority do not replenish its grants.
pub struct PackageBuildSession {
    root: PathBuf,
    filesystem_sponsor: FilesystemSponsor,
    evaluation_sponsor: BuildEvaluationSponsor,
    next_occurrence: u64,
    active: bool,
}

impl std::fmt::Debug for PackageBuildSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageBuildSession")
            .field("root", &self.root)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl PackageBuildSession {
    /// Reserve private staging and bounded sponsors for one invocation. The
    /// caller owns disposal after all targets and review rounds have finished.
    pub fn create(build_workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
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
            match FilesystemSponsor::create_private(
                &root,
                filesystem_authority::FilesystemSponsorLimits::COMPILER_DEFAULT,
            ) {
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
                    // A fresh invocation chooses Terminal units explicitly. Old
                    // tree-measured records remain distinct and cannot pay for it.
                    let evaluation_sponsor = BuildEvaluationSponsor::new_with_measurement(
                        evaluation_limits,
                        EvaluationMeasurementIdentity::TerminalPsi {
                            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                        },
                    )
                    .map_err(|error| {
                        CompileResolvedPackageReviewsError::BuildStagingCreate {
                            path: root.clone(),
                            error: io::Error::other(error),
                        }
                    })?;
                    return Ok(Self {
                        root: canonical_root,
                        filesystem_sponsor,
                        evaluation_sponsor,
                        next_occurrence: 0,
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

    /// Reserve a fresh private output root for this compilation attempt, even
    /// when another pass already evaluated the same package occurrence.
    pub(crate) fn create_occurrence_directory(
        &mut self,
        key: &crate::declarations::PackageKey,
        resolution: &package_source::ImmutableSourceResolution,
        purpose: crate::declarations::DependencyPurpose,
    ) -> Result<PathBuf, CompileResolvedPackageReviewsError> {
        let base = package_build_root(&self.root, key, resolution, purpose);
        self.create_build_directory(base)
    }

    /// Admit acquired source work before its Build declares a package name.
    /// These sponsors belong to the whole invocation, including subsequent
    /// review and Product occurrences. This grants no restricted operation.
    pub fn source_build_request(
        &mut self,
        capture: &package_compilation::CapturedLocalSource,
        entry: &Path,
        profile: target::TargetProfile,
    ) -> Result<compiler::CheckedCompileRequest, CompileResolvedPackageReviewsError> {
        let directory = self.create_build_directory(self.root.join("source"))?;
        Ok(compiler::CheckedCompileRequest {
            source_capture: Some(capture.clone()),
            build: package_compilation::BuildExecutionInputs {
                directory: Some(directory),
                filesystem_sponsor: Some(self.filesystem_sponsor.clone()),
                evaluation_sponsor: Some(self.evaluation_sponsor.clone()),
                ..Default::default()
            },
            ..compiler::CheckedCompileRequest::new(entry, Some(profile.target_name()))
        })
    }

    fn create_build_directory(
        &mut self,
        base: PathBuf,
    ) -> Result<PathBuf, CompileResolvedPackageReviewsError> {
        self.require_open()?;
        let sequence = self.next_occurrence;
        self.next_occurrence = sequence.checked_add(1).ok_or_else(|| {
            CompileResolvedPackageReviewsError::BuildStagingCreate {
                path: self.root.clone(),
                error: io::Error::other("package build occurrence sequence exhausted"),
            }
        })?;
        let directory = base.with_extension(sequence.to_string());
        let sponsor_error = |error| CompileResolvedPackageReviewsError::BuildStagingSponsor {
            path: directory.clone(),
            error,
        };
        let path = self
            .filesystem_sponsor
            .bind_path(&directory)
            .map_err(sponsor_error)?;
        let prepared = self
            .filesystem_sponsor
            .prepare_create_directory(&path)
            .map_err(sponsor_error)?;
        if let Err(error) = fs::create_dir(&directory) {
            prepared.abort();
            return Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
                path: directory,
                error,
            });
        }
        // A failed accounting commit aborts the invocation. Session disposal
        // removes the private directory before any checked result can escape.
        prepared.commit().map_err(sponsor_error)?;
        Ok(directory)
    }

    pub(crate) fn require_open(&self) -> Result<(), CompileResolvedPackageReviewsError> {
        if self.active {
            return Ok(());
        }
        Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
            path: self.root.clone(),
            error: io::Error::other("package build invocation is already closed"),
        })
    }

    pub(crate) fn filesystem_sponsor(&self) -> &FilesystemSponsor {
        &self.filesystem_sponsor
    }

    /// Observe or spend the invocation's existing allowance. Cloning this
    /// sponsor shares its counters; it never creates a fresh budget.
    pub fn evaluation_sponsor(&self) -> &BuildEvaluationSponsor {
        &self.evaluation_sponsor
    }

    /// Release staging before publishing accepted project files. Closing does
    /// not create another grant, and this owner cannot admit another Build.
    pub fn close(&mut self) -> Result<(), CompileResolvedPackageReviewsError> {
        if !self.active {
            return Ok(());
        }
        self.filesystem_sponsor
            .dispose_private_staging()
            .map_err(
                |error| CompileResolvedPackageReviewsError::BuildStagingCleanup {
                    path: self.root.clone(),
                    error,
                    prior: None,
                },
            )?;
        self.active = false;
        Ok(())
    }

    /// Finish the invocation on success or failure, retaining an earlier
    /// operation failure in the diagnostic if staging cleanup also fails.
    pub fn dispose<T, Error>(mut self, result: Result<T, Error>) -> Result<T, Error>
    where
        Error: std::fmt::Display + From<CompileResolvedPackageReviewsError>,
    {
        match self.close() {
            Ok(()) => result,
            Err(mut error) => {
                if let CompileResolvedPackageReviewsError::BuildStagingCleanup { prior, .. } =
                    &mut error
                {
                    *prior = result.err().map(|error| error.to_string());
                }
                Err(Error::from(error))
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
