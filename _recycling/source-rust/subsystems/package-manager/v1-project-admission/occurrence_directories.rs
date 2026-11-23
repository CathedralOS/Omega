/*
    Parked from `source-rust/subsystems/package-manager/review/build_session.rs`
    and `review/build_session/order.rs` on 2026-11-23. Their only caller is this
    directory's `accepted_compilation.rs`, which gave each accepted package
    occurrence its own Build directory and the session's filesystem sponsor.
    Restore them into `PackageBuildSession` together with that caller.

    `package_build_root` encodes the package key digest, content digest, and
    dependency purpose directly into one child directory of the session root
    rather than creating intermediate directories outside the filesystem
    sponsor's creation protocol.
*/

// In `impl PackageBuildSession` (review/build_session.rs):

    pub(crate) fn create_occurrence_directory(
        &mut self,
        key: &crate::declarations::PackageKey,
        resolution: &source_acquisition::ImmutableSourceResolution,
        purpose: crate::declarations::DependencyPurpose,
    ) -> Result<PathBuf, CompileResolvedPackageReviewsError> {
        let base = package_build_root(&self.root, key, resolution, purpose);
        self.create_build_directory(base)
    }

    pub(crate) fn filesystem_sponsor(&self) -> &FilesystemSponsor {
        &self.filesystem_sponsor
    }

// In review/build_session/order.rs:

use crate::declarations::DependencyPurpose;
use crate::declarations::PackageKey;
use source_acquisition::ImmutableSourceResolution;
use std::path::{Path, PathBuf};

pub(crate) fn package_build_root(
    build_root: &Path,
    key: &PackageKey,
    resolution: &ImmutableSourceResolution,
    purpose: DependencyPurpose,
) -> PathBuf {
    build_root.join(format!(
        "{}-{}-{}",
        crate::Hex(&key.identity().digest()),
        resolution.content().to_hex(),
        purpose.name(),
    ))
}

#[cfg(test)]
mod tests {
    use super::package_build_root;
    use crate::declarations::DependencyPurpose;
    use crate::declarations::{PackageKey, PackageName};
    use source_acquisition::{ImmutableSourceResolution, SourceContentDigest, SourceLineage};
    use std::path::Path;

    #[test]
    fn build_roots_bind_package_and_source_selection() {
        let key = PackageKey::new(
            PackageName::parse("arithmetic_kernels").unwrap(),
            SourceLineage::git("https://github.com/CathedralOS/arithmetic-kernels.git").unwrap(),
        );
        let first = ImmutableSourceResolution::workspace(SourceContentDigest::derive(b"a"));
        let second = ImmutableSourceResolution::workspace(SourceContentDigest::derive(b"b"));

        assert_ne!(
            package_build_root(Path::new("build"), &key, &first, DependencyPurpose::Product),
            package_build_root(
                Path::new("build"),
                &key,
                &second,
                DependencyPurpose::Product
            )
        );
        let product =
            package_build_root(Path::new("build"), &key, &first, DependencyPurpose::Product);
        let helper = package_build_root(Path::new("build"), &key, &first, DependencyPurpose::Build);
        assert_ne!(product, helper);
        assert_eq!(product.parent(), Some(Path::new("build")));
        assert_eq!(helper.parent(), Some(Path::new("build")));
    }
}
