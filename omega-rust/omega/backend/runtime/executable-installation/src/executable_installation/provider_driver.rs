//! The composed lifecycle each resident realization drives through this
//! provider: every method performs the contracted provider operation and
//! consumes its minted receipt through the matching lifecycle gate in one
//! call.
//!
//! The operations in `owned_image_provider.rs` each produce evidence; the
//! transitions in `executable_installation.rs` consume it. Without this
//! driver a caller must hand-sequence provider call, receipt custody, and
//! gate — the seams where the lifecycle functions had no caller outside
//! tests. Each method here is one call for the pair: the provider performs
//! the work, mints the receipt, and the gate validates it before custody
//! moves. Every refusal returns every input — the receiver-side custody, the
//! authority, and whichever receipts were minted — so a rejected drive leaves
//! nothing partially consumed.
//!
//! The drain-or-quarantine join composes the same way: a caller that knows
//! residual authority still holds the realization supplies the attributed
//! `MappingQuarantineCause`; the provider parks the mapping, reports the
//! drain as incomplete, and the join routes the custody to quarantine. A
//! caller with no residual authority drives the plain retirement ending.

use crate::executable_installation::{
    EntryReferenceAuthority, EntryReferenceReceipt, InstallAuthority, InstallationDiagnostic,
    InstallationReceipt, InstalledCode, InstalledCodeId, InstalledEntryReference,
    MappingQuarantineCause, MappingQuarantineReceipt, QuarantinedInstallation,
    ReplacementAuthority, ReplacementOutcome, ReplacementReceipt, RetiredInstallation,
    RetirementAuthority, RetirementReceipt, UninstallOutcome, ValidatedPlacement,
    install_validated, quarantine_installed, replace_installed, retire_installed,
    uninstall_installed,
};

use super::OwnedImageProvider;
use super::ResidentEntryCall;

#[cfg(test)]
mod tests;

impl OwnedImageProvider {
    /// Drive one validated placement through this provider's contracted
    /// install operation and the lifecycle's install gate in a single call:
    /// the provider copies the final bytes into a resident image, orders the
    /// stores, reads them back, and suspends write authority; the gate then
    /// validates the minted receipt and returns the installed custody.
    ///
    /// A provider-stage refusal returns the placement and authority untouched;
    /// a gate-stage refusal returns them plus the receipt the gate rejected.
    /// The returned identity names the provider-resident image the installed
    /// code binds.
    pub fn install_code(
        &mut self,
        validated: ValidatedPlacement,
        authority: InstallAuthority,
    ) -> Result<(InstalledCodeId, InstalledCode), InstallCodeError> {
        let (installed, receipt) = match self.install(&validated, &authority) {
            Ok(pair) => pair,
            Err(diagnostic) => {
                return Err(InstallCodeError {
                    validated,
                    authority,
                    receipt: None,
                    diagnostic,
                });
            }
        };
        match install_validated(validated, authority, receipt) {
            Ok(code) => Ok((installed, code)),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (validated, authority, receipt) = error.into_parts();
                Err(InstallCodeError {
                    validated,
                    authority,
                    receipt: Some(receipt),
                    diagnostic,
                })
            }
        }
    }

    /// Drive one declared entry through this provider's entry-sealing
    /// operation and the lifecycle's control-flow-integrity gate in a single
    /// call: the provider replays the committed entry extent and reports
    /// requirement compatibility and fetch visibility; the gate then
    /// validates the minted receipt and returns the sealed reference.
    ///
    /// The provider borrows the installed realization, so the returned
    /// reference cannot outlive this borrow of the code custody.
    pub fn seal_code_entry<'installed>(
        &self,
        installed: &'installed InstalledCode,
        authority: EntryReferenceAuthority,
    ) -> Result<InstalledEntryReference<'installed>, SealCodeEntryError> {
        let receipt = match self.seal_entry(installed, &authority) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(SealCodeEntryError {
                    authority,
                    receipt: None,
                    diagnostic,
                });
            }
        };
        match installed.seal_entry_reference(authority, receipt) {
            Ok(reference) => Ok(reference),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (authority, receipt) = error.into_parts();
                Err(SealCodeEntryError {
                    authority,
                    receipt: Some(receipt),
                    diagnostic,
                })
            }
        }
    }

    /// Seal one declared entry and invoke it in a single call: the provider
    /// performs its entry-sealing operation, the CFI gate mints the
    /// [`InstalledEntryReference`], and the provider's call path hands the
    /// resident entry extent to the caller's executor. While the returned
    /// call is held, this provider stays borrowed — the in-flight call is
    /// the executor-quiescence witness a later retirement asks about.
    pub fn call_code<'provider, 'installed>(
        &'provider self,
        installed: &'installed InstalledCode,
        authority: EntryReferenceAuthority,
    ) -> Result<ResidentEntryCall<'provider>, CallCodeError<'installed>> {
        let reference = match self.seal_code_entry(installed, authority) {
            Ok(reference) => reference,
            Err(error) => return Err(CallCodeError::Seal(error)),
        };
        match self.call(installed, &reference) {
            Ok(call) => Ok(call),
            Err(diagnostic) => Err(CallCodeError::Call {
                reference,
                diagnostic,
            }),
        }
    }

    /// Drive one installed realization through this provider's contracted
    /// retirement and the lifecycle's retire gate: the provider unwinds the
    /// write-to-execute transition — execute authority off, write authority
    /// restored — and the gate returns the placement. The provider's resident
    /// storage stays mapped until the caller, holding the retired outcome,
    /// invokes `release` on the returned identity.
    pub fn retire_code(
        &mut self,
        installed: InstalledCode,
        authority: RetirementAuthority,
    ) -> Result<(InstalledCodeId, RetiredInstallation), RetireCodeError> {
        let receipt = match self.retire(&installed, &authority) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(RetireCodeError {
                    installed,
                    authority,
                    receipt: None,
                    diagnostic,
                });
            }
        };
        let identity = installed.identity();
        match retire_installed(installed, authority, receipt) {
            Ok(retired) => Ok((identity, retired)),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (installed, authority, receipt) = error.into_parts();
                Err(RetireCodeError {
                    installed,
                    authority,
                    receipt: Some(receipt),
                    diagnostic,
                })
            }
        }
    }

    /// Drive one installed realization through this provider's quarantine
    /// transition and the lifecycle's quarantine gate: the provider parks the
    /// mapping as an unserved reserved range, and the gate returns the
    /// attributed quarantine custody. The range stays reserved; no release
    /// can free it.
    pub fn quarantine_code(
        &mut self,
        installed: InstalledCode,
        cause: MappingQuarantineCause,
    ) -> Result<QuarantinedInstallation, QuarantineCodeError> {
        let receipt = match self.quarantine(&installed, cause.clone()) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(QuarantineCodeError {
                    installed,
                    cause,
                    receipt: None,
                    diagnostic,
                });
            }
        };
        match quarantine_installed(installed, receipt) {
            Ok(quarantined) => Ok(quarantined),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let (installed, receipt) = error.into_parts();
                Err(QuarantineCodeError {
                    installed,
                    cause,
                    receipt: Some(receipt),
                    diagnostic,
                })
            }
        }
    }

    /// Drive one installed realization through the uninstall join: with no
    /// `residual` cause the provider performs the retirement drain and the
    /// join retires the custody outright; with an attributed residual holder
    /// the provider reports the incomplete drain, parks the mapping in its
    /// trapping quarantine, and the join takes the quarantine ending.
    ///
    /// On a retired ending the provider keeps the drained storage resident
    /// until the caller invokes `release` on the returned identity; on a
    /// quarantined ending the range stays reserved under trapping.
    pub fn uninstall_code(
        &mut self,
        installed: InstalledCode,
        authority: RetirementAuthority,
        residual: Option<MappingQuarantineCause>,
    ) -> Result<(InstalledCodeId, UninstallOutcome), UninstallCodeError> {
        let identity = installed.identity();
        let (retirement, quarantine) = match &residual {
            None => match self.retire(&installed, &authority) {
                Ok(receipt) => (receipt, None),
                Err(diagnostic) => {
                    return Err(UninstallCodeError {
                        installed,
                        authority,
                        retirement: None,
                        quarantine: None,
                        cause: residual,
                        drain_diagnostic: diagnostic,
                        quarantine_diagnostic: None,
                    });
                }
            },
            Some(cause) => {
                // Residual authority remains: the drain cannot complete, so
                // the provider reports it unperformed and performs the
                // quarantine transition the join will route to.
                let drain = RetirementReceipt::from_provider(
                    &installed,
                    false,
                    false,
                    false,
                    std::iter::empty(),
                );
                match self.quarantine(&installed, cause.clone()) {
                    Ok(receipt) => (drain, Some(receipt)),
                    Err(diagnostic) => {
                        return Err(UninstallCodeError {
                            installed,
                            authority,
                            retirement: Some(drain),
                            quarantine: None,
                            cause: residual,
                            drain_diagnostic: diagnostic,
                            quarantine_diagnostic: None,
                        });
                    }
                }
            }
        };
        match uninstall_installed(installed, authority, retirement, quarantine) {
            Ok(outcome) => Ok((identity, outcome)),
            Err(error) => {
                let drain_diagnostic = error.retirement_diagnostic().clone();
                let quarantine_diagnostic = error.quarantine_diagnostic().cloned();
                let (installed, authority, retirement, quarantine) = error.into_parts();
                Err(UninstallCodeError {
                    installed,
                    authority,
                    retirement: Some(retirement),
                    quarantine,
                    cause: residual,
                    drain_diagnostic,
                    quarantine_diagnostic,
                })
            }
        }
    }

    /// Drive one installed realization through the replacement join: the
    /// provider patches every demanded declared site with its bound admitted
    /// fragment, then the superseded custody drains — retiring outright when
    /// `residual` is `None`, or parking in quarantine when a residual holder
    /// keeps the mapping unreclaimable.
    pub fn replace_code(
        &mut self,
        installed: InstalledCode,
        successor: &InstalledCode,
        authority: ReplacementAuthority,
        retirement_authority: RetirementAuthority,
        residual: Option<MappingQuarantineCause>,
    ) -> Result<(InstalledCodeId, ReplacementOutcome), ReplaceCodeError> {
        let patch = match self.patch(&installed, successor, &authority) {
            Ok(receipt) => receipt,
            Err(diagnostic) => {
                return Err(ReplaceCodeError {
                    installed,
                    authority,
                    retirement_authority,
                    patch: None,
                    retirement: None,
                    quarantine: None,
                    cause: residual,
                    diagnostic,
                    drain_diagnostic: None,
                    quarantine_diagnostic: None,
                });
            }
        };
        let identity = installed.identity();
        let (retirement, quarantine) = match &residual {
            None => match self.retire(&installed, &retirement_authority) {
                Ok(receipt) => (receipt, None),
                Err(diagnostic) => {
                    return Err(ReplaceCodeError {
                        installed,
                        authority,
                        retirement_authority,
                        patch: Some(patch),
                        retirement: None,
                        quarantine: None,
                        cause: residual,
                        diagnostic,
                        drain_diagnostic: None,
                        quarantine_diagnostic: None,
                    });
                }
            },
            Some(cause) => {
                let drain = RetirementReceipt::from_provider(
                    &installed,
                    false,
                    false,
                    false,
                    std::iter::empty(),
                );
                match self.quarantine(&installed, cause.clone()) {
                    Ok(receipt) => (drain, Some(receipt)),
                    Err(diagnostic) => {
                        return Err(ReplaceCodeError {
                            installed,
                            authority,
                            retirement_authority,
                            patch: Some(patch),
                            retirement: Some(drain),
                            quarantine: None,
                            cause: residual,
                            diagnostic,
                            drain_diagnostic: None,
                            quarantine_diagnostic: None,
                        });
                    }
                }
            }
        };
        match replace_installed(
            installed,
            successor,
            authority,
            patch,
            retirement_authority,
            retirement,
            quarantine,
        ) {
            Ok(outcome) => Ok((identity, outcome)),
            Err(error) => {
                let patch_diagnostic = error.patch_diagnostic().cloned();
                let drain_diagnostic = error.retirement_diagnostic().cloned();
                let quarantine_diagnostic = error.quarantine_diagnostic().cloned();
                let (installed, authority, patch, retirement_authority, retirement, quarantine) =
                    error.into_parts();
                Err(ReplaceCodeError {
                    installed,
                    authority,
                    retirement_authority,
                    patch: Some(patch),
                    retirement: Some(retirement),
                    quarantine,
                    cause: residual,
                    diagnostic: patch_diagnostic.unwrap_or_else(|| {
                        InstallationDiagnostic("replacement join refused".into())
                    }),
                    drain_diagnostic,
                    quarantine_diagnostic,
                })
            }
        }
    }
}

/// A refused [`OwnedImageProvider::install_code`] drive: the provider-stage
/// refusal returns no receipt; a gate-stage refusal returns the minted
/// receipt the gate rejected.
#[derive(Debug)]
pub struct InstallCodeError {
    validated: ValidatedPlacement,
    authority: InstallAuthority,
    receipt: Option<InstallationReceipt>,
    diagnostic: InstallationDiagnostic,
}

impl InstallCodeError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    /// The minted install receipt when the refusal was the gate's rather
    /// than the provider's.
    pub const fn receipt(&self) -> Option<&InstallationReceipt> {
        self.receipt.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedPlacement,
        InstallAuthority,
        Option<InstallationReceipt>,
    ) {
        (self.validated, self.authority, self.receipt)
    }
}

impl std::fmt::Display for InstallCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstallCodeError {}

/// A refused [`OwnedImageProvider::seal_code_entry`] drive.
#[derive(Debug)]
pub struct SealCodeEntryError {
    authority: EntryReferenceAuthority,
    receipt: Option<EntryReferenceReceipt>,
    diagnostic: InstallationDiagnostic,
}

impl SealCodeEntryError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EntryReferenceAuthority, Option<EntryReferenceReceipt>) {
        (self.authority, self.receipt)
    }
}

impl std::fmt::Display for SealCodeEntryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for SealCodeEntryError {}

/// A refused [`OwnedImageProvider::call_code`] drive. `Seal` carries the
/// seal-stage inputs; `Call` keeps the minted reference so the caller can
/// retry the call without re-sealing.
#[derive(Debug)]
pub enum CallCodeError<'installed> {
    Seal(SealCodeEntryError),
    Call {
        reference: InstalledEntryReference<'installed>,
        diagnostic: InstallationDiagnostic,
    },
}

impl CallCodeError<'_> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        match self {
            Self::Seal(error) => error.diagnostic(),
            Self::Call { diagnostic, .. } => diagnostic,
        }
    }
}

impl std::fmt::Display for CallCodeError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl std::error::Error for CallCodeError<'_> {}

/// A refused [`OwnedImageProvider::retire_code`] drive.
#[derive(Debug)]
pub struct RetireCodeError {
    installed: InstalledCode,
    authority: RetirementAuthority,
    receipt: Option<RetirementReceipt>,
    diagnostic: InstallationDiagnostic,
}

impl RetireCodeError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        RetirementAuthority,
        Option<RetirementReceipt>,
    ) {
        (self.installed, self.authority, self.receipt)
    }
}

impl std::fmt::Display for RetireCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for RetireCodeError {}

/// A refused [`OwnedImageProvider::quarantine_code`] drive.
#[derive(Debug)]
pub struct QuarantineCodeError {
    installed: InstalledCode,
    cause: MappingQuarantineCause,
    receipt: Option<MappingQuarantineReceipt>,
    diagnostic: InstallationDiagnostic,
}

impl QuarantineCodeError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        MappingQuarantineCause,
        Option<MappingQuarantineReceipt>,
    ) {
        (self.installed, self.cause, self.receipt)
    }
}

impl std::fmt::Display for QuarantineCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for QuarantineCodeError {}

/// A refused [`OwnedImageProvider::uninstall_code`] drive: every supplied
/// input returns, and `drain_diagnostic` names the leg that refused first —
/// the provider's drain, the provider's quarantine, or the join itself.
#[derive(Debug)]
pub struct UninstallCodeError {
    installed: InstalledCode,
    authority: RetirementAuthority,
    retirement: Option<RetirementReceipt>,
    quarantine: Option<MappingQuarantineReceipt>,
    cause: Option<MappingQuarantineCause>,
    drain_diagnostic: InstallationDiagnostic,
    quarantine_diagnostic: Option<InstallationDiagnostic>,
}

impl UninstallCodeError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.drain_diagnostic
    }

    pub const fn quarantine_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.quarantine_diagnostic.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        RetirementAuthority,
        Option<RetirementReceipt>,
        Option<MappingQuarantineReceipt>,
        Option<MappingQuarantineCause>,
    ) {
        (
            self.installed,
            self.authority,
            self.retirement,
            self.quarantine,
            self.cause,
        )
    }
}

impl std::fmt::Display for UninstallCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.drain_diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UninstallCodeError {}

/// A refused [`OwnedImageProvider::replace_code`] drive: every supplied
/// input returns, with the refused stage's diagnostic in `diagnostic` and
/// any later drain-leg diagnostics kept alongside.
#[derive(Debug)]
pub struct ReplaceCodeError {
    installed: InstalledCode,
    authority: ReplacementAuthority,
    retirement_authority: RetirementAuthority,
    patch: Option<ReplacementReceipt>,
    retirement: Option<RetirementReceipt>,
    quarantine: Option<MappingQuarantineReceipt>,
    cause: Option<MappingQuarantineCause>,
    diagnostic: InstallationDiagnostic,
    drain_diagnostic: Option<InstallationDiagnostic>,
    quarantine_diagnostic: Option<InstallationDiagnostic>,
}

impl ReplaceCodeError {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub const fn drain_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.drain_diagnostic.as_ref()
    }

    pub const fn quarantine_diagnostic(&self) -> Option<&InstallationDiagnostic> {
        self.quarantine_diagnostic.as_ref()
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        InstalledCode,
        ReplacementAuthority,
        RetirementAuthority,
        Option<ReplacementReceipt>,
        Option<RetirementReceipt>,
        Option<MappingQuarantineReceipt>,
        Option<MappingQuarantineCause>,
    ) {
        (
            self.installed,
            self.authority,
            self.retirement_authority,
            self.patch,
            self.retirement,
            self.quarantine,
            self.cause,
        )
    }
}

impl std::fmt::Display for ReplaceCodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ReplaceCodeError {}
