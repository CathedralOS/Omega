use super::{
    Block, CodecError, CrashCause, CrashRouteBucket, CrashRouteGuard, IntegerValue,
    MachineContract, Operation, OperationKind, OperationResult, Proposition, ScalarTerm,
    ScalarType, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, block_id,
    contract_id, decode_module, edge_id, encode_module, i32_type, machine_id, operation_id,
    semantic_fingerprint, unit_fixture, value_id,
};
use terminal_psi::{CrashPredicateTerm, TerminalOperationCrashContract};

fn negative(value: u64) -> Proposition {
    Proposition::LessThan(
        ScalarTerm::value(value_id(value), ScalarType::Integer(i32_type())),
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
    )
}

fn guarded(cause: CrashCause, proposition: Proposition) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            proposition,
        ))],
    }
}

fn declaration(raw: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(raw),
        scalar_type,
    }
}

fn row() -> TerminalOperationCrashContract {
    TerminalOperationCrashContract {
        machine: machine_id(900),
        operation: operation_id(901),
        published_routes: vec![guarded(CrashCause::Trap, negative(2))],
        crash_continuations: vec![guarded(CrashCause::Trap, negative(920))],
    }
}

/// The Unit fixture's machine becomes `compare(left: i32, right: i32) -> bool
/// { left == right }` with the equality's `crashes Trap right < 0` contract
/// carried at the operation and published over the caller's `right`.
fn comparison_fixture() -> TerminalModule {
    let integer_type = ScalarType::Integer(i32_type());
    let mut module = unit_fixture();
    let machine = &mut module.machines[0];
    machine.parameters = vec![
        declaration(910, integer_type),
        declaration(920, integer_type),
    ];
    machine.result = TerminalMachineResult::Scalar(declaration(940, ScalarType::Boolean));
    machine.blocks = vec![Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block_id(900),
        parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation_id(901),
            result: OperationResult::Scalar(declaration(930, ScalarType::Boolean)),
            kind: OperationKind::IntegerEqual {
                left: value_id(910),
                right: value_id(920),
            },
        }],
        terminator: Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: edge_id(900),
            value: value_id(930),
        },
    }];
    machine.contract = MachineContract {
        erased_scalar_formals: Vec::new(),
        id: contract_id(900),
        crash_routes: vec![guarded(CrashCause::Trap, negative(920))],
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    };
    module.operation_crash_contracts = vec![row()];
    module
}

#[test]
fn operation_crash_contracts_round_trip_and_enter_semantic_identity() {
    let module = comparison_fixture();
    let bytes = encode_module(&module).expect("operation crash contract encodes");
    assert_eq!(&bytes[8..12], &[100, 0, 107, 0]);
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let mut without_row = module.clone();
    without_row.operation_crash_contracts.clear();
    assert_ne!(
        semantic_fingerprint(&without_row).unwrap(),
        semantic_fingerprint(&module).unwrap()
    );
    let mut abort = module.clone();
    abort.operation_crash_contracts[0].published_routes =
        vec![guarded(CrashCause::Abort, negative(2))];
    abort.operation_crash_contracts[0].crash_continuations =
        vec![guarded(CrashCause::Abort, negative(920))];
    abort.machines[0].contract.crash_routes = vec![guarded(CrashCause::Abort, negative(920))];
    assert_ne!(
        semantic_fingerprint(&abort).unwrap(),
        semantic_fingerprint(&module).unwrap()
    );
}

/// The roster is a required section of format 99. Bytes laid out without it
/// (the format 98 layout) must not decode as a current module with zero rows,
/// and the previous marker rejects before the body is read.
#[test]
fn modules_without_the_roster_section_reject() {
    let mut without_row = comparison_fixture();
    without_row.operation_crash_contracts.clear();
    without_row.machines[0].contract.crash_routes.clear();
    let current = encode_module(&without_row).expect("roster-free module encodes");
    let with_row = encode_module(&comparison_fixture()).expect("one-row module encodes");
    // The roster count is the first byte that differs between the two
    // encodings; the previous layout has no such section at all.
    let roster_offset = current
        .iter()
        .zip(&with_row)
        .position(|(left, right)| left != right)
        .expect("the roster changes the encoding");
    assert_eq!(&current[roster_offset..roster_offset + 4], &[0, 0, 0, 0]);
    let mut previous_layout = current.clone();
    previous_layout.drain(roster_offset..roster_offset + 4);
    assert_ne!(previous_layout, current);
    let decoded = decode_module(&previous_layout);
    assert!(
        decoded.is_err() || decoded != Ok(without_row.clone()),
        "previous-layout bytes decoded cleanly as a current module: {decoded:?}"
    );

    let mut previous_marker = current;
    previous_marker[8..10].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        decode_module(&previous_marker),
        Err(CodecError::UnsupportedFormatMarker(99))
    );
}

#[test]
fn operation_crash_contract_rows_must_be_canonical() {
    let mut reordered = comparison_fixture();
    let duplicate = reordered.operation_crash_contracts[0].clone();
    reordered.operation_crash_contracts.push(duplicate);
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "operation crash contracts by machine and operation"
        ))
    );
    let mut empty = comparison_fixture();
    empty.operation_crash_contracts[0].published_routes.clear();
    assert_eq!(
        encode_module(&empty),
        Err(CodecError::NonCanonicalOrder(
            "operation crash contract published route buckets"
        ))
    );
    let mut misordered = comparison_fixture();
    misordered.operation_crash_contracts[0].published_routes = vec![
        guarded(CrashCause::Abort, negative(2)),
        guarded(CrashCause::Trap, negative(2)),
    ];
    assert_eq!(
        encode_module(&misordered),
        Err(CodecError::NonCanonicalOrder(
            "operation crash contract published route buckets"
        ))
    );
    let mut widened = comparison_fixture();
    widened.operation_crash_contracts[0].crash_continuations = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![
            CrashRouteGuard::Truth,
            CrashRouteGuard::Predicate(CrashPredicateTerm::new(negative(920))),
        ],
    }];
    assert_eq!(
        encode_module(&widened),
        Err(CodecError::NonCanonicalOrder(
            "operation crash contract continuation buckets"
        ))
    );
}
