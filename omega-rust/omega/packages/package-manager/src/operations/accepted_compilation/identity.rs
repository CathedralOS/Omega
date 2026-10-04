//! Join the loaded compiler and the actually selected toolchain library.

use sha2::{Digest, Sha256};

use crate::lock::PackageLockTarget;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};

pub(crate) fn compiler_input_identity() -> Result<[u8; 32], String> {
    let library = package_compilation::toolchain_library::identity()?;
    let mut digest = Sha256::new();
    digest.update(b"omega-accepted-compiler-inputs-v1\0");
    digest.update(compiler::compiler_identity());
    digest.update(library.as_bytes());
    Ok(digest.finalize().into())
}

/// Refuse stale acceptance before a Build activation or compiler pass executes.
/// Request-path spellings are navigation, not compiler input: the same canonical
/// local project can be selected through `.` or an absolute path. Package
/// identity, resolution, root role, and all dependency selections remain exact.
pub(super) fn require_current_acceptance(
    closure: &ExactTargetPackageSourceClosure<'_>,
    accepted: &PackageLockTarget,
    compiler_identity: [u8; 32],
) -> Result<(), String> {
    let source = CanonicalSourceClosureSubject::from_resolved(
        closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    let root_name = source.root().selected().key().name().as_str();
    let stale = |package: &str, reason: &str| {
        format!(
            "accepted package `{package}` {reason}; run `omega update` for fresh review before compiling"
        )
    };
    if accepted.target() != source.target_profile()
        || accepted.execution_profile() != target::TargetProfile::host_if_supported()
    {
        return Err(stale(
            root_name,
            "was reviewed for a different compilation context",
        ));
    }
    if accepted.source().root().selected().key() != source.root().selected().key()
        || accepted.source().root_role() != source.root_role()
    {
        return Err(stale(
            root_name,
            "has a different accepted root identity or role",
        ));
    }
    for current in source.packages() {
        let previous = accepted
            .source()
            .packages()
            .iter()
            .find(|previous| previous.key() == current.key())
            .ok_or_else(|| {
                stale(
                    current.key().name().as_str(),
                    "has no accepted source identity",
                )
            })?;
        if previous.resolution() != current.resolution() {
            return Err(stale(
                current.key().name().as_str(),
                "source identity changed",
            ));
        }
        if accepted
            .source()
            .package_dependency_projection(current.key())
            != source.package_dependency_projection(current.key())
            || accepted.source().package_navigation(current.key())
                != source.package_navigation(current.key())
        {
            return Err(stale(
                current.key().name().as_str(),
                "source selection changed",
            ));
        }
    }
    if accepted.source().packages().len() != source.packages().len()
        || accepted.source().dependency_requests() != source.dependency_requests()
    {
        return Err(stale(root_name, "dependency selections changed"));
    }
    // Lock recovery already validates exact purpose/target occurrence coverage.
    // Compare every occurrence, not just the root or the first target section.
    for occurrence in accepted.occurrences() {
        if occurrence
            .compilation()
            .map(|inputs| inputs.compiler_identity())
            != Some(compiler_identity)
        {
            let package = source
                .packages()
                .iter()
                .find(|package| package.key().identity() == occurrence.acceptance().package())
                .map(|package| package.key().name().as_str())
                .unwrap_or(root_name);
            return Err(stale(package, "compiler identity changed"));
        }
    }
    Ok(())
}
