use crate::compiler_handoff::reachable_package_keys;
use crate::review_evidence::ReviewOnlyCanonicalRow;
use crate::source::{SourceResolveError, verify_package_source_snapshot};
use crate::{
    ImmutableSourceResolution, PackageKey, ResolvedPackageSourceClosure,
    package_compilation_inputs_for,
};
use omega_compiler::{
    BuildObservationSummary, CheckedPackageReviewProjection, CompilerExecutableCommitment,
    CompilerExecutableCommitmentError, FilesystemSponsor, FilesystemSponsorError,
    PackageCompilationInputError, PackageReviewCanonicalRow, PackageReviewEncodingError,
    PackageSourceConsumptionCommitment, compile_to_checked_with_packages_in_sponsored_build_dir,
    ordinary_package_obligation_ledger_from_compiler_rows, project_checked_package_review,
    validate_ordinary_package_obligation_ledger,
};
use psi_diagnostics::Diagnostic;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static REVIEW_BUILD_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Compiler-issued review material for one exact package source selection.
///
/// There is deliberately no public constructor. The source resolution and
/// review projection are joined only by compiling resolver-owned custody in
/// `compile_resolved_package_reviews`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIssuedPackageReview {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    compiler_executable_commitment: CompilerExecutableCommitment,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    build_observation_summary: Option<BuildObservationSummary>,
    projection: CheckedPackageReviewProjection,
    canonical_review_bytes: Vec<u8>,
    canonical_rows: Vec<PackageReviewCanonicalRow>,
    comparison_rows: Vec<ReviewOnlyCanonicalRow>,
}

impl CompilerIssuedPackageReview {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn compiler_executable_commitment(&self) -> CompilerExecutableCommitment {
        self.compiler_executable_commitment
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    /// Selected build-machine execution evidence. This is deliberately
    /// separate from canonical capability/API comparison bytes.
    pub const fn build_observation_summary(&self) -> Option<&BuildObservationSummary> {
        self.build_observation_summary.as_ref()
    }

    pub fn projection(&self) -> &CheckedPackageReviewProjection {
        &self.projection
    }

    pub fn canonical_review_bytes(&self) -> &[u8] {
        &self.canonical_review_bytes
    }

    pub fn canonical_rows(&self) -> &[PackageReviewCanonicalRow] {
        &self.canonical_rows
    }

    pub(crate) fn comparison_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.comparison_rows
    }
}

/// Complete review-only compiler output for one resolved source closure.
///
/// Rows are dependency-first and deterministic. This remains review material,
/// not an accepted package instance, certificate, or lock payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIssuedPackageReviewSet {
    reviews: Vec<CompilerIssuedPackageReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceVerificationPhase {
    BeforeCompilation,
    AfterCompilation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerExecutableVerificationPhase {
    BeforeCompilation,
    AfterCompilation,
}

impl CompilerIssuedPackageReviewSet {
    pub fn reviews(&self) -> &[CompilerIssuedPackageReview] {
        &self.reviews
    }

    pub fn review(&self, key: &PackageKey) -> Option<&CompilerIssuedPackageReview> {
        self.reviews.iter().find(|review| review.key() == key)
    }
}

#[derive(Debug)]
pub enum CompileResolvedPackageReviewsError {
    BuildStagingCreate {
        path: PathBuf,
        error: io::Error,
    },
    BuildStagingSponsor {
        path: PathBuf,
        error: FilesystemSponsorError,
    },
    BuildStagingCleanup {
        path: PathBuf,
        error: io::Error,
        prior: Option<Box<CompileResolvedPackageReviewsError>>,
    },
    CompilerExecutable {
        phase: CompilerExecutableVerificationPhase,
        error: CompilerExecutableCommitmentError,
    },
    CompilerExecutableDrift {
        before: CompilerExecutableCommitment,
        after: CompilerExecutableCommitment,
    },
    SourceCustody {
        compiling_package: PackageKey,
        source_package: PackageKey,
        phase: PackageSourceVerificationPhase,
        error: SourceResolveError,
    },
    CompilationInputs {
        package: PackageKey,
        errors: Vec<PackageCompilationInputError>,
    },
    Compilation {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    Projection {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    Encoding {
        package: PackageKey,
        error: PackageReviewEncodingError,
    },
    SourceConsumptionMissing {
        package: PackageKey,
    },
    SourceConsumptionDrift {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    IdentityMismatch {
        package: PackageKey,
    },
}

impl fmt::Display for CompileResolvedPackageReviewsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildStagingCreate { path, error } => write!(
                formatter,
                "could not create a fresh package-review build session at `{}`: {error}",
                path.display()
            ),
            Self::BuildStagingSponsor { path, error } => write!(
                formatter,
                "failed to sponsor package-review staging root `{}`: {error}",
                path.display()
            ),
            Self::BuildStagingCleanup { path, error, prior } => {
                write!(
                    formatter,
                    "could not dispose package-review build session `{}`: {error}",
                    path.display()
                )?;
                if let Some(prior) = prior {
                    write!(formatter, "; review had already failed: {prior}")?;
                }
                Ok(())
            }
            Self::CompilerExecutable { phase, error } => write!(
                formatter,
                "compiler executable verification failed {phase:?}: {error}"
            ),
            Self::CompilerExecutableDrift { .. } => write!(
                formatter,
                "compiler executable bytes changed while package reviews were being produced"
            ),
            Self::SourceCustody {
                compiling_package,
                source_package,
                phase,
                error,
            } => write!(
                formatter,
                "source custody verification failed {phase:?} for package `{}` while compiling `{}`: {error}",
                source_package.name().as_str(),
                compiling_package.name().as_str()
            ),
            Self::CompilationInputs { package, errors } => write!(
                formatter,
                "compiler input validation failed for package `{}` with {} error(s)",
                package.name().as_str(),
                errors.len()
            ),
            Self::Compilation {
                package,
                diagnostics,
            } => write!(
                formatter,
                "checked compilation failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
            ),
            Self::Projection {
                package,
                diagnostics,
            } => write!(
                formatter,
                "review projection failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
            ),
            Self::Encoding { package, error } => write!(
                formatter,
                "review encoding failed for package `{}`: {error}",
                package.name().as_str()
            ),
            Self::SourceConsumptionMissing { package } => write!(
                formatter,
                "package-aware compilation for `{}` emitted no source-consumption commitment",
                package.name().as_str()
            ),
            Self::SourceConsumptionDrift {
                package,
                diagnostics,
            } => write!(
                formatter,
                "compiler-consumed source verification failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
            ),
            Self::IdentityMismatch { package } => write!(
                formatter,
                "compiler review identity did not match package `{}`",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for CompileResolvedPackageReviewsError {}

/// Compile every package in an exact resolver-owned closure and project its
/// review material locally.
///
/// Each package is temporarily re-rooted over only its transitive dependencies
/// and receives a source-specific writable directory within a fresh disposable
/// review session. Downloaded source remains immutable and cannot supply its
/// own review rows. No review set is returned until the session is removed.
pub fn compile_resolved_package_reviews(
    closure: &ResolvedPackageSourceClosure,
    target: &str,
    build_root: &Path,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let build_session = ReviewBuildSession::create(build_root)?;
    let result = compile_resolved_package_reviews_in_session(
        closure,
        target,
        build_session.root(),
        build_session.sponsor(),
    );
    build_session.dispose(result)
}

fn compile_resolved_package_reviews_in_session(
    closure: &ResolvedPackageSourceClosure,
    target: &str,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let compiler_executable_commitment =
        CompilerExecutableCommitment::derive_current().map_err(|error| {
            CompileResolvedPackageReviewsError::CompilerExecutable {
                phase: CompilerExecutableVerificationPhase::BeforeCompilation,
                error,
            }
        })?;
    let mut reviews = Vec::with_capacity(closure.custodies().len());
    for key in dependency_first_package_order(closure) {
        verify_transitive_source_custody(
            closure,
            &key,
            PackageSourceVerificationPhase::BeforeCompilation,
        )?;
        let custody = closure
            .custody(&key)
            .expect("validated source closure retains custody for every graph package");
        let inputs = package_compilation_inputs_for(closure, &key).map_err(|errors| {
            CompileResolvedPackageReviewsError::CompilationInputs {
                package: key.clone(),
                errors,
            }
        })?;
        let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
            &custody.snapshot_root().join("main.omg"),
            &package_build_root(build_session_root, &key, custody.resolution()),
            Some(target),
            inputs,
            filesystem_sponsor.clone(),
        )
        .map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Compilation {
                package: key.clone(),
                diagnostics,
            },
        )?;
        verify_transitive_source_custody(
            closure,
            &key,
            PackageSourceVerificationPhase::AfterCompilation,
        )?;
        checked
            .verify_current_source_consumption()
            .map_err(
                |diagnostics| CompileResolvedPackageReviewsError::SourceConsumptionDrift {
                    package: key.clone(),
                    diagnostics,
                },
            )?;
        let source_consumption_commitment =
            checked.source_consumption_commitment().ok_or_else(|| {
                CompileResolvedPackageReviewsError::SourceConsumptionMissing {
                    package: key.clone(),
                }
            })?;
        let build_observation_summary = checked.build_observation_summary().cloned();
        let projection = project_checked_package_review(&checked).map_err(|diagnostics| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            }
        })?;
        if projection.package() != key.identity() {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch { package: key });
        }
        let canonical_review_bytes = projection.canonical_review_bytes().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
        let canonical_rows = projection.canonical_rows().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
        let obligation_ledger = ordinary_package_obligation_ledger_from_compiler_rows(
            &canonical_rows,
        )
        .map_err(|error| CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics: vec![Diagnostic::error(format!(
                "compiler-issued ordinary package obligation ledger is structurally invalid: {error}"
            ))],
        })?;
        validate_ordinary_package_obligation_ledger(&obligation_ledger, &checked).map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            },
        )?;
        let comparison_rows = canonical_rows
            .iter()
            .map(ReviewOnlyCanonicalRow::from_compiler_issued)
            .collect();
        reviews.push(CompilerIssuedPackageReview {
            key,
            resolution: custody.resolution().clone(),
            compiler_executable_commitment,
            source_consumption_commitment,
            build_observation_summary,
            projection,
            canonical_review_bytes,
            canonical_rows,
            comparison_rows,
        });
    }
    let compiler_executable_commitment_after = CompilerExecutableCommitment::derive_current()
        .map_err(
            |error| CompileResolvedPackageReviewsError::CompilerExecutable {
                phase: CompilerExecutableVerificationPhase::AfterCompilation,
                error,
            },
        )?;
    if compiler_executable_commitment_after != compiler_executable_commitment {
        return Err(
            CompileResolvedPackageReviewsError::CompilerExecutableDrift {
                before: compiler_executable_commitment,
                after: compiler_executable_commitment_after,
            },
        );
    }
    Ok(CompilerIssuedPackageReviewSet { reviews })
}

#[derive(Debug)]
struct ReviewBuildSession {
    root: PathBuf,
    sponsor: FilesystemSponsor,
    active: bool,
}

impl ReviewBuildSession {
    fn create(build_workspace: &Path) -> Result<Self, CompileResolvedPackageReviewsError> {
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
                ".omega-package-review-{}-{sequence}",
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
                    let sponsor = match FilesystemSponsor::new(&canonical_root) {
                        Ok(sponsor) => sponsor,
                        Err(error) => {
                            let _ = fs::remove_dir(&canonical_root);
                            return Err(CompileResolvedPackageReviewsError::BuildStagingSponsor {
                                path: canonical_root,
                                error,
                            });
                        }
                    };
                    return Ok(Self {
                        root: canonical_root,
                        sponsor,
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

    fn root(&self) -> &Path {
        &self.root
    }

    fn sponsor(&self) -> &FilesystemSponsor {
        &self.sponsor
    }

    fn dispose<T>(
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
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn verify_transitive_source_custody(
    closure: &ResolvedPackageSourceClosure,
    compiling_package: &PackageKey,
    phase: PackageSourceVerificationPhase,
) -> Result<(), CompileResolvedPackageReviewsError> {
    for source_package in reachable_package_keys(closure, compiling_package) {
        let custody = closure
            .custody(&source_package)
            .expect("validated source closure retains every reachable custody");
        verify_package_source_snapshot(
            custody.snapshot_root(),
            custody.resolution().content(),
            custody.source_limits(),
        )
        .map_err(|error| CompileResolvedPackageReviewsError::SourceCustody {
            compiling_package: compiling_package.clone(),
            source_package,
            phase,
            error,
        })?;
    }
    Ok(())
}

fn dependency_first_package_order(closure: &ResolvedPackageSourceClosure) -> Vec<PackageKey> {
    fn visit(
        closure: &ResolvedPackageSourceClosure,
        key: &PackageKey,
        visited: &mut BTreeSet<PackageKey>,
        ordered: &mut Vec<PackageKey>,
    ) {
        if !visited.insert(key.clone()) {
            return;
        }
        let mut dependencies = closure
            .graph()
            .package(key)
            .expect("validated closure contains every traversed package")
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().clone())
            .collect::<Vec<_>>();
        dependencies.sort();
        for dependency in dependencies {
            visit(closure, &dependency, visited, ordered);
        }
        ordered.push(key.clone());
    }

    let mut visited = BTreeSet::new();
    let mut ordered = Vec::with_capacity(closure.custodies().len());
    visit(closure, closure.graph().root(), &mut visited, &mut ordered);
    ordered
}

fn package_build_root(
    build_root: &Path,
    key: &PackageKey,
    resolution: &ImmutableSourceResolution,
) -> PathBuf {
    build_root.join(format!(
        "{}-{}",
        encode_hex(&key.identity().digest()),
        resolution.content().to_hex()
    ))
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_workspace(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omega-package-review-{label}-{}-{}",
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
            crate::PackageName::parse("arithmetic-kernels").unwrap(),
            crate::SourceLineage::git("https://github.com/CathedralOS/arithmetic-kernels.git")
                .unwrap(),
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

    #[test]
    fn build_roots_bind_package_and_source_selection() {
        let key = PackageKey::new(
            crate::PackageName::parse("arithmetic-kernels").unwrap(),
            crate::SourceLineage::git("https://github.com/CathedralOS/arithmetic-kernels.git")
                .unwrap(),
        );
        let first = ImmutableSourceResolution::workspace(crate::SourceContentDigest::derive(b"a"));
        let second = ImmutableSourceResolution::workspace(crate::SourceContentDigest::derive(b"b"));

        assert_ne!(
            package_build_root(Path::new("build"), &key, &first),
            package_build_root(Path::new("build"), &key, &second)
        );
    }
}
