use super::inputs::reachable_package_keys;
use super::{CompileResolvedPackageReviewsError, PackageSourceVerificationPhase};
use crate::graph::ResolvedPackageSourceClosure;
use crate::identity::PackageKey;
use omega_package_source::ImmutableSourceResolution;
use omega_package_source::local::operations::verify_package_source_snapshot;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) fn verify_transitive_source_custody(
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
            custody.materialization().content(),
            custody.source_limits(),
        )
        .map_err(|error| CompileResolvedPackageReviewsError::SourceCustody {
            compiling_package: compiling_package.clone(),
            source_package: source_package.clone(),
            phase,
            error,
        })?;
        custody.selection_evidence().revalidate().map_err(|error| {
            CompileResolvedPackageReviewsError::SourceSelectionCustody {
                compiling_package: compiling_package.clone(),
                source_package,
                phase,
                error,
            }
        })?;
    }
    Ok(())
}

pub(super) fn dependency_first_package_order(
    closure: &ResolvedPackageSourceClosure,
) -> Vec<PackageKey> {
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

pub(super) fn package_build_root(
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
    use super::package_build_root;
    use crate::identity::{PackageKey, PackageName};
    use omega_package_source::{ImmutableSourceResolution, SourceContentDigest, SourceLineage};
    use std::path::Path;

    #[test]
    fn build_roots_bind_package_and_source_selection() {
        let key = PackageKey::new(
            PackageName::parse("arithmetic-kernels").unwrap(),
            SourceLineage::git("https://github.com/CathedralOS/arithmetic-kernels.git").unwrap(),
        );
        let first = ImmutableSourceResolution::workspace(SourceContentDigest::derive(b"a"));
        let second = ImmutableSourceResolution::workspace(SourceContentDigest::derive(b"b"));

        assert_ne!(
            package_build_root(Path::new("build"), &key, &first),
            package_build_root(Path::new("build"), &key, &second)
        );
    }
}
