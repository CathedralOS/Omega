use crate::compiler_handoff::reachable_package_keys;
use crate::source::{SourceResolveError, verify_package_source_snapshot};
use crate::{
    ImmutableSourceResolution, PackageKey, ResolvedPackageSourceClosure,
    package_compilation_inputs_for,
};
use omega_compiler::{
    CheckedPackageReviewProjection, PackageCompilationInputError, PackageReviewEncodingError,
    PackageSourceConsumptionCommitment, compile_to_checked_with_packages_in_build_dir,
    project_checked_package_review,
};
use psi_diagnostics::Diagnostic;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// Compiler-issued review material for one exact package source selection.
///
/// There is deliberately no public constructor. The source resolution and
/// review projection are joined only by compiling resolver-owned custody in
/// `compile_resolved_package_reviews`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIssuedPackageReview {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    projection: CheckedPackageReviewProjection,
    canonical_review_bytes: Vec<u8>,
}

impl CompilerIssuedPackageReview {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub fn projection(&self) -> &CheckedPackageReviewProjection {
        &self.projection
    }

    pub fn canonical_review_bytes(&self) -> &[u8] {
        &self.canonical_review_bytes
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
/// and receives a source-specific writable build directory. Downloaded source
/// remains immutable and cannot supply its own review rows.
pub fn compile_resolved_package_reviews(
    closure: &ResolvedPackageSourceClosure,
    target: &str,
    build_root: &Path,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
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
        let checked = compile_to_checked_with_packages_in_build_dir(
            &custody.snapshot_root().join("main.omg"),
            &package_build_root(build_root, &key, custody.resolution()),
            Some(target),
            inputs,
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
        reviews.push(CompilerIssuedPackageReview {
            key,
            resolution: custody.resolution().clone(),
            source_consumption_commitment,
            projection,
            canonical_review_bytes,
        });
    }
    Ok(CompilerIssuedPackageReviewSet { reviews })
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
