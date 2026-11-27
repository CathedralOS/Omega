//! Bind the Psi work meter to the account that pays for one Build evaluation.
use crate::accounting::EvaluationInvocationAccount;
use crate::contracts::{EvaluationAccountingError, EvaluationMeasurementIdentity};
use terminal_work::{WorkMeter, WorkPayment, WorkSchedule};

#[derive(Debug, Clone)]
pub struct EvaluationWorkPayment(Option<EvaluationInvocationAccount>);
pub type EvaluationWorkMeter = WorkMeter<EvaluationWorkPayment>;

impl EvaluationWorkPayment {
    pub fn invocation(&self) -> Option<&EvaluationInvocationAccount> {
        self.0.as_ref()
    }
}

impl WorkPayment for EvaluationWorkPayment {
    fn pay(&self, units: u64) -> Result<(), String> {
        if let Some(account) = &self.0 {
            account
                .charge_work(units)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

pub fn sponsored_meter(
    account: EvaluationInvocationAccount,
    quantum: Option<u64>,
) -> Result<EvaluationWorkMeter, EvaluationAccountingError> {
    let EvaluationMeasurementIdentity::TerminalPsi { work_schedule, .. } = account.measurement();
    if work_schedule != WorkSchedule::CURRENT.identity() {
        return Err(EvaluationAccountingError::MeasurementSponsorMismatch);
    }
    Ok(WorkMeter::with_payment(
        EvaluationWorkPayment(Some(account)),
        quantum,
    ))
}

pub fn unsponsored_meter(quantum: Option<u64>) -> EvaluationWorkMeter {
    WorkMeter::with_payment(EvaluationWorkPayment(None), quantum)
}
