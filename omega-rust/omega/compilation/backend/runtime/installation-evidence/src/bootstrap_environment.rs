//! Explicit platform assumption for the interval before interrupt publication.
//!
//! This is caller input, not observed hardware state or proof. Ordinary UEFI
//! requests do not acquire it from their target, permissions, or source names.

/// The caller assumes no asynchronous NMI, machine check, or physical failure
/// from this exact entry until this exact installation publishes its IDT.
/// Software faults, storage validity, CPU configuration and stack capacity
/// remain separate obligations; this assumption establishes none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapEnvironmentAssumption {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub profile: target::TargetProfile,
    pub entry: semantic_vocabulary::MachineId,
    pub installation: semantic_vocabulary::OperationId,
    pub interval: BootstrapAssumptionInterval,
    /// Digest of the reviewed launch/environment description, not a proof that
    /// the physical machine obeys it. Retain that description with the artifact.
    pub environment_description_sha256: [u8; 32],
    pub source_provenance: String,
    pub caller_provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapAssumptionInterval {
    EntryUntilInstallationPublished,
}

impl BootstrapEnvironmentAssumption {
    pub fn validate_description(&self) -> Result<(), &'static str> {
        validate_description(
            &self.environment_description_sha256,
            &self.source_provenance,
            &self.caller_provenance,
        )
    }
}

pub(super) fn validate_description(
    digest: &[u8; 32],
    source: &str,
    caller: &str,
) -> Result<(), &'static str> {
    if *digest == [0; 32] {
        return Err("installation assumption has no environment description digest");
    }
    for value in [source, caller] {
        if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err("installation assumption requires bounded source and caller provenance");
        }
    }
    Ok(())
}
