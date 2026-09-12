//! Scoped Source/Output authority, staging custody, and sponsored host access.

use crate::BuildCanonicalSourceMetadataIdentity;
use build_output::{BuildStagedOutputTree, capture, empty};
use build_time_evaluation::{
    BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemSponsor,
};
use checked_interpreter::FilesystemSponsorEntry;
use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

pub const BUILD_SOURCE_ROOT_IDENTITY: BuildMachineFilesystemGrantRootIdentity =
    match BuildMachineFilesystemGrantRootIdentity::new(1) {
        Some(identity) => identity,
        None => panic!("build source root identity must be nonzero"),
    };
pub const BUILD_OUTPUT_ROOT_IDENTITY: BuildMachineFilesystemGrantRootIdentity =
    match BuildMachineFilesystemGrantRootIdentity::new(2) {
        Some(identity) => identity,
        None => panic!("build output root identity must be nonzero"),
    };

/// Host filesystem authority granted to an admitted `build.omg` machine.
///
/// Source roots are read-only. The build directory is the only write root and
/// also permits read-back through the checked interpreter's `RealScoped`
/// contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMachineFilesystemScope {
    source_root: PathBuf,
    canonical_source_metadata: Option<checked_interpreter::CanonicalFilesystemMetadataIndex>,
    canonical_source_metadata_required: bool,
    build_dir: PathBuf,
    sponsor: Option<BuildMachineFilesystemSponsor>,
    replay: Option<checked_interpreter::FilesystemReplay>,
}

impl BuildMachineFilesystemScope {
    pub fn for_root(
        root_path: &Path,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
    ) -> Self {
        let source_root = root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            source_root,
            canonical_source_metadata: None,
            canonical_source_metadata_required: false,
            build_dir,
            sponsor,
            replay: None,
        }
    }

    #[doc(hidden)]
    pub fn for_package_root(
        source_root: PathBuf,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
        metadata: Option<checked_interpreter::CanonicalFilesystemMetadataIndex>,
    ) -> Self {
        Self {
            source_root,
            canonical_source_metadata: metadata,
            canonical_source_metadata_required: true,
            build_dir,
            sponsor,
            replay: None,
        }
    }

    pub fn with_replay(mut self, replay: checked_interpreter::FilesystemReplay) -> Self {
        self.replay = Some(replay);
        self
    }

    pub(super) const fn is_replay(&self) -> bool {
        self.replay.is_some()
    }

    pub(super) fn filesystem_access(&self) -> BuildMachineFilesystemAccess {
        if let Some(replay) = &self.replay {
            return BuildMachineFilesystemAccess::ReplayFilesystem(replay.clone());
        }
        let mut source_root = BuildMachineFilesystemGrantRoot::new(
            BUILD_SOURCE_ROOT_IDENTITY,
            self.source_root.clone(),
        );
        if let Some(metadata) = &self.canonical_source_metadata {
            source_root = source_root.with_canonical_metadata(metadata.clone());
        }
        let grants = BuildMachineFilesystemGrants {
            read_roots: vec![source_root],
            write_roots: vec![BuildMachineFilesystemGrantRoot::new(
                BUILD_OUTPUT_ROOT_IDENTITY,
                self.build_dir.clone(),
            )],
        };
        match &self.sponsor {
            Some(sponsor) => BuildMachineFilesystemAccess::RealScopedSponsored {
                grants,
                sponsor: sponsor.clone(),
            },
            None => BuildMachineFilesystemAccess::RealScoped(grants),
        }
    }

    pub(super) fn ensure_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        if self.replay.is_some() {
            return Ok(());
        }
        if let Some(sponsor) = &self.sponsor {
            let path = sponsor
                .bind_path(&self.build_dir)
                .map_err(|error| self.sponsor_diagnostic(error))?;
            match sponsor
                .entry(&path)
                .map_err(|error| self.sponsor_diagnostic(error))?
            {
                Some(FilesystemSponsorEntry::Directory) => return Ok(()),
                Some(_) => {
                    return Err(vec![Diagnostic::error(format!(
                        "sponsored build machine write root `{}` is not a directory",
                        self.build_dir.display()
                    ))]);
                }
                None => {}
            }
            let prepared = sponsor
                .prepare_create_directory(&path)
                .map_err(|error| self.sponsor_diagnostic(error))?;
            if let Err(error) = std::fs::create_dir(&self.build_dir) {
                prepared.abort();
                return Err(vec![Diagnostic::error(format!(
                    "failed to create sponsored build machine filesystem write root `{}`: {error}",
                    self.build_dir.display()
                ))]);
            }
            if let Err(error) = prepared.commit() {
                let _ = std::fs::remove_dir(&self.build_dir);
                return Err(self.sponsor_diagnostic(error));
            }
            return Ok(());
        }
        std::fs::create_dir_all(&self.build_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to create build machine filesystem write root `{}`: {error}",
                self.build_dir.display()
            ))]
        })
    }

    pub(super) fn ensure_canonical_source_metadata(&self) -> Result<(), Vec<Diagnostic>> {
        if self.canonical_source_metadata_required && self.canonical_source_metadata.is_none() {
            return Err(vec![Diagnostic::error(
                "package-aware build filesystem access requires compiler-validated canonical Source metadata",
            )]);
        }
        Ok(())
    }

    pub(super) const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        match &self.canonical_source_metadata {
            Some(metadata) => Some(BuildCanonicalSourceMetadataIdentity {
                policy_version: metadata.policy_version(),
                source_content_commitment: *metadata.source_content_commitment(),
            }),
            None => None,
        }
    }

    fn sponsor_diagnostic(
        &self,
        error: checked_interpreter::FilesystemSponsorError,
    ) -> Vec<Diagnostic> {
        vec![Diagnostic::error(format!(
            "build machine staging sponsor rejected `{}`: {error}",
            self.build_dir.display()
        ))]
    }

    pub(super) fn staged_output_tree(
        &self,
        filesystem_reachable: bool,
    ) -> Result<Option<BuildStagedOutputTree>, Vec<Diagnostic>> {
        if self.replay.is_some() {
            return Ok(None);
        }
        // When the selected build machine cannot reach the filesystem, the
        // Output namespace is exactly empty without needing a physical
        // staging sponsor. Canonicalize that semantic fact so sponsored
        // review and ordinary production retain the same observation.
        if !filesystem_reachable {
            return Ok(Some(empty()));
        }
        let Some(sponsor) = &self.sponsor else {
            return Ok(None);
        };
        capture(&self.build_dir, sponsor).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::BuildMachineFilesystemScope;
    use build_time_evaluation::BuildMachineFilesystemAccess;
    use checked_interpreter::{FilesystemSponsor, FilesystemSponsorLimits};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static STAGING_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_staging_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "omega-build-staging-{label}-{}-{}",
            std::process::id(),
            STAGING_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn sponsored_build_roots_share_one_session_entry_ceiling() {
        let session_root = temporary_staging_root("shared-account");
        fs::create_dir(&session_root).expect("create session root");
        let sponsor = FilesystemSponsor::with_limits(
            &session_root,
            FilesystemSponsorLimits {
                maximum_entries: 1,
                maximum_total_logical_bytes: 64,
                maximum_object_extent: 64,
            },
        )
        .expect("create filesystem sponsor");

        let first_build_dir = session_root.join("first-package");
        BuildMachineFilesystemScope::for_root(
            &session_root.join("first-source/main.omg"),
            first_build_dir.clone(),
            Some(sponsor.clone()),
        )
        .ensure_write_roots()
        .expect("first package consumes the one available entry");

        let second_build_dir = session_root.join("second-package");
        let diagnostics = BuildMachineFilesystemScope::for_root(
            &session_root.join("second-source/main.omg"),
            second_build_dir.clone(),
            Some(sponsor.clone()),
        )
        .ensure_write_roots()
        .expect_err("the second package must share the exhausted session ceiling");

        assert_eq!(sponsor.snapshot().unwrap().entries, 1);
        assert!(first_build_dir.is_dir());
        assert!(!second_build_dir.exists());
        assert!(diagnostics[0].to_string().contains("entry limit 1"));

        fs::remove_dir_all(session_root).expect("remove session root");
    }

    #[test]
    fn build_scope_attaches_canonical_metadata_only_to_source() {
        let metadata = checked_interpreter::CanonicalFilesystemMetadataIndex::version_1(
            [0x5a; 32],
            [checked_interpreter::CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                checked_interpreter::CanonicalFilesystemMetadataRowKind::Directory,
            )],
        )
        .expect("construct canonical source metadata");
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("build"),
            None,
            Some(metadata.clone()),
        );

        let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
            panic!("unsponsored build scope must use scoped real filesystem grants")
        };
        assert_eq!(grants.read_roots.len(), 1);
        assert_eq!(grants.write_roots.len(), 1);
        assert_eq!(grants.read_roots[0].canonical_metadata(), Some(&metadata));
        assert!(grants.write_roots[0].canonical_metadata().is_none());
    }

    #[test]
    fn package_build_scope_rejects_filesystem_authority_without_canonical_source_metadata() {
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("package-root"),
            PathBuf::from("build"),
            None,
            None,
        );

        let diagnostics = scope
            .ensure_canonical_source_metadata()
            .expect_err("package filesystem access must not fall back to physical metadata");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("canonical Source metadata"));
        assert_eq!(scope.source_root, PathBuf::from("package-root"));
    }
}
