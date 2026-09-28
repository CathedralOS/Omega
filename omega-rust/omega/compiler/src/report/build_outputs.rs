//! Completed build files, detached from scratch and bound to their execution.
//!
//! A staged tree also contains temporary files and generated source. Only the
//! settled required-output roster selects products. Selection shares immutable
//! bytes; neither report construction nor publication executes the build again.

use build_evaluation::{BuildObservationIdentity, BuildObservationSummary};
use build_output::{BuildStagedOutputEntryKind, BuildStagedOutputTree};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
        manifest.extend_from_slice(&(names.len() as u64).to_le_bytes());
        for entry in tree.entries() {
            if let BuildStagedOutputEntryKind::File { bytes, .. } = entry.kind() {
                manifest.extend_from_slice(&(entry.relative_path().len() as u64).to_le_bytes());
                manifest.extend_from_slice(entry.relative_path());
                manifest.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                manifest.extend_from_slice(&Sha256::digest(bytes));
            }
        }
        let identity = Sha256::digest(&manifest).into();
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
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest
    }
    pub fn published_directory(&self) -> Option<&Path> {
        self.published_directory.as_deref()
    }

    /// Record the directory after the publisher installs or replays its exact bytes.
    pub(crate) fn record_publication(&mut self, directory: PathBuf) {
        self.published_directory = Some(directory);
    }
}
