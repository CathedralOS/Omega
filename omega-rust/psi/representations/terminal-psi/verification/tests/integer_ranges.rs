//! Retained authored integer entry ranges: the verifier admits only rows whose
//! owner publishes the same inclusive bounds as `requires` propositions, and
//! rejects an exact `IntegerConstant` delivery outside the authored interval.
//! Forwarded and computed arguments discharge through call-composition proof
//! replay, so the delivery check below only names the constant shape it can
//! settle eagerly.
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ScalarIntegerRange,
    ScalarQualificationCatalog, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, validate_module};

/// Caller produces one `u64` constant and delivers it to a callee whose single
/// parameter carries `range`.
fn constant_delivery_module(range: ScalarIntegerRange, argument: u128) -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = range.parameter;
    let callee_result = value_id(5);
    integer_module(
        vec![],
        vec![
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation_id(1),
                result: OperationResult::Scalar(declaration(caller_constant)),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(argument),
                },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation_id(2),
                result: OperationResult::Scalar(declaration(call_result)),
                kind: OperationKind::Call {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    callee: range.machine,
                    arguments: vec![caller_constant],
                    // The callee publishes one merged `requires` proposition,
                    // so the call owes exactly one requirement obligation.
                    requirement_obligations: vec![obligation_id(1)],
                    crash_continuations: Vec::new(),
                },
            },
        ],
        call_result,
        caller_result,
        vec![range],
        callee_parameter,
        callee_result,
        range_requires(range),
    )
}

/// The owner contract must publish both inclusive bounds of `range` as
/// `LTE(minimum, parameter)` and `LTE(parameter, maximum)` conjuncts.
fn range_requires(range: ScalarIntegerRange) -> Vec<Proposition> {
    let carrier = ScalarType::Integer(range.integer_type);
    let subject = ScalarTerm::value(range.parameter, carrier);
    let minimum = ScalarTerm::integer(range.integer_type, range.minimum).unwrap();
    let maximum = ScalarTerm::integer(range.integer_type, range.maximum).unwrap();
    vec![Proposition::Conjunction(vec![
        Proposition::LessOrEqual(minimum, subject.clone()),
        Proposition::LessOrEqual(subject, maximum),
    ])]
}

#[allow(clippy::too_many_arguments)]
fn integer_module(
    caller_parameters: Vec<ValueDeclaration>,
    caller_operations: Vec<Operation>,
    return_value: ValueId,
    caller_result: ValueId,
    integer_entry_ranges: Vec<ScalarIntegerRange>,
    callee_parameter: ValueId,
    callee_result: ValueId,
    callee_requires: Vec<Proposition>,
) -> TerminalModule {
    TerminalModule {
        scalar_qualifications: ScalarQualificationCatalog {
            integer_entry_ranges,
            ..Default::default()
        },
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: caller_parameters,
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(caller_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: caller_operations,
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: return_value,
                    },
                }],
                contract: empty_contract(1),
            },
            TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![declaration(callee_parameter)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(callee_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: callee_parameter,
                    },
                }],
                contract: MachineContract {
                    requires: callee_requires,
                    ..empty_contract(2)
                },
            },
        ],
    }
}

fn range(minimum: u128, maximum: u128) -> ScalarIntegerRange {
    ScalarIntegerRange {
        machine: machine_id(2),
        parameter: value_id(4),
        integer_type: u64_integer(),
        minimum: IntegerValue::Unsigned(minimum),
        maximum: IntegerValue::Unsigned(maximum),
    }
}

fn validates(module: &TerminalModule) {
    validate_module(module).expect("range-conforming module must validate");
}

fn delivery_rejects(module: &TerminalModule) {
    assert!(matches!(
        validate_module(module),
        Err(ModuleError::ScalarIntegerRangeDelivery {
            caller,
            callee,
            ..
        }) if caller == machine_id(1) && callee == machine_id(2)
    ));
}

#[test]
fn constant_inside_inclusive_range_delivers() {
    validates(&constant_delivery_module(range(0, 3), 2));
    // Both inclusive endpoints admit their own boundary constants.
    validates(&constant_delivery_module(range(0, 3), 0));
    validates(&constant_delivery_module(range(0, 3), 3));
}

#[test]
fn constant_above_maximum_rejects() {
    delivery_rejects(&constant_delivery_module(range(0, 3), 4));
}

#[test]
fn constant_below_minimum_rejects() {
    delivery_rejects(&constant_delivery_module(range(1, 3), 0));
}

#[test]
fn forwarded_parameter_is_not_eagerly_rejected() {
    // A forwarded caller parameter is not an `IntegerConstant`, so the eager
    // delivery check leaves it to call-composition proof replay. Validation
    // itself must not reject it as a delivery fault.
    let caller_parameter = value_id(9);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_range = range(0, 3);
    let module = integer_module(
        vec![declaration(caller_parameter)],
        vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(2),
            result: OperationResult::Scalar(declaration(call_result)),
            kind: OperationKind::Call {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: callee_range.machine,
                arguments: vec![caller_parameter],
                requirement_obligations: vec![obligation_id(1)],
                crash_continuations: Vec::new(),
            },
        }],
        call_result,
        caller_result,
        vec![callee_range],
        callee_range.parameter,
        value_id(5),
        range_requires(callee_range),
    );
    validates(&module);
}

#[test]
fn roster_rejects_malformed_rows() {
    let conforming = range(0, 3);
    // Unknown owner machine.
    let mut unknown_owner = constant_delivery_module(conforming, 2);
    unknown_owner.scalar_qualifications.integer_entry_ranges[0].machine = machine_id(9);
    assert!(matches!(
        validate_module(&unknown_owner),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Row attached to a value that is not a direct scalar parameter.
    let mut not_parameter = constant_delivery_module(conforming, 2);
    not_parameter.scalar_qualifications.integer_entry_ranges[0].parameter = value_id(5);
    assert!(matches!(
        validate_module(&not_parameter),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Reversed interval: minimum above maximum.
    let mut unordered = constant_delivery_module(conforming, 2);
    unordered.scalar_qualifications.integer_entry_ranges[0].minimum = IntegerValue::Unsigned(4);
    assert!(matches!(
        validate_module(&unordered),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Carrier narrower than the declared parameter type.
    let mut mismatched_carrier = constant_delivery_module(conforming, 2);
    mismatched_carrier
        .scalar_qualifications
        .integer_entry_ranges[0]
        .integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert!(matches!(
        validate_module(&mismatched_carrier),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // An address carrier is not an entry-range carrier.
    let mut address_carrier = constant_delivery_module(conforming, 2);
    address_carrier.scalar_qualifications.integer_entry_ranges[0].integer_type =
        IntegerType::address(64).unwrap();
    assert!(matches!(
        validate_module(&address_carrier),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Endpoint the carrier does not admit (a signed value on an unsigned type).
    let mut bad_endpoint = constant_delivery_module(conforming, 2);
    bad_endpoint.scalar_qualifications.integer_entry_ranges[0].maximum = IntegerValue::Signed(-1);
    assert!(matches!(
        validate_module(&bad_endpoint),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Owner that publishes a `requires` proposition other than the retained
    // bounds fails closed. Keep one bare proposition so call arity still
    // matches — the failure must be the missing bound conjuncts, not an arity
    // fault. A single `Conjunction` would itself be noncanonical.
    let carrier = ScalarType::Integer(u64_integer());
    let subject = ScalarTerm::value(value_id(4), carrier);
    let mut unpublished = constant_delivery_module(conforming, 2);
    unpublished.machines[1].contract.requires =
        vec![Proposition::LessOrEqual(subject.clone(), subject.clone())];
    let unpublished_error = validate_module(&unpublished).unwrap_err();
    assert!(
        matches!(
            unpublished_error,
            ModuleError::InvalidScalarIntegerRange { .. }
        ),
        "unpublished bounds must fail as an integer range fault: {unpublished_error:?}"
    );
    // Owner publishing only one bound conjunct fails closed: the low bound is
    // present as a bare proposition but the high bound is absent.
    let mut half_published = constant_delivery_module(conforming, 2);
    half_published.machines[1].contract.requires = vec![Proposition::LessOrEqual(
        ScalarTerm::integer(u64_integer(), IntegerValue::Unsigned(0)).unwrap(),
        subject,
    )];
    assert!(matches!(
        validate_module(&half_published),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Duplicate row for the same (machine, parameter) is noncanonical.
    let mut duplicated = constant_delivery_module(conforming, 2);
    duplicated
        .scalar_qualifications
        .integer_entry_ranges
        .push(conforming);
    assert!(matches!(
        validate_module(&duplicated),
        Err(ModuleError::InvalidScalarIntegerRange { .. })
    ));
    // Reversed (machine, parameter) order is noncanonical. Give the callee a
    // second ranged parameter, deliver a second argument, and publish both
    // parameters' bound conjuncts so only the roster ordering is at fault.
    let mut reversed = constant_delivery_module(conforming, 2);
    let mut second = conforming;
    second.parameter = value_id(6);
    reversed.scalar_qualifications.integer_entry_ranges = vec![second, conforming];
    reversed.machines[1]
        .parameters
        .push(declaration(value_id(6)));
    let OperationKind::Call { arguments, .. } =
        &mut reversed.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!("the caller's second operation is the call")
    };
    arguments.push(value_id(1));
    let bound = |parameter: ValueId| {
        vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(u64_integer(), IntegerValue::Unsigned(0)).unwrap(),
                ScalarTerm::value(parameter, carrier),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::value(parameter, carrier),
                ScalarTerm::integer(u64_integer(), IntegerValue::Unsigned(3)).unwrap(),
            ),
        ]
    };
    reversed.machines[1].contract.requires = vec![Proposition::Conjunction(
        [bound(value_id(4)), bound(value_id(6))].concat(),
    )];
    let reversed_error = validate_module(&reversed).unwrap_err();
    assert!(
        matches!(
            reversed_error,
            ModuleError::InvalidScalarIntegerRange { .. }
        ),
        "reversed roster order must fail as an integer range fault: {reversed_error:?}"
    );
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: ContractId::new(raw).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn declaration(id: ValueId) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type: ScalarType::Integer(u64_integer()),
    }
}

fn u64_integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}
