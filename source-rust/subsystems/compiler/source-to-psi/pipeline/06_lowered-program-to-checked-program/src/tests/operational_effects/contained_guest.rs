//! product.md, "Publication and separate consumption": an untrusted artifact
//! is interpreted only through a containment-audited interpreter and host
//! bindings. A guest admitted without producer trust needs an invocation
//! account for its storage and binds only adapters that serve contained
//! execution.

use super::{boundary_calls, elaborate, judge};
use crate::RouteVerdict;
use build_model::accounting::BuildEvaluationSponsor;
use build_model::contracts::{
    BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use std::sync::Arc;
use terminal_interpreter::{
    TerminalBindingRejection, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionStatus, TerminalImportAdapter, TerminalImportSignature,
    TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplyRequest,
};

const CALLS_HOST: &str = r"
boundary trait Gate { machine visit() reaches none crashes none; }
machine main() -> u8 reaches Gate invokes Gate {
    Gate::visit();
    3
}";

struct Gate {
    signature: TerminalImportSignature,
    contained: bool,
}
impl TerminalImportAdapter for Gate {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
    fn serves_contained_execution(&self) -> bool {
        self.contained
    }
}
impl TerminalEffectHandler for Gate {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }
}

#[test]
fn a_contained_guest_needs_an_account_and_contained_adapters() {
    let portable = elaborate(
        boundary_calls::typed(CALLS_HOST),
        terminal_codec::TerminalPublication::Portable,
        &[],
    );
    let RouteVerdict::Accepted(accepted) = judge(portable).unwrap() else {
        panic!("the portable program is accepted")
    };
    let produced = accepted.publish().unwrap().artifact;
    let received = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&produced.to_bytes()).unwrap(),
    );
    let unadmitted = Arc::clone(&received);
    let contained = psi_authorization::admit_contained_interpretation(received).unwrap();
    // The admitted copy still requires producer trust on every trusted route.
    assert!(TerminalProgram::borrow(contained.artifact().view()).is_err());
    // Compile-time evaluation borrows it only as a contained program, and a
    // received copy without that admission not at all.
    assert!(
        TerminalProgram::borrow_admitted(contained.artifact().view())
            .unwrap()
            .is_contained()
    );
    assert!(TerminalProgram::borrow_admitted(unadmitted.view()).is_err());

    let program = TerminalProgram::admit_contained(&contained);
    assert!(program.is_contained());
    let imports: Vec<_> = program.imports().collect();
    let [gate] = imports.as_slice() else {
        panic!("one callable host slot expected")
    };
    let supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
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
    assert!(
        program
            .instantiate(TerminalInstantiation {
                installation: Some(&supplies),
                ..Default::default()
            })
            .is_err(),
        "contained storage must be charged to an invocation account"
    );

    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(
            1_000_000, 1, 1, 1, 100_000, 1_000_000, 100_000, 1_000_000,
        )
        .unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 1_000_000)
        .unwrap();
    let mut instance = program
        .instantiate(TerminalInstantiation {
            installation: Some(&supplies),
            account: Some(&account),
            ..Default::default()
        })
        .unwrap();
    let signature = TerminalImportSignature::from(*gate);
    assert_eq!(
        instance.bind_import(
            gate.id,
            Gate {
                signature: signature.clone(),
                contained: false,
            },
        ),
        Err(TerminalBindingRejection::UncontainedAdapter { boundary: gate.id })
    );
    instance
        .bind_import(
            gate.id,
            Gate {
                signature,
                contained: true,
            },
        )
        .unwrap();
    assert!(matches!(
        instance.invoke(&mut terminal_work::WorkMeter::with_budget(10_000)),
        Ok(TerminalExecutionStatus::Complete(_))
    ));
}
