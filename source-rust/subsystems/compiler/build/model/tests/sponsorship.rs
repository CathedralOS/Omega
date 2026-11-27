use build_model::work::sponsored_meter;
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::{Operation, OperationKind, Terminator, ValueDeclaration};
use terminal_work::{WorkChargeSite, WorkMeterError, WorkSchedule};

fn operation() -> Operation {
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).expect("valid test integer type"),
    );
    Operation {
        static_reach_binding: None,
        required_contract: None,
        suspension_crossing: None,
        id: operation_id(1),
        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Signed(7),
        },
    }
}

fn terminal_sponsor(units: u64) -> build_model::accounting::BuildEvaluationSponsor {
    use build_model::accounting::BuildEvaluationSponsor;
    use build_model::contracts::{
        BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(units, 8, 2, 2, 8, 8, 8, 8).unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap()
}

#[test]
fn invocation_payment_is_atomic_with_local_attribution_and_shared_between_meters() {
    use build_model::contracts::{EvaluationAccountingError, EvaluationResource};
    let sponsor = terminal_sponsor(2);
    let invocation = sponsor.begin_invocation(sponsor.measurement(), 3).unwrap();
    let mut meter = sponsored_meter(invocation.clone(), Some(0)).unwrap();
    let operation = operation();
    assert!(matches!(
        meter.charge_operation(&operation),
        Err(WorkMeterError::Exhausted(_))
    ));
    assert_eq!(sponsor.consumed_work_units(), 0);
    meter.replenish(10).unwrap();
    meter.charge_operation(&operation).unwrap();
    let mut alias = meter.clone();
    alias.charge_operation(&operation).unwrap();
    assert_eq!(sponsor.consumed_work_units(), 2);
    let before = meter.usage().clone();
    let budget = meter.work_remaining();
    assert_eq!(
        meter.charge_operation(&operation),
        Err(WorkMeterError::PaymentRefused {
            site: WorkChargeSite::Operation(operation.id),
            reason: EvaluationAccountingError::BudgetExhausted(EvaluationResource::Work)
                .to_string(),
        })
    );
    assert_eq!(meter.usage(), &before);
    assert_eq!(meter.work_remaining(), budget);
    assert_eq!(invocation.snapshot().unwrap().record().work_units, 2);
    let second = sponsor.begin_invocation(sponsor.measurement(), 3).unwrap();
    let mut independent = sponsored_meter(second.clone(), None).unwrap();
    assert!(matches!(
        independent.charge_operation(&operation),
        Err(WorkMeterError::PaymentRefused { .. })
    ));
    assert_eq!(second.snapshot().unwrap().record().work_units, 0);
    assert_eq!(independent.usage().total_units(), 0);
}

#[test]
fn invocation_ceiling_cannot_be_changed_by_the_meter() {
    let sponsor = terminal_sponsor(8);
    let invocation = sponsor.begin_invocation(sponsor.measurement(), 1).unwrap();
    let mut meter = sponsored_meter(invocation.clone(), None).unwrap();
    meter.charge_operation(&operation()).unwrap();
    meter.replenish(100).unwrap();
    let edge = Terminator::ReturnUnit {
        edge: edge_id(3),
        cleanup_actions: Vec::new(),
    };
    assert!(matches!(
        meter.charge_terminator(&edge),
        Err(WorkMeterError::PaymentRefused {
            site: WorkChargeSite::Edge(_),
            ..
        })
    ));
    assert_eq!(invocation.snapshot().unwrap().record().work_units, 1);
    assert_eq!(meter.usage().total_units(), 1);
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("test operation identity is nonzero")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("test edge identity is nonzero")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("test value identity is nonzero")
}
