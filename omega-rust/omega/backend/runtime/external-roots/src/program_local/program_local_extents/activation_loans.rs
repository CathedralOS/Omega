//! Call-scoped loans over held program-local Extents.
//!
//! Beyond-call dispositions live in [`super::retained_foreign_arguments`];
//! the ordinary borrow inside the establishing entry call is an
//! [`Extent::loan`]/[`Extent::loan_mut`] subrange. The registry mints that
//! subrange only when the exact [`ProgramLocalEntryActivation`] that observed
//! and established the occurrence is presented: the activation token exists
//! only inside its enter/leave scope on the issuing lifecycle ledger, so a
//! foreign ledger, a different epoch, or another invocation of the same
//! epoch rejects before any subrange authority is derived. The loan carries
//! the account's program-local origin and ends with the borrow, before the
//! account can complete through [`ProgramLocalExtentRegistry::retire`].

use extents::{Extent, ExtentLoan};

use super::{ExternalRootDiagnostic, ProgramLocalExtentRegistry};
use crate::ProgramLocalEntryActivation;

impl ProgramLocalExtentRegistry<'_, '_> {
    /// Borrow one subrange of a held program-local Extent for the exact
    /// entry activation that established its account.
    ///
    /// The presented activation is the only call-scope loan authority. Its
    /// issuing ledger, entered era, and invocation must equal the held
    /// occurrence's lifecycle ledger, epoch, and establishing invocation
    /// exactly — the Extent is that invocation's observed parameter
    /// authority, so an activation of another lifecycle ledger, another
    /// epoch, or another invocation of the same epoch cannot borrow it.
    /// Unknown ambient backing, substituted lineage, and mismatched runtime
    /// facts reject through the same held-account check as retained foreign
    /// arguments.
    pub fn loan_under_activation<'extent>(
        &self,
        activation: &ProgramLocalEntryActivation,
        extent: &'extent Extent,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'extent>, ExternalRootDiagnostic> {
        self.validate_activation_loan(activation, extent)?;
        extent
            .loan(offset, length)
            .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))
    }

    /// Exclusive-borrow variant of [`Self::loan_under_activation`].
    pub fn loan_mut_under_activation<'extent>(
        &self,
        activation: &ProgramLocalEntryActivation,
        extent: &'extent mut Extent,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'extent>, ExternalRootDiagnostic> {
        self.validate_activation_loan(activation, extent)?;
        extent
            .loan_mut(offset, length)
            .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))
    }

    /// Reject a loan not presented under the account's establishing
    /// activation: the Extent must resolve to a held account through
    /// [`ProgramLocalExtentRegistry::validate_backing`], and the
    /// activation's issuing ledger, entered era, and invocation must match
    /// that occurrence's lifecycle ledger, epoch, and establishing
    /// invocation exactly.
    fn validate_activation_loan(
        &self,
        activation: &ProgramLocalEntryActivation,
        extent: &Extent,
    ) -> Result<(), ExternalRootDiagnostic> {
        let (held, _origin) = self.validate_backing(extent)?;
        let occurrence = held.root.occurrence_identity();
        if occurrence.lifecycle_ledger() != activation.ledger()
            || occurrence.lifecycle_epoch() != activation.era_identity()
        {
            return Err(ExternalRootDiagnostic(
                "program-local Extent loan is not under a live activation of the exact occurrence lifecycle ledger and epoch"
                    .into(),
            ));
        }
        if held.root.invocation() != activation.invocation() {
            return Err(ExternalRootDiagnostic(
                "program-local Extent loan is not under the activation that established the exact occurrence"
                    .into(),
            ));
        }
        Ok(())
    }
}
