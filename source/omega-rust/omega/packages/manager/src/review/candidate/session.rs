use super::CompileResolvedPackageReviewsError;
use psi_checked_interpreter::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, FilesystemSponsor,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static REVIEW_BUILD_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Deterministic evaluator work granted to one complete package-review
/// closure. The sibling limits below bound retained compiler resources; none
/// claims to bound CPU time or process memory.
const PACKAGE_REVIEW_BUILD_FUEL_CEILING: u64 = 100_000_000;
/// Aggregate compiler-owned BuildLog bytes across initial evaluation and
/// replay for the complete package closure.
const PACKAGE_REVIEW_BUILD_LOG_CEILING: u64 = 16 * 1024 * 1024;
/// Aggregate canonical filesystem operation attempts across initial evaluation
/// and replay for the complete package closure.
const PACKAGE_REVIEW_FILESYSTEM_ATTEMPT_CEILING: u64 = 65_536;
/// Compiler-owned filesystem resources live concurrently across the package
/// closure. This does not count unrelated host-process descriptors.
const PACKAGE_REVIEW_LIVE_FILESYSTEM_HANDLE_CEILING: u64 = 4_096;
/// Interpreter semantic storage-cell allocations live concurrently across the
/// package closure. This is a cell count, not a memory-byte claim.
const PACKAGE_REVIEW_LIVE_CELL_CEILING: u64 = 1_048_576;
/// Logical bytes held by concurrently live interpreter Text backing buffers.
/// This is not Vec capacity, allocator overhead, or a process-memory ceiling.
const PACKAGE_REVIEW_LIVE_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;
/// Aggregate recursive value cells returned by successful initial and replay
/// evaluations across the closure.
const PACKAGE_REVIEW_RESULT_CELL_CEILING: u64 = 1_048_576;
/// Aggregate Text payload bytes retained by successful build results.
const PACKAGE_REVIEW_RESULT_TEXT_BYTE_CEILING: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct ReviewBuildSession {
    root: PathBuf,
    filesystem_sponsor: FilesystemSponsor,
    evaluation_sponsor: BuildEvaluationSponsor,
    active: bool,
}

impl ReviewBuildSession {
    pub(super) fn create(
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
            let sequence = REVIEW_BUILD_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = canonical_workspace.join(format!(
                ".omega-package-evidence-{}-{sequence}",
                std::process::id()
            ));
            match create_private_directory(&root) {
                Ok(()) => {
                    let canonical_root = match fs::canonicalize(&root) {
                        Ok(canonical_root) => canonical_root,
                        Err(error) => {
                            let _ = fs::remove_dir(&root);
                            return Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
                                path: root,
                                error,
                            });
                        }
                    };
                    if canonical_root.parent() != Some(canonical_workspace.as_path()) {
                        let _ = fs::remove_dir(&canonical_root);
                        return Err(CompileResolvedPackageReviewsError::BuildStagingCreate {
                            path: canonical_root,
                            error: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "created review session escaped its canonical workspace",
                            ),
                        });
                    }
                    let filesystem_sponsor = match FilesystemSponsor::new(&canonical_root) {
                        Ok(sponsor) => sponsor,
                        Err(error) => {
                            let _ = fs::remove_dir(&canonical_root);
                            return Err(CompileResolvedPackageReviewsError::BuildStagingSponsor {
                                path: canonical_root,
                                error,
                            });
                        }
                    };
                    let evaluation_limits = BuildEvaluationSponsorLimits::new(
                        PACKAGE_REVIEW_BUILD_FUEL_CEILING,
                        PACKAGE_REVIEW_BUILD_LOG_CEILING,
                        PACKAGE_REVIEW_FILESYSTEM_ATTEMPT_CEILING,
                        PACKAGE_REVIEW_LIVE_FILESYSTEM_HANDLE_CEILING,
                        PACKAGE_REVIEW_LIVE_CELL_CEILING,
                        PACKAGE_REVIEW_LIVE_TEXT_BYTE_CEILING,
                        PACKAGE_REVIEW_RESULT_CELL_CEILING,
                        PACKAGE_REVIEW_RESULT_TEXT_BYTE_CEILING,
                    )
                    .expect("package-review build ceilings are nonzero");
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
                "could not reserve a unique package-review build session after 128 attempts",
            ),
        })
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn filesystem_sponsor(&self) -> &FilesystemSponsor {
        &self.filesystem_sponsor
    }

    pub(super) fn evaluation_sponsor(&self) -> &BuildEvaluationSponsor {
        &self.evaluation_sponsor
    }

    pub(super) fn dispose<T>(
        mut self,
        result: Result<T, CompileResolvedPackageReviewsError>,
    ) -> Result<T, CompileResolvedPackageReviewsError> {
        match fs::remove_dir_all(&self.root) {
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

impl Drop for ReviewBuildSession {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

#[cfg(test)]
mod tests {
    use super::{REVIEW_BUILD_SESSION_SEQUENCE, ReviewBuildSession};
    use crate::declarations::{PackageKey, PackageName};
    use crate::review::candidate::CompileResolvedPackageReviewsError;
    use omega_package_source::SourceLineage;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    fn temporary_workspace(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omega-package-evidence-{label}-{}-{}",
            std::process::id(),
            REVIEW_BUILD_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn review_build_sessions_are_fresh_and_dispose_only_their_owned_child() {
        let workspace = temporary_workspace("lifecycle");
        fs::create_dir(&workspace).expect("create review workspace");
        let sentinel = workspace.join("caller-owned");
        fs::write(&sentinel, b"retain").expect("write caller-owned sentinel");

        let session = ReviewBuildSession::create(&workspace).expect("create review session");
        let session_root = session.root().to_path_buf();
        let canonical_workspace = fs::canonicalize(&workspace).unwrap();
        assert_eq!(session_root.parent(), Some(canonical_workspace.as_path()));
        assert!(fs::read_dir(&session_root).unwrap().next().is_none());
        fs::write(session_root.join("staged"), b"discard").expect("write staged output");

        session.dispose(Ok(())).expect("dispose review session");

        assert!(!session_root.exists());
        assert_eq!(fs::read(&sentinel).unwrap(), b"retain");
        fs::remove_dir_all(workspace).expect("remove review workspace");
    }

    #[test]
    fn review_build_sessions_dispose_staging_after_review_failure() {
        let workspace = temporary_workspace("failure");
        let session = ReviewBuildSession::create(&workspace).expect("create review session");
        let session_root = session.root().to_path_buf();
        fs::write(session_root.join("partial"), b"discard").expect("write partial output");
        let package = PackageKey::new(
            PackageName::parse("arithmetic-kernels").unwrap(),
            SourceLineage::git("https://github.com/CathedralOS/arithmetic-kernels.git").unwrap(),
        );

        let result: Result<(), _> =
            session.dispose(Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package,
            }));

        assert!(matches!(
            result,
            Err(CompileResolvedPackageReviewsError::IdentityMismatch { .. })
        ));
        assert!(!session_root.exists());
        fs::remove_dir_all(workspace).expect("remove review workspace");
    }

    #[test]
    fn review_build_sessions_withhold_success_when_cleanup_fails() {
        let workspace = temporary_workspace("cleanup-failure");
        let session = ReviewBuildSession::create(&workspace).expect("create review session");
        let session_root = session.root().to_path_buf();
        fs::remove_dir(&session_root).expect("remove owned empty session");
        fs::write(&session_root, b"replacement").expect("replace session directory with a file");

        let result = session.dispose(Ok(()));

        assert!(matches!(
            result,
            Err(CompileResolvedPackageReviewsError::BuildStagingCleanup { prior: None, .. })
        ));
        fs::remove_file(session_root).expect("remove replacement file");
        fs::remove_dir_all(workspace).expect("remove review workspace");
    }
}
