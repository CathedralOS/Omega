use psi_core::{EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId};
use psi_terminal::{Operation, OperationKind, Terminator, ValueDeclaration};
use psi_terminal_fuel::{
    FuelChargeSite, FuelExhaustion, FuelMeterError, FuelScheduleIdentity, TerminalFuelMeter,
    TerminalFuelSchedule,
};

#[test]
fn schedule_identity_is_nonzero_and_independent() {
    assert_eq!(FuelScheduleIdentity::new(0), None);
    assert_eq!(
        TerminalFuelSchedule::CURRENT.identity().schedule_version(),
        1
    );
    assert_ne!(
        FuelScheduleIdentity::new(2).unwrap(),
        TerminalFuelSchedule::CURRENT.identity()
    );
}

#[test]
fn current_vocabulary_has_explicit_v1_costs_and_attribution() {
    assert_eq!(
        TerminalFuelSchedule::V1.operation_units(&OperationKind::BooleanConstant { value: true }),
        1,
        "adding the v2 Boolean operation must not leave its v1-schedule cost implicit"
    );
    assert_eq!(
        TerminalFuelSchedule::V1.operation_units(&OperationKind::BooleanNot {
            operand: value_id(1),
        }),
        1,
        "Boolean logical not has one explicit v1-schedule unit"
    );
    assert_eq!(
        TerminalFuelSchedule::V1.operation_units(&OperationKind::BooleanEqual {
            left: value_id(1),
            right: value_id(2),
        }),
        1,
        "Boolean equality has one explicit v1-schedule unit"
    );
    let operation = operation();
    let jump = Terminator::Jump {
        edge: edge_id(1),
        target: psi_core::BlockId::new(2).unwrap(),
        arguments: vec![value_id(1)],
    };
    let return_edge = Terminator::Return {
        edge: edge_id(2),
        value: value_id(1),
    };
    let mut meter = TerminalFuelMeter::unbounded();

    meter.charge_operation(&operation).unwrap();
    meter.charge_terminator(&jump).unwrap();
    meter.charge_operation(&operation).unwrap();
    meter.charge_terminator(&return_edge).unwrap();

    assert_eq!(meter.usage().total_units(), 4);
    let operation_usage = meter
        .usage()
        .at(FuelChargeSite::Operation(operation_id(1)))
        .unwrap();
    assert_eq!(operation_usage.executions(), 2);
    assert_eq!(operation_usage.units(), 2);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(1)))
            .unwrap()
            .units(),
        1
    );
    assert_eq!(meter.usage().attribution().len(), 3);
}

#[test]
fn sponsor_allowance_exhausts_atomically_before_execution() {
    let operation = operation();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    meter.charge_operation(&operation).unwrap();
    let usage_before = meter.usage().clone();

    assert_eq!(
        meter.charge_operation(&operation),
        Err(FuelMeterError::Exhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(1)),
            required_units: 1,
            remaining_units: 0,
        }))
    );
    assert_eq!(meter.remaining_allowance(), Some(0));
    assert_eq!(meter.usage(), &usage_before);

    meter.replenish(1).unwrap();
    meter.charge_operation(&operation).unwrap();
    assert_eq!(meter.remaining_allowance(), Some(0));
    assert_eq!(meter.usage().total_units(), 2);
}

#[test]
fn allowance_replenishment_fails_closed_on_overflow() {
    let mut meter = TerminalFuelMeter::with_allowance(u64::MAX);
    assert_eq!(meter.replenish(1), Err(FuelMeterError::AllowanceOverflow));
    assert_eq!(meter.remaining_allowance(), Some(u64::MAX));
    assert_eq!(meter.usage().total_units(), 0);
}

fn operation() -> Operation {
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).expect("valid test integer type"),
    );
    Operation {
        id: operation_id(1),
        result: ValueDeclaration {
            id: value_id(1),
            scalar_type,
        },
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Signed(7),
        },
    }
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
