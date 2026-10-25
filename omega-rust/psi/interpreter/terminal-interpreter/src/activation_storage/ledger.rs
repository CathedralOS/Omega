//! The interpreter pays for each live claim before the authority ledger grows.
//! The foundation ledger still owns all establishment, lineage and release
//! judgments. Its records are logical cells, not a bound on Vec capacity or RSS.

use crate::errors::{TerminalInterpretError, malformed};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use extents::{ActivationClaimId, ActivationClaimLedger, ActivationClaimRequest};

pub(super) struct RuntimeActivationLedger {
    ledger: ActivationClaimLedger,
    lease: Option<EvaluationStorageLease>,
    account: Option<EvaluationInvocationAccount>,
}

impl RuntimeActivationLedger {
    pub(super) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            ledger: ActivationClaimLedger::new(),
            lease: None,
            account: account.cloned(),
        }
    }

    fn reserve_claim(&mut self) -> Result<(), TerminalInterpretError> {
        if let Some(lease) = &mut self.lease {
            lease.grow(1, 0).map_err(TerminalInterpretError::Storage)?;
        } else if let Some(account) = &self.account {
            self.lease = Some(
                account
                    .reserve_storage(1, 0)
                    .map_err(TerminalInterpretError::Storage)?,
            );
        }
        Ok(())
    }

    fn retire_claims(&mut self) {
        if let Some(lease) = &mut self.lease {
            // Only admitted growth reaches this ledger; removal and semantic
            // refusal can never increase its reserved count.
            lease.truncate_cells(
                u64::try_from(self.ledger.live_claims().count()).unwrap_or(u64::MAX),
            );
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.ledger.live_claims().next().is_none()
    }

    pub(super) fn establish(
        &mut self,
        request: ActivationClaimRequest,
    ) -> Result<ActivationClaimId, TerminalInterpretError> {
        self.reserve_claim()?;
        let result = self.ledger.establish(request).map_err(|_| malformed());
        self.retire_claims();
        result
    }

    pub(super) fn release(
        &mut self,
        identity: ActivationClaimId,
    ) -> Result<(), TerminalInterpretError> {
        self.ledger.release(identity).map_err(|_| malformed())?;
        self.retire_claims();
        Ok(())
    }

    pub(super) fn split(
        &mut self,
        identity: ActivationClaimId,
        committed_at: u64,
        bound_at: u64,
    ) -> Result<Option<(ActivationClaimId, ActivationClaimId)>, TerminalInterpretError> {
        // One parent becomes two children. A budget refusal leaves the parent
        // live; a semantic rejection refunds the unused extra reservation.
        self.reserve_claim()?;
        let children = self
            .ledger
            .split_claim(identity, committed_at, bound_at)
            .ok();
        self.retire_claims();
        Ok(children)
    }

    pub(super) fn merge(
        &mut self,
        lower: ActivationClaimId,
        upper: ActivationClaimId,
    ) -> Option<ActivationClaimId> {
        let parent = self.ledger.merge_claims(lower, upper).ok();
        self.retire_claims();
        parent
    }
}
