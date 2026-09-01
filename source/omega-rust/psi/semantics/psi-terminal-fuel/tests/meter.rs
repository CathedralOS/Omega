use psi_core::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, ServiceId, ValueId,
};
use psi_terminal::{
    CrashCause, Operation, OperationKind, TerminalAffineCleanupAction, Terminator, ValueDeclaration,
};
use psi_terminal_fuel::{
    FuelChargeSite, FuelExhaustion, FuelMeterError, FuelScheduleIdentity, TerminalFuelMeter,
    TerminalFuelSchedule,
};

#[test]
fn schedule_identity_is_nonzero_and_independent() {
    assert_eq!(FuelScheduleIdentity::new(0), None);
    assert_eq!(TerminalFuelSchedule::CURRENT.identity().marker(), 1);
    assert_ne!(
        FuelScheduleIdentity::new(2).unwrap(),
        TerminalFuelSchedule::CURRENT.identity()
    );
}

#[test]
fn current_vocabulary_has_explicit_costs_and_attribution() {
    assert_eq!(
        TerminalFuelSchedule::CURRENT
            .operation_units(&OperationKind::BooleanConstant { value: true }),
        1,
        "every current Boolean operation has an explicit schedule cost"
    );
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&OperationKind::BooleanNot {
            operand: value_id(1),
        }),
        1,
        "Boolean logical not has one explicit schedule unit"
    );
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&OperationKind::BooleanEqual {
            left: value_id(1),
            right: value_id(2),
        }),
        1,
        "Boolean equality has one explicit schedule unit"
    );
    for kind in [
        OperationKind::IntegerLessThan {
            left: value_id(1),
            right: value_id(2),
        },
        OperationKind::IntegerLessOrEqual {
            left: value_id(1),
            right: value_id(2),
        },
        OperationKind::IntegerBitwiseAnd {
            left: value_id(1),
            right: value_id(2),
        },
        OperationKind::IntegerBitwiseOr {
            left: value_id(1),
            right: value_id(2),
        },
        OperationKind::IntegerBitwiseXor {
            left: value_id(1),
            right: value_id(2),
        },
        OperationKind::WrappingIntegerShiftLeft {
            value: value_id(1),
            count: value_id(2),
        },
        OperationKind::WrappingIntegerShiftRight {
            value: value_id(1),
            count: value_id(2),
        },
    ] {
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&kind),
            1,
            "each integer comparison, bitwise operation, or wrapping shift has one explicit schedule unit"
        );
    }
    for kind in [
        OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(1),
            value: value_id(1),
        },
        OperationKind::CallUnit {
            callee: MachineId::new(1).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
        OperationKind::BoundaryCall {
            boundary: BoundaryMachineId::new(1).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
        OperationKind::PortWrite {
            service: ServiceId::new(1).unwrap(),
            port: 0x3f8,
            value: 0x5a,
        },
    ] {
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&kind),
            1,
            "every represented operation, including a primitive store, has the schedule's uniform one-unit cost"
        );
    }
    let operation = operation();
    let jump = Terminator::Jump {
        edge: edge_id(1),
        target: psi_core::BlockId::new(2).unwrap(),
        arguments: vec![value_id(1)],
        trivial_affine_discards: vec![place_id(1)],
    };
    let return_edge = Terminator::Return {
        edge: edge_id(2),
        value: value_id(1),
        cleanup_actions: Vec::new(),
    };
    let unit_return_edge = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    let scalar_cleanup_edge = Terminator::Return {
        edge: edge_id(5),
        value: value_id(1),
        cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(1))],
    };
    let structural_return_edge = Terminator::ReturnStructural {
        edge: edge_id(6),
        source: place_id(1),
        returned_claims: vec![psi_core::ClaimId::new(1).unwrap()],
        trivial_affine_discards: Vec::new(),
    };
    let crash_edge = Terminator::Crash {
        edge: edge_id(3),
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.terminator_units(&crash_edge),
        1,
        "an explicit crash exit has one schedule unit"
    );
    assert_eq!(
        TerminalFuelSchedule::CURRENT.terminator_units(&unit_return_edge),
        1,
        "a value-less normal return has one explicit edge unit"
    );
    assert_eq!(
        TerminalFuelSchedule::CURRENT.terminator_units(&structural_return_edge),
        1,
        "a structural ownership transfer is one explicit edge unit"
    );
    let mut structural_meter = TerminalFuelMeter::with_allowance(1);
    structural_meter
        .charge_terminator(&structural_return_edge)
        .unwrap();
    assert_eq!(
        structural_meter
            .usage()
            .at(FuelChargeSite::Edge(edge_id(6)))
            .unwrap()
            .units(),
        1
    );
    assert_eq!(
        TerminalFuelSchedule::CURRENT.terminator_units(&scalar_cleanup_edge),
        1,
        "a scalar return with no-code affine cleanup has one explicit edge unit"
    );
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
        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
            id: value_id(1),
            scalar_type,
        }),
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

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}
