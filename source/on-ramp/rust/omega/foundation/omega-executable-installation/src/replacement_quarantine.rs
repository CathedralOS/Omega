use super::{
    AdmittedArtifact, InstallationDiagnostic, InstalledCode, InstalledCodeEvidence,
    InstalledCodeId, MappingQuarantineId,
};

/// Attributed reason an installed mapping cannot be reclaimed for ordinary
/// reuse after replacement routing has moved elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingQuarantineCause {
    IncompleteDrain { residual_authority_count: usize },
    PossibleOpaqueHolder { provider_identity: String },
}

/// Provider result establishing the fail-closed quarantine transition for one
/// exact installed realization. It does not assert quiescence or discharge any
/// lifecycle obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingQuarantineReceipt {
    installed: InstalledCodeEvidence,
    quarantine: MappingQuarantineId,
    execute_disabled: bool,
    mapping_unmapped: bool,
    range_reserved: bool,
    cause: MappingQuarantineCause,
}

impl MappingQuarantineReceipt {
    pub fn from_provider(
        installed: &InstalledCode,
        quarantine: MappingQuarantineId,
        execute_disabled: bool,
        mapping_unmapped: bool,
        range_reserved: bool,
        cause: MappingQuarantineCause,
    ) -> Self {
        Self {
            installed: InstalledCodeEvidence::from_installed(installed),
            quarantine,
            execute_disabled,
            mapping_unmapped,
            range_reserved,
            cause,
        }
    }
}

/// Reserved, unmapped/trapping range whose placement authority remains
/// unavailable until a wider isolation domain is retired.
#[derive(Debug)]
pub struct QuarantinedInstallation {
    installed: InstalledCodeId,
    previous_artifact: AdmittedArtifact,
    quarantine: MappingQuarantineId,
    cause: MappingQuarantineCause,
    attributed_capacity_loss: u64,
}

impl QuarantinedInstallation {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed
    }

    pub const fn quarantine(&self) -> MappingQuarantineId {
        self.quarantine
    }

    pub const fn cause(&self) -> &MappingQuarantineCause {
        &self.cause
    }

    pub const fn attributed_capacity_loss(&self) -> u64 {
        self.attributed_capacity_loss
    }

    pub const fn previous_artifact(&self) -> &AdmittedArtifact {
        &self.previous_artifact
    }

    /// A call naming the retired realization faults at the trapping mapping.
    /// The returned evidence deliberately has no completion/discharge token.
    pub fn stale_entry_fault(
        &self,
        attempted: InstalledCodeId,
    ) -> Result<StaleEntryFault, InstallationDiagnostic> {
        if attempted != self.installed {
            return Err(InstallationDiagnostic(
                "stale entry attempt does not name this quarantined installation".into(),
            ));
        }
        Ok(StaleEntryFault {
            installed: attempted,
            quarantine: self.quarantine,
        })
    }
}

/// Evidence that a stale entry hit quarantine. A fault detects the stale call;
/// it is not an acknowledgement that any claim, lock, callback, or protocol
/// obligation completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaleEntryFault {
    installed: InstalledCodeId,
    quarantine: MappingQuarantineId,
}

impl StaleEntryFault {
    pub const fn installed_code(self) -> InstalledCodeId {
        self.installed
    }

    pub const fn quarantine(self) -> MappingQuarantineId {
        self.quarantine
    }

    pub const fn discharged_obligations(self) -> bool {
        false
    }
}

/// Consume installed code into a reserved trapping quarantine. Unlike normal
/// retirement, this function never returns `CodePlacement`, so its address
/// range cannot be reused merely because routing moved to a new era.
pub fn quarantine_installed(
    installed: InstalledCode,
    receipt: MappingQuarantineReceipt,
) -> Result<QuarantinedInstallation, Box<MappingQuarantineError>> {
    let evidence = InstalledCodeEvidence::from_installed(&installed);
    let cause_valid = match &receipt.cause {
        MappingQuarantineCause::IncompleteDrain {
            residual_authority_count,
        } => *residual_authority_count > 0,
        MappingQuarantineCause::PossibleOpaqueHolder { provider_identity } => {
            !provider_identity.trim().is_empty()
        }
    };
    let mismatch = if receipt.installed != evidence {
        Some("mapping-quarantine receipt does not match installed code")
    } else if !receipt.execute_disabled {
        Some("mapping quarantine does not establish execute removal")
    } else if !receipt.mapping_unmapped {
        Some("mapping quarantine does not establish an unmapped/trapping range")
    } else if !receipt.range_reserved {
        Some("mapping quarantine does not reserve the retired address range")
    } else if !cause_valid {
        Some("mapping quarantine has no attributed residual holder")
    } else {
        None
    };
    if let Some(message) = mismatch {
        return Err(Box::new(MappingQuarantineError {
            installed,
            receipt,
            diagnostic: InstallationDiagnostic(message.into()),
        }));
    }

    let installed_identity = installed.identity;
    let validated = installed.validated;
    let capacity_loss = validated.frozen.placement.extent.length();
    Ok(QuarantinedInstallation {
        installed: installed_identity,
        previous_artifact: validated.frozen.artifact,
        quarantine: receipt.quarantine,
        cause: receipt.cause,
        attributed_capacity_loss: capacity_loss,
    })
}

#[derive(Debug)]
pub struct MappingQuarantineError {
    installed: InstalledCode,
    receipt: MappingQuarantineReceipt,
    diagnostic: InstallationDiagnostic,
}

impl MappingQuarantineError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstalledCode, MappingQuarantineReceipt) {
        (self.installed, self.receipt)
    }
}
