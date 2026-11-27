//! Independently published Omega code may park on its own host import. The
//! outer caller must keep the inner call in custody until the host settles it.
use super::{Gate, LIVE_PAYLOAD_CEILING, Provider, account, host_request, outstanding};
use crate::tests::provider_supplies::checked_provider_closure;
use build_model::accounting::EvaluationInvocationAccount;
use psi_provider_selections::{ProviderInstallationSelection, admit_component_provider};
use std::rc::Rc;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    TerminalComponentBinding, TerminalDiscardStatus, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalImportSignature, TerminalInstance, TerminalInstanceCloseError,
    TerminalInstanceError, TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplyRequest,
    TerminalScalarValue,
};

const SOURCE: &str = r"
public boundary trait Gate { machine visit() reaches none suspends crashes none; }
public boundary trait Service { machine wait(value: u8) -> u8 reaches Gate suspends crashes none; }
public Native satisfies Service {
    machine wait(value: u8) -> u8 reaches Gate suspends crashes none {
        suspend Gate::visit();
        value
    }
}
machine caller() -> u8 reaches Service Gate suspends {
    suspend Service::wait(3)
}
machine producer_entry() {}
";

fn park<'artifact>(
    receiver: &'artifact CanonicalTerminalArtifact,
    producer: &'artifact CanonicalTerminalArtifact,
    provider: Rc<Provider>,
    account: &EvaluationInvocationAccount,
) -> TerminalInstance<'static, 'artifact> {
    let module = producer.semantic_module();
    let [candidate] = module.interfaces.provider_candidates.as_slice() else {
        panic!("expected one published service provider")
    };
    let export = module
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| row.id == candidate.boundary)
        .unwrap();
    let caller_program = TerminalProgram::borrow(receiver.view()).unwrap();
    let slot = receiver
        .semantic_module()
        .interfaces
        .boundary_machines
        .iter()
        .find(|row| {
            receiver.semantic_module().identities.text(row.identity)
                == module.identities.text(export.identity)
        })
        .unwrap()
        .id;
    let service = admit_component_provider(
        receiver.view(),
        slot,
        producer.view(),
        &ProviderInstallationSelection {
            boundary: candidate.boundary,
            candidate: candidate.candidate,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .unwrap()
                .into(),
            provider_declaration: module
                .identities
                .text(candidate.provider_declaration.identity())
                .unwrap()
                .into(),
        },
    )
    .unwrap();
    let component_program = TerminalProgram::borrow(producer.view()).unwrap();
    let gate = component_program
        .imports()
        .find(|row| row.id != candidate.boundary)
        .unwrap();
    let gate_id = gate.id;
    let gate_signature = TerminalImportSignature::from(gate);
    let component_supplies = component_program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            installation: Some(service.selection()),
            entry: Some(terminal_psi::TargetMachineCallable::Machine(
                service.candidate(),
            )),
            ..host_request(&[gate_id])
        })
        .unwrap();
    let caller_supplies = caller_program
        .admit_runtime_supplies(host_request(&[]))
        .unwrap()
        .with_component_provider(&service)
        .unwrap();
    let mut component =
        TerminalComponentBinding::new(service, component_program, component_supplies, None)
            .unwrap();
    component
        .bind_import(
            gate_id,
            Gate {
                signature: gate_signature,
                provider,
            },
        )
        .unwrap();
    let mut caller = caller_program
        .instantiate(TerminalInstantiation {
            installation: Some(&caller_supplies),
            account: Some(account),
            ..Default::default()
        })
        .unwrap();
    caller.bind_component(component).unwrap();
    let status = caller
        .invoke(&mut terminal_work::WorkMeter::unbounded())
        .unwrap();
    let TerminalExecutionStatus::AwaitingEffect(wait) = status else {
        panic!("the caller must park inside its component: {status:?}")
    };
    assert_eq!(
        wait.program,
        terminal_codec::TerminalExecutionIdentity::portable(producer.view().identity())
    );
    assert_eq!(wait.boundary, gate_id);
    caller
}

#[test]
fn discarding_a_caller_settles_its_parked_component_without_resuming_either_guest() {
    let receiver = checked_provider_closure(SOURCE, "caller", None)
        .publish()
        .unwrap()
        .artifact;
    let producer = checked_provider_closure(SOURCE, "producer_entry", Some(("Service", "Native")))
        .publish()
        .unwrap()
        .artifact;
    assert_ne!(receiver.view().identity(), producer.view().identity());
    let account = account();
    let mut meter = terminal_work::WorkMeter::unbounded();

    // The same wait can complete normally when the host admits the visit.
    let gate = Rc::new(Provider::default());
    let mut control = park(&receiver, &producer, gate.clone(), &account);
    gate.ready.set(true);
    let completed = control.invoke(&mut meter).unwrap();
    assert!(
        matches!(
            completed,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(3),
                    ..
                }
            ))
        ),
        "{completed:?}"
    );
    assert_eq!(gate.handled.get(), 1);
    control.close().unwrap();

    let provider = Rc::new(Provider::default());
    let mut caller = park(&receiver, &producer, provider.clone(), &account);
    provider.retained_bytes.set(4096);
    assert_eq!(caller.discard(), outstanding(4096));
    assert_eq!(caller.close(), Err(TerminalInstanceCloseError::Busy));
    assert_eq!(provider.cancellations.get(), 1);
    assert!(
        account
            .reserve_storage(0, LIVE_PAYLOAD_CEILING - 4096 + 1)
            .is_err()
    );
    assert_eq!(
        caller.invoke(&mut meter),
        Err(TerminalInstanceError::Discarded)
    );

    provider.ready.set(true);
    assert_eq!(caller.discard(), TerminalDiscardStatus::Settled);
    assert_eq!(caller.close(), Ok(()));
    assert!(account.reserve_storage(0, LIVE_PAYLOAD_CEILING).is_ok());
    assert_eq!(provider.handled.get(), 0);
    assert!(caller.result().is_none());
}
