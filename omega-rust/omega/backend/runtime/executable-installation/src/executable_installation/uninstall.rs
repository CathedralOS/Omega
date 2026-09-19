//! The uninstall join over the retirement and quarantine transitions
//! (wiki/spec/build/executable_installation.md#visibility-and-retirement).
//!
//! One decision point consumes installed code toward its two endings: a
//! complete drain retires the realization and returns the placement for
//! reuse, and a drain that cannot complete — quiescence unproven, residual
//! authority outstanding, or an opaque holder — routes to the trapping
//! quarantine that reserves the range. A provider that reports an incomplete
//! drain supplies both receipts; a caller that omits the quarantine receipt
//! asks for retirement only and keeps every input when it fails.
//!
//! Both legs validate the complete installed evidence independently, so a
//! mismatched retirement receipt never reaches quarantine on its word, and a
//! quarantine receipt naming a different realization rejects the same way it
//! does at the direct crossing. Any refusal returns every supplied input:
//! nothing about the mapping's custody is lost to a failed transition.

use crate::executable_installation::{
    InstallationDiagnostic, InstalledCode, MappingQuarantineReceipt, QuarantinedInstallation,
    RetiredInstallation, RetirementAuthority, RetirementError, RetirementReceipt,
    quarantine_installed, retire_installed,
};

/// The two endings an uninstall produces: the placement returned through a
/// complete retirement, or the same range held reserved and trapping under
/// quarantine.
#[derive(Debug)]
pub enum UninstallOutcome {
    Retired(RetiredInstallation),
    Quarantined(QuarantinedInstallation),
}

/// Every input, returned when neither ending could be established. The
/// diagnostics record both legs when a quarantine receipt was attempted:
/// `retirement` is the drain refusal and `quarantine` the parking refusal.
#[derive(Debug)]
pub struct UninstallError {
    installed: InstalledCode,
    authority: RetirementAuthority,
    retirement: RetirementReceipt,
    quarantine: Option<MappingQuarantineReceipt>,
    retirement_diagnostic: InstallationDiagnostic,
    quarantine_diagnostic: Option<InstallationDiagnostic>,
}

impl UninstallError {
    /// The retirement leg's refusal — always present; it is what sent the
    /// uninstall looking for the quarantine route.
    pub const fn retirement_diagnostic(&self) -> &InstallationDiagnostic {
        &self.retirement_diagnostic
    }

    /// The quarantine leg's refusal, present only when a quarantine receipt
    /// was supplied and itself rejected.
    pub const fn quarantine_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.quarantine_diagnostic.as_ref()
    }

    /// All inputs back, unchanged: the installed custody, both authorities'
    /// receipt evidence and the authority itself.
    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        RetirementAuthority,
        RetirementReceipt,
        Option<MappingQuarantineReceipt>,
    ) {
        (
            self.installed,
            self.authority,
            self.retirement,
            self.quarantine,
        )
    }
}

/// Uninstall one installed realization: drain it toward retired custody, and
/// when the drain cannot complete, park it in the trapping quarantine if the
/// provider supplied that evidence. `quarantine == None` means a
/// retirement-only request whose failure returns every input.
pub fn uninstall_installed(
    installed: InstalledCode,
    authority: RetirementAuthority,
    retirement: RetirementReceipt,
    quarantine: Option<MappingQuarantineReceipt>,
) -> Result<UninstallOutcome, Box<UninstallError>> {
    let retirement_error = match retire_installed(installed, authority, retirement) {
        Ok(retired) => return Ok(UninstallOutcome::Retired(retired)),
        Err(error) => error,
    };
    let RetirementError {
        installed,
        authority,
        receipt: retirement,
        diagnostic: retirement_diagnostic,
    } = *retirement_error;
    let Some(quarantine_receipt) = quarantine else {
        return Err(Box::new(UninstallError {
            installed,
            authority,
            retirement,
            quarantine: None,
            retirement_diagnostic,
            quarantine_diagnostic: None,
        }));
    };
    match quarantine_installed(installed, quarantine_receipt) {
        Ok(quarantined) => Ok(UninstallOutcome::Quarantined(quarantined)),
        Err(quarantine_error) => {
            let quarantine_diagnostic = quarantine_error.diagnostic().clone();
            let (installed, quarantine_receipt) = quarantine_error.into_parts();
            Err(Box::new(UninstallError {
                installed,
                authority,
                retirement,
                quarantine: Some(quarantine_receipt),
                retirement_diagnostic,
                quarantine_diagnostic: Some(quarantine_diagnostic),
            }))
        }
    }
}
