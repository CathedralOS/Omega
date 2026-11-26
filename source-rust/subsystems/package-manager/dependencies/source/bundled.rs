// A distribution lists the declared package names it supplies. Selection stays
// anchored to its verified library even when a caller declares a local namesake.

use super::{
    AcquiredBuildSource, BuildSourceAcquisitions, PackageSourceNavigation,
    PackageSourceSelectionEvidence,
};
use crate::declarations::PackageName;
use crate::resolution::graph::PackageRootSourceRequest;
use package_compilation::toolchain_library;
use source_acquisition::local::resolution_observations::CapturedLocalSource;
use source_acquisition::{
    ImmutableSourceResolution, LocalSourceLimits, SourceContentDigest, SourceLineage,
    SourceRelativePath,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) struct BundledPackages {
    capture: CapturedLocalSource,
    library: SourceContentDigest,
    directories: BTreeMap<PackageName, SourceRelativePath>,
}

impl BundledPackages {
    fn open() -> Result<Self, String> {
        let capture = toolchain_library::snapshot()?;
        let library = SourceContentDigest::parse_hex(&capture.normalized().content_identity)
            .map_err(|error| error.to_string())?;
        let text = capture
            .tree()
            .read_to_string(Path::new(toolchain_library::PACKAGE_CATALOG))
            .map_err(|error| format!("cannot read installed package catalog: {error}"))?;
        let mut directories = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for (index, line) in text.lines().enumerate() {
            let (name, directory) = line
                .split_once('\t')
                .ok_or_else(|| format!("invalid bundled package catalog row {}", index + 1))?;
            let name = PackageName::parse(name)?;
            let directory =
                SourceRelativePath::parse(directory).map_err(|error| error.to_string())?;
            if !paths.insert(directory.clone()) || directories.insert(name, directory).is_some() {
                return Err(format!(
                    "duplicate directory or name in bundled package catalog row {}",
                    index + 1
                ));
            }
        }
        Ok(Self {
            capture,
            library,
            directories,
        })
    }

    pub(super) fn relative_name(
        &self,
        owner: &SourceContentDigest,
        base: &SourceRelativePath,
        location: &str,
    ) -> Result<PackageName, String> {
        if &self.library != owner {
            return Err("relative package request belongs to a different installed library".into());
        }
        let path = super::workspace_path::normalize_member_path(Some(base.as_str()), location)?;
        self.directories
            .iter()
            .find(|(_, value)| value.as_str() == path.as_str())
            .map(|(name, _)| name.clone())
            .ok_or_else(|| {
                format!(
                    "the installed library does not supply a package at {}",
                    path.as_str()
                )
            })
    }
}

impl BuildSourceAcquisitions<'_> {
    pub(super) fn acquire_bundled(
        &mut self,
        name: &PackageName,
        limits: LocalSourceLimits,
    ) -> Result<AcquiredBuildSource, String> {
        if self.bundled.is_none() {
            self.bundled = Some(BundledPackages::open().map_err(|error| {
                format!(
                    "cannot select bundled package named {}: {error}",
                    name.as_str()
                )
            })?);
        }
        let catalog = self
            .bundled
            .as_ref()
            .expect("opened the installed package catalog");
        let directory = catalog.directories.get(name).ok_or_else(|| {
            format!(
                "the installed library does not supply bundled package named {}",
                name.as_str()
            )
        })?;
        let lineage = SourceLineage::Bundled {
            library: catalog.library.clone(),
            directory: directory.clone(),
        };
        if let Some(retained) = self
            .sources
            .iter()
            .find(|candidate| candidate.lineage == lineage)
        {
            let mut acquired = retained.clone();
            acquired.check_bundled_limits(limits)?;
            return Ok(acquired);
        }
        let limits = limits.compiler_bounded();
        let capture = catalog
            .capture
            .workspace_member(
                Path::new(directory.as_str()),
                &source_acquisition::SourceCaptureRequest::entire_tree(),
            )
            .map_err(|error| {
                format!("cannot acquire bundled package {}: {error}", name.as_str())
            })?;
        let content = SourceContentDigest::derive(capture.normalized().content_identity.as_bytes());
        let mut acquired = AcquiredBuildSource {
            capture,
            successor_capture: None,
            lineage,
            limits,
            resolution: ImmutableSourceResolution::Bundled { content },
            root_request: PackageRootSourceRequest::Bundled { name: name.clone() },
            navigation: PackageSourceNavigation::Root,
            selection: PackageSourceSelectionEvidence::Root,
        };
        acquired.check_bundled_limits(limits)?;
        Ok(acquired)
    }
}

impl AcquiredBuildSource {
    fn check_bundled_limits(&mut self, limits: LocalSourceLimits) -> Result<(), String> {
        let limits = limits.compiler_bounded();
        if self.capture.normalized().byte_count > limits.max_bytes
            || self.capture.tree().entries().count() > limits.max_entries
            || self
                .capture
                .tree()
                .entries()
                .any(|(path, _)| path.components().count() > limits.max_depth)
        {
            return Err("bundled package exceeds source acquisition limits".into());
        }
        self.limits = limits;
        Ok(())
    }
}
