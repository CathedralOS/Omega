/*
We represent caller-supplied platform environment assumptions for one exact
installed execution scope (`BootstrapEnvironmentAssumption` and
`X86InterruptDeliveryAssumption`). These records carry reviewed environment
description digests and provenance strings; they are explicit caller premises
rather than hardware observations, software-fault proofs, or resource grants.
*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationEnvironmentAssumption {
    Bootstrap(BootstrapEnvironmentAssumption),
    InterruptDelivery(X86InterruptDeliveryAssumption),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapEnvironmentAssumption {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub profile: target::TargetProfile,
    pub entry: semantic_vocabulary::MachineId,
    pub installation: semantic_vocabulary::OperationId,
    pub interval: BootstrapAssumptionInterval,
    pub environment_description_sha256: [u8; 32], // Digest of the reviewed launch/environment description retained with the artifact.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86InterruptDeliveryAssumption {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub profile: target::TargetProfile,
    pub entry: semantic_vocabulary::MachineId,
    pub installation: semantic_vocabulary::OperationId,
    pub interval: InterruptDeliveryInterval,
    pub delivery: X86InterruptDeliveryProfile,
    pub environment_description_sha256: [u8; 32],
    pub source_provenance: String,
    pub caller_provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryInterval {
    InstalledScopeLifetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86InterruptDeliveryProfile {
    ArchitecturalExceptionsAndExternalIrqs, // Vectors 32..255 for external IRQs, 2/18 for true NMI/MCE; no guest debug injection or active hardware breakpoints.
}

impl X86InterruptDeliveryAssumption {
    pub fn validate_description(&self) -> Result<(), &'static str> {
        if self.profile.native_target().architecture != target::Architecture::X86_64 {
            return Err("interrupt delivery assumption requires an x86-64 target");
        }
        validate_description(
            &self.environment_description_sha256,
            &self.source_provenance,
            &self.caller_provenance,
        )
    }
}

impl InstallationEnvironmentAssumption {
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Bootstrap(_) => {
                "no asynchronous NMI, machine check or physical failure from the named entry until IDT publication"
            }
            Self::InterruptDelivery(_) => {
                "while this Scope is installed, one execution context owns its table/TSS/stacks; ordinary external interrupts use vectors 32..255; vectors 2/18 use true NMI/MCE delivery and architectural blocking; guest debug injection, tracing and active hardware breakpoints are absent"
            }
        }
    }

    pub fn validate_description(&self) -> Result<(), &'static str> {
        match self {
            Self::Bootstrap(row) => row.validate_description(),
            Self::InterruptDelivery(row) => row.validate_description(),
        }
    }

    pub fn bootstrap(&self) -> Option<&BootstrapEnvironmentAssumption> {
        match self {
            Self::Bootstrap(row) => Some(row),
            _ => None,
        }
    }

    pub fn interrupt_delivery(&self) -> Option<&X86InterruptDeliveryAssumption> {
        match self {
            Self::InterruptDelivery(row) => Some(row),
            _ => None,
        }
    }

    pub const fn condition_tag(&self) -> u8 {
        match self {
            Self::Bootstrap(_) => 0,
            Self::InterruptDelivery(_) => 1,
        }
    }

    pub fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        match self {
            Self::Bootstrap(row) => row.psi,
            Self::InterruptDelivery(row) => row.psi,
        }
    }

    pub fn profile(&self) -> target::TargetProfile {
        match self {
            Self::Bootstrap(row) => row.profile,
            Self::InterruptDelivery(row) => row.profile,
        }
    }

    pub fn entry(&self) -> semantic_vocabulary::MachineId {
        match self {
            Self::Bootstrap(row) => row.entry,
            Self::InterruptDelivery(row) => row.entry,
        }
    }

    pub fn installation(&self) -> semantic_vocabulary::OperationId {
        match self {
            Self::Bootstrap(row) => row.installation,
            Self::InterruptDelivery(row) => row.installation,
        }
    }

    pub fn environment_description_sha256(&self) -> &[u8; 32] {
        match self {
            Self::Bootstrap(row) => &row.environment_description_sha256,
            Self::InterruptDelivery(row) => &row.environment_description_sha256,
        }
    }

    pub fn source_provenance(&self) -> &str {
        match self {
            Self::Bootstrap(row) => &row.source_provenance,
            Self::InterruptDelivery(row) => &row.source_provenance,
        }
    }

    pub fn caller_provenance(&self) -> &str {
        match self {
            Self::Bootstrap(row) => &row.caller_provenance,
            Self::InterruptDelivery(row) => &row.caller_provenance,
        }
    }
}

fn validate_description(digest: &[u8; 32], source: &str, caller: &str) -> Result<(), &'static str> {
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
