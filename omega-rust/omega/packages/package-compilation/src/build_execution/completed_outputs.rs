//! Completed build files, detached from scratch and bound to their execution.
//!
//! A staged tree also contains temporary files and generated source. Only the
//! settled required-output roster selects products. Selection shares immutable
//! bytes; neither report construction nor publication executes the build again.

use super::{BuildObservationIdentity, BuildObservationSummary};
use build_output::{BuildStagedOutputEntryKind, BuildStagedOutputTree};
use std::path::{Path, PathBuf};

mod verification;

#[derive(Debug)]
pub struct RetainedBuildOutputs {
    observation: BuildObservationIdentity,
    tree: BuildStagedOutputTree,
    manifest: Vec<u8>,
    identity: [u8; 32],
    published_directory: Option<PathBuf>,
}

impl RetainedBuildOutputs {
    /// Select exactly the completed files after build settlement. A build
    /// with no required outputs has no file product, even if it wrote scratch.
    pub fn from_observation(observation: &BuildObservationSummary) -> Result<Option<Self>, String> {
        let names = observation
            .required_output_settlements()
            .iter()
            .map(|settlement| settlement.relative_path())
            .collect::<Vec<_>>();
        if names.is_empty() {
            return Ok(None);
        }
        let tree = observation
            .staged_output_tree()
            .ok_or("completed build outputs have no retained staged content")?
            .select_files(&names)
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        let observation = observation.identity();
        let mut manifest = b"OMEGA-COMPLETED-BUILD-OUTPUTS-V1\0".to_vec();
        manifest.extend_from_slice(observation.as_bytes());
        manifest.extend_from_slice(&tree.digest());
        crate::codec_bytes::push_encoded(&mut manifest, (names.len() as u64).to_le_bytes());
        for entry in tree.entries() {
            if let BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() {
                crate::codec_bytes::push_encoded(
                    &mut manifest,
                    (entry.relative_path().len() as u64).to_le_bytes(),
                );
                manifest.extend_from_slice(entry.relative_path());
                crate::codec_bytes::push_encoded(&mut manifest, (bytes.len() as u64).to_le_bytes());
                manifest.extend_from_slice(&crate::digest_encoder::content_identity(bytes));
            }
        }
        let identity = crate::digest_encoder::content_identity(&manifest);
        Ok(Some(Self {
            observation,
            tree,
            manifest,
            identity,
            published_directory: None,
        }))
    }

    pub const fn observation_identity(&self) -> BuildObservationIdentity {
        self.observation
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub fn files(&self) -> &BuildStagedOutputTree {
        &self.tree
    }
    pub(crate) fn manifest_bytes(&self) -> &[u8] {
        &self.manifest
    }
    pub fn published_directory(&self) -> Option<&Path> {
        self.published_directory.as_deref()
    }

    /// Install the content-addressed file set and its manifest; an existing set
    /// is reused only after its manifest and complete file namespace match
    /// the retained product. The manifest is installed last for a fresh set.
    /// Failure may leave a partial copy but never records successful publication.
    pub fn publish(&mut self, build_directory: &Path) -> Result<(), String> {
        if self.published_directory.is_some() {
            return Err("completed outputs have already been published".into());
        }
        let name = crate::digest_encoder::hex(self.identity());
        let sets = build_directory.join("completed");
        std::fs::create_dir_all(&sets)
            .map_err(|error| format!("cannot create completed-output directory: {error}"))?;
        let directory = sets.join(&name);
        let fresh = match std::fs::create_dir(&directory) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(format!("cannot create completed-output set: {error}")),
        };
        let files = directory.join("files");
        let manifest = directory.join("manifest.bin");
        if fresh {
            std::fs::create_dir(&files)
                .map_err(|error| format!("cannot create completed-output files: {error}"))?;
            self.files()
                .materialize_into(&files)
                .map_err(|error| error.to_string())?;
            platform_custody::file_publication::publish_exact_file_bytes(
                &manifest,
                self.manifest_bytes(),
            )?;
        } else {
            // A digest in a host pathname is not custody. An earlier export
            // can have been interrupted or changed since its publication.
            verification::verify_existing_set(
                build_directory,
                &name,
                self.files(),
                self.manifest_bytes(),
            )?;
        }
        self.published_directory = Some(directory);
        Ok(())
    }
}
