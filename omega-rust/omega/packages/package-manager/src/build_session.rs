/*
Invocation-owned private staging and aggregate Build resource grants.

We borrow one `PackageBuildSession` across review targets and passes rather
than replenishing its grants. Every fresh compilation attempt receives an
explicitly sponsored empty directory, and checked results retain immutable
output custody after session disposal.

The invocation ceilings bound retained compiler resources (deterministic
Terminal work, BuildLog bytes, canonical filesystem operation attempts, live
filesystem handles, live interpreter storage cells, live Text backing bytes,
and retained result cells/bytes) without claiming to bound host CPU time or
process memory. Fresh invocations choose Terminal measurement units explicitly
so old tree-measured records remain distinct and cannot pay for new runs. A
failed accounting commit during directory creation aborts the invocation so
session disposal removes the private directory before any checked result can
escape.
*/

use std::iter::empty;
use std::process::id;
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

const PACKAGE_BUILD_WORK_CEILING: u64 = 100_000_000;
const PACKAGE_BUILD_LOG_CEILING: u64 = 16 * 1024 * 1024;
const PACKAGE_FILESYSTEM_ATTEMPT_CEILING: u64 = 65_536;
const PACKAGE_LIVE_FILESYSTEM_HANDLE_CEILING: u64 = 4_096;
const PACKAGE_LIVE_CELL_CEILING: u64 = 1_048_576;
const PACKAGE_LIVE_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;
const PACKAGE_RESULT_CELL_CEILING: u64 = 1_048_576;
const PACKAGE_RESULT_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;

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
            let root =
                canonical_workspace.join(format!(".omega-package-build-{}-{sequence}", id()));
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
                        PACKAGE_BUILD_WORK_CEILING,
                        PACKAGE_BUILD_LOG_CEILING,
                        PACKAGE_FILESYSTEM_ATTEMPT_CEILING,
                        PACKAGE_LIVE_FILESYSTEM_HANDLE_CEILING,
                        PACKAGE_LIVE_CELL_CEILING,
                        PACKAGE_LIVE_TEXT_BYTE_CEILING,
                        PACKAGE_RESULT_CELL_CEILING,
                        PACKAGE_RESULT_TEXT_BYTE_CEILING,
                    )
                    .expect("package build ceilings are nonzero");
                    let evaluation_sponsor = BuildEvaluationSponsor::new_with_measurement(
                        evaluation_limits,
                        EvaluationMeasurementIdentity::TerminalPsi {
                            work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
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

    pub(crate) fn create_occurrence_directory(
        &mut self,
        key: &crate::declarations::PackageKey,
        resolution: &package_source::ImmutableSourceResolution,
        purpose: crate::declarations::DependencyPurpose,
    ) -> Result<PathBuf, CompileResolvedPackageReviewsError> {
        let base = package_build_root(&self.root, key, resolution, purpose);
        self.create_build_directory(base)
    }

    pub fn source_build_request(
        &mut self,
        capture: &package_compilation::CapturedLocalSource,
        entry: &Path,
        profile: target::TargetProfile,
        execution: package_compilation::BuildExecutionInputs,
    ) -> Result<compiler::CheckedCompileRequest, CompileResolvedPackageReviewsError> {
        self.require_open()?;
        let profile = execution.profile.unwrap_or(profile);
        let directory = match execution.directory {
            Some(directory) => directory,
            None => self.create_build_directory(self.root.join("source"))?,
        };
        Ok(compiler::CheckedCompileRequest {
            source_capture: Some(capture.clone()),
            build: package_compilation::BuildExecutionInputs {
                profile: Some(profile),
                snapshot: execution
                    .snapshot
                    .or_else(|| Some(package_compilation::BuildSnapshotRequest::new(empty()))),
                directory: Some(directory),
                filesystem_sponsor: execution
                    .filesystem_sponsor
                    .or_else(|| Some(self.filesystem_sponsor.clone())),
                evaluation_sponsor: execution
                    .evaluation_sponsor
                    .or_else(|| Some(self.evaluation_sponsor.clone())),
                restricted_grants: execution.restricted_grants,
                dependency_activation: execution.dependency_activation,
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

    pub fn evaluation_sponsor(&self) -> &BuildEvaluationSponsor {
        &self.evaluation_sponsor
    }

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
