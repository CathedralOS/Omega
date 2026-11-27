//! embedding.md, "Closing and failure": a discarded guest never resumes, its
//! waiting provider operation is asked to cancel, a late completion is
//! consumed for settlement only, and an uncooperative provider's storage
//! stays retained and charged until it settles.

use super::accepted;
use build_model::accounting::{BuildEvaluationSponsor, EvaluationInvocationAccount};
use build_model::contracts::{
    BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use std::{cell::Cell, rc::Rc};
use terminal_interpreter::{
    TerminalAbandonedEffect, TerminalDiscardStatus, TerminalEffect, TerminalEffectHandler,
    TerminalEffectRejection, TerminalExecutionStatus, TerminalImportAdapter,
    TerminalImportSignature, TerminalInstance, TerminalInstanceCloseError, TerminalInstanceError,
    TerminalInstantiation, TerminalOutstandingCustody, TerminalProgram,
    TerminalRuntimeSupplyRequest,
};

const WAITING: &str = r"
boundary trait Gate { machine visit() reaches none crashes none suspends; }
machine main() -> u8 reaches Gate invokes Gate suspends {
    suspend Gate::visit();
    3
}";

#[derive(Default)]
struct Provider {
    ready: Cell<bool>,
    failing: Cell<bool>,
    confirms_cancellation: Cell<bool>,
    retained_bytes: Cell<u64>,
    handled: Cell<usize>,
    cancellations: Cell<usize>,
    settlements: Cell<usize>,
}

struct Gate {
    signature: TerminalImportSignature,
    provider: Rc<Provider>,
}
impl TerminalImportAdapter for Gate {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for Gate {
    fn poll_effect_readiness(&mut self, _: &TerminalEffect) -> Result<bool, TerminalEffectRejection> {
        if self.provider.failing.get() {
            return Err(TerminalEffectRejection::new("provider lost its device"));
        }
        Ok(self.provider.ready.get())
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.provider.handled.set(self.provider.handled.get() + 1);
        Ok(())
    }
    fn request_effect_cancellation(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalAbandonedEffect, TerminalEffectRejection> {
        self.provider
            .cancellations
            .set(self.provider.cancellations.get() + 1);
        if self.provider.confirms_cancellation.get() {
            return Ok(TerminalAbandonedEffect::Settled);
        }
        self.outstanding(effect)
    }
    fn settle_abandoned_effect(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalAbandonedEffect, TerminalEffectRejection> {
        self.provider
            .settlements
            .set(self.provider.settlements.get() + 1);
        self.outstanding(effect)
    }
}
impl Gate {
    fn outstanding(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalAbandonedEffect, TerminalEffectRejection> {
        Ok(if self.poll_effect_readiness(effect)? {
            TerminalAbandonedEffect::Settled
        } else {
            TerminalAbandonedEffect::Outstanding {
                retained_bytes: self.provider.retained_bytes.get(),
            }
        })
    }
}

const LIVE_PAYLOAD_CEILING: u64 = 100_000;

fn account() -> EvaluationInvocationAccount {
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(
            1_000_000,
            1,
            1,
            1,
            100_000,
            LIVE_PAYLOAD_CEILING,
            100_000,
            100_000,
        )
        .unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap();
    sponsor
        .begin_invocation(sponsor.measurement(), 1_000_000)
        .unwrap()
}

/// Run `body` against a guest parked on its provider. With `account`, the
/// instance charges that invocation.
fn with_waiting_guest(
    account: Option<&EvaluationInvocationAccount>,
    body: impl FnOnce(&mut TerminalInstance<'_, '_>, &Provider),
) {
    let checked = accepted(WAITING).publish_invocation().unwrap();
    let program = TerminalProgram::borrow(checked.artifact.view()).unwrap();
    let imports: Vec<_> = program.imports().collect();
    let [gate] = imports.as_slice() else {
        panic!("one callable host slot expected")
    };
    let supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            assembly_returns: None,
            host_imports: &[gate.id],
            installation: None,
            target_machines: None,
            target_layouts: None,
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: None,
            entry: None,
            external_completions: &[],
        })
        .unwrap();
    let provider = Rc::new(Provider::default());
    let mut instance = program
        .instantiate(TerminalInstantiation {
            installation: Some(&supplies),
            account,
            ..Default::default()
        })
        .unwrap();
    instance
        .bind_import(
            gate.id,
            Gate {
                signature: TerminalImportSignature::from(*gate),
                provider: Rc::clone(&provider),
            },
        )
        .unwrap();
    assert!(matches!(
        instance.invoke(&mut terminal_work::WorkMeter::unbounded()),
        Ok(TerminalExecutionStatus::AwaitingEffect(_))
    ));
    body(&mut instance, &provider);
}

fn outstanding(retained_bytes: u64) -> TerminalDiscardStatus {
    TerminalDiscardStatus::Outstanding(TerminalOutstandingCustody {
        provider_operations: 1,
        retained_bytes,
        charge_refusal: None,
        provider_refusal: None,
    })
}

#[test]
fn a_late_completion_after_discard_settles_without_resuming_the_guest() {
    with_waiting_guest(None, |instance, provider| {
        assert_eq!(instance.discard(), outstanding(0));
        assert_eq!(provider.cancellations.get(), 1);
        assert_eq!(
            instance.invoke(&mut terminal_work::WorkMeter::unbounded()),
            Err(TerminalInstanceError::Discarded)
        );
        assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));
        assert!(!instance.is_closed());

        provider.ready.set(true);
        assert_eq!(instance.discard(), TerminalDiscardStatus::Settled);
        assert_eq!(instance.close(), Ok(()));
        assert!(instance.is_closed());
        // The completion settled the operation; it never reached the guest
        // or performed the host effect.
        assert_eq!(provider.handled.get(), 0);
        assert!(instance.effects().is_empty());
        assert!(instance.result().is_none());
    });
}

#[test]
fn a_confirmed_cancellation_settles_the_discard_at_once() {
    with_waiting_guest(None, |instance, provider| {
        provider.confirms_cancellation.set(true);
        assert_eq!(instance.discard(), TerminalDiscardStatus::Settled);
        assert_eq!(provider.settlements.get(), 0);
        assert_eq!(instance.close(), Ok(()));
        assert_eq!(provider.handled.get(), 0);
    });
}

#[test]
fn an_uncooperative_provider_keeps_its_storage_retained_and_charged() {
    let account = account();
    let retained = 4096;
    with_waiting_guest(Some(&account), |instance, provider| {
        provider.retained_bytes.set(retained);
        for _ in 0..3 {
            assert_eq!(instance.discard(), outstanding(retained));
            assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));
        }
        // The retained bytes stay charged to the invocation; the rest of the
        // guest's storage was reclaimed.
        assert!(
            account
                .reserve_storage(0, LIVE_PAYLOAD_CEILING - retained + 1)
                .is_err()
        );
        drop(
            account
                .reserve_storage(0, LIVE_PAYLOAD_CEILING - retained)
                .unwrap(),
        );

        // A provider that holds more is charged for more.
        provider.retained_bytes.set(2 * retained);
        assert_eq!(instance.discard(), outstanding(2 * retained));
        assert!(
            account
                .reserve_storage(0, LIVE_PAYLOAD_CEILING - retained)
                .is_err()
        );

        provider.ready.set(true);
        assert_eq!(instance.discard(), TerminalDiscardStatus::Settled);
        drop(account.reserve_storage(0, LIVE_PAYLOAD_CEILING).unwrap());
        assert_eq!(instance.close(), Ok(()));
        assert_eq!(provider.handled.get(), 0);
    });
}

#[test]
fn a_faulted_guest_keeps_its_provider_operation_until_it_settles() {
    with_waiting_guest(None, |instance, provider| {
        provider.failing.set(true);
        assert!(matches!(
            instance.invoke(&mut terminal_work::WorkMeter::unbounded()),
            Err(TerminalInstanceError::Execution(_))
        ));
        // A refusal is not a settlement: the provider may still hold the call.
        let TerminalDiscardStatus::Outstanding(custody) = instance.discard() else {
            panic!("a failing provider cannot settle its operation")
        };
        assert_eq!(custody.provider_operations, 1);
        assert_eq!(
            custody.provider_refusal.as_deref(),
            Some("provider lost its device")
        );
        assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));

        provider.failing.set(false);
        provider.ready.set(true);
        assert_eq!(instance.discard(), TerminalDiscardStatus::Settled);
        assert_eq!(instance.close(), Ok(()));
        assert_eq!(provider.handled.get(), 0);
    });
}
