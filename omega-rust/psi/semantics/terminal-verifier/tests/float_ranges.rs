//! Retained authored floating entry ranges: the verifier admits only
//! deliveries that are provably inside the authored IEEE range — a constant
//! the range contains, or a caller parameter whose own range is subsumed.
//! The exact endpoint, NaN, and every other unprovable delivery reject.
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IeeeFloatValue, MachineId, OperationId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, ScalarFloatRange,
    ScalarQualificationCatalog, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, validate_module, verify_module};

/// Caller produces one `f64` constant and delivers it to a callee whose
/// single parameter carries `range`.
fn constant_delivery_module(range: ScalarFloatRange, argument_bits: u64) -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = range.parameter;
    let callee_result = value_id(5);
    float_module(
        vec![],
        vec![
            Operation {
                static_reach_binding: None,
                id: operation_id(1),
                result: OperationResult::Scalar(declaration(caller_constant, f64())),
                kind: OperationKind::IeeeFloatConstant {
                    value: IeeeFloatValue::Binary64(argument_bits),
                },
            },
            Operation {
                static_reach_binding: None,
                id: operation_id(2),
                result: OperationResult::Scalar(declaration(call_result, f64())),
                kind: OperationKind::Call {
                    erased_arguments: Vec::new(),
                    callee: range.machine,
                    arguments: vec![caller_constant],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        ],
        call_result,
        caller_result,
        vec![range],
        callee_parameter,
        callee_result,
    )
}

/// Caller has its own ranged `f64` parameter and forwards it to the callee.
fn forwarding_module(
    caller_range: ScalarFloatRange,
    callee_range: ScalarFloatRange,
) -> TerminalModule {
    let caller_parameter = caller_range.parameter;
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = callee_range.parameter;
    let callee_result = value_id(5);
    float_module(
        vec![declaration(caller_parameter, f64())],
        vec![Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: OperationResult::Scalar(declaration(call_result, f64())),
            kind: OperationKind::Call {
                erased_arguments: Vec::new(),
                callee: callee_range.machine,
                arguments: vec![caller_parameter],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }],
        call_result,
        caller_result,
        vec![caller_range, callee_range],
        callee_parameter,
        callee_result,
    )
}

fn float_module(
    caller_parameters: Vec<ValueDeclaration>,
    caller_operations: Vec<Operation>,
    return_value: ValueId,
    caller_result: ValueId,
    float_entry_ranges: Vec<ScalarFloatRange>,
    callee_parameter: ValueId,
    callee_result: ValueId,
) -> TerminalModule {
    TerminalModule {
        scalar_qualifications: ScalarQualificationCatalog {
            float_entry_ranges,
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
                result: TerminalMachineResult::Scalar(declaration(caller_result, f64())),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
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
                parameters: vec![declaration(callee_parameter, f64())],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(declaration(callee_result, f64())),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
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
                contract: empty_contract(2),
            },
        ],
    }
}

fn range(minimum: f64, maximum: f64, maximum_inclusive: bool) -> ScalarFloatRange {
    ScalarFloatRange {
        machine: machine_id(2),
        parameter: value_id(4),
        minimum: IeeeFloatValue::Binary64(minimum.to_bits()),
        maximum: IeeeFloatValue::Binary64(maximum.to_bits()),
        maximum_inclusive,
    }
}

fn caller_range(minimum: f64, maximum: f64, maximum_inclusive: bool) -> ScalarFloatRange {
    ScalarFloatRange {
        machine: machine_id(1),
        parameter: value_id(9),
        minimum: IeeeFloatValue::Binary64(minimum.to_bits()),
        maximum: IeeeFloatValue::Binary64(maximum.to_bits()),
        maximum_inclusive,
    }
}

fn verifies(module: &TerminalModule) {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("range-conforming delivery must verify");
}

fn delivery_rejects(module: &TerminalModule) {
    let error = verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect_err("out-of-range delivery must reject");
    assert!(matches!(
        error,
        terminal_verifier::VerificationError::Module(ModuleError::ScalarFloatRangeDelivery {
            caller,
            callee,
            ..
        }) if caller == machine_id(1) && callee == machine_id(2)
    ));
}

#[test]
fn constant_below_exclusive_endpoint_delivers() {
    verifies(&constant_delivery_module(
        range(0.0, 1.5, false),
        1.0f64.to_bits(),
    ));
}

#[test]
fn exact_exclusive_endpoint_rejects() {
    delivery_rejects(&constant_delivery_module(
        range(0.0, 1.5, false),
        1.5f64.to_bits(),
    ));
}

#[test]
fn nan_delivery_rejects() {
    delivery_rejects(&constant_delivery_module(
        range(0.0, 1.5, false),
        f64::NAN.to_bits(),
    ));
}

#[test]
fn delivery_above_endpoint_rejects() {
    delivery_rejects(&constant_delivery_module(
        range(0.0, 1.5, false),
        2.0f64.to_bits(),
    ));
}

#[test]
fn delivery_below_minimum_rejects() {
    delivery_rejects(&constant_delivery_module(
        range(0.0, 1.5, false),
        (-1.0f64).to_bits(),
    ));
}

#[test]
fn inclusive_endpoint_admits_the_endpoint_itself() {
    verifies(&constant_delivery_module(
        range(0.0, 1.5, true),
        1.5f64.to_bits(),
    ));
}

#[test]
fn signed_zero_is_ieee_member_not_bit_member() {
    // [0.0..=0.0] admits -0.0 under IEEE equality even though its bits differ.
    verifies(&constant_delivery_module(
        range(0.0, 0.0, true),
        (-0.0f64).to_bits(),
    ));
    // Under an exclusive endpoint, -0.0 is not below +0.0: IEEE order rejects.
    delivery_rejects(&constant_delivery_module(
        range(-0.0, 0.0, false),
        (-0.0f64).to_bits(),
    ));
}

#[test]
fn infinity_is_rejected_by_finite_exclusive_endpoint() {
    delivery_rejects(&constant_delivery_module(
        range(0.0, f64::INFINITY, false),
        f64::INFINITY.to_bits(),
    ));
}

#[test]
fn caller_parameter_subsumed_range_delivers() {
    verifies(&forwarding_module(
        caller_range(0.0, 1.5, false),
        range(0.0, 1.5, true),
    ));
}

#[test]
fn caller_parameter_wider_range_rejects() {
    // Caller's [0.0..=1.5] can produce the endpoint the callee's [0.0..1.5)
    // excludes.
    delivery_rejects(&forwarding_module(
        caller_range(0.0, 1.5, true),
        range(0.0, 1.5, false),
    ));
}

#[test]
fn unranged_caller_parameter_rejects() {
    // Forwarding a parameter with no retained range cannot prove membership.
    let mut module = forwarding_module(caller_range(0.0, 1.5, false), range(0.0, 1.5, true));
    module.scalar_qualifications.float_entry_ranges.remove(0);
    delivery_rejects(&module);
}

#[test]
fn roster_rejects_malformed_rows() {
    let conforming = range(0.0, 1.5, false);
    // Unknown owner machine.
    let mut unknown_owner = constant_delivery_module(conforming, 1.0f64.to_bits());
    unknown_owner.scalar_qualifications.float_entry_ranges[0].machine = machine_id(9);
    assert!(matches!(
        validate_module(&unknown_owner),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
    // Row attached to a value that is not a direct scalar parameter.
    let mut not_parameter = constant_delivery_module(conforming, 1.0f64.to_bits());
    not_parameter.scalar_qualifications.float_entry_ranges[0].parameter = value_id(5);
    assert!(matches!(
        validate_module(&not_parameter),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
    // Endpoints that do not IEEE-order.
    let mut unordered = constant_delivery_module(conforming, 1.0f64.to_bits());
    unordered.scalar_qualifications.float_entry_ranges[0].minimum =
        IeeeFloatValue::Binary64(2.0f64.to_bits());
    assert!(matches!(
        validate_module(&unordered),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
    // NaN endpoint.
    let mut nan_endpoint = constant_delivery_module(conforming, 1.0f64.to_bits());
    nan_endpoint.scalar_qualifications.float_entry_ranges[0].maximum =
        IeeeFloatValue::Binary64(f64::NAN.to_bits());
    assert!(matches!(
        validate_module(&nan_endpoint),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
    // Endpoint format narrower than the declared carrier.
    let mut mismatched_format = constant_delivery_module(conforming, 1.0f64.to_bits());
    mismatched_format.scalar_qualifications.float_entry_ranges[0].maximum =
        IeeeFloatValue::Binary32(1.5f32.to_bits());
    assert!(matches!(
        validate_module(&mismatched_format),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
    // Duplicate row for the same (machine, parameter) is noncanonical.
    let mut duplicated = constant_delivery_module(conforming, 1.0f64.to_bits());
    duplicated
        .scalar_qualifications
        .float_entry_ranges
        .push(conforming);
    assert!(matches!(
        validate_module(&duplicated),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
}

#[test]
fn f32_carrier_retains_binary32_endpoints() {
    // A binary32 carrier rejects binary64 endpoints and vice versa.
    let mut module = constant_delivery_module(range(0.0, 1.5, false), 1.0f64.to_bits());
    module.scalar_qualifications.float_entry_ranges[0] = ScalarFloatRange {
        machine: machine_id(2),
        parameter: value_id(4),
        minimum: IeeeFloatValue::Binary32(0.0f32.to_bits()),
        maximum: IeeeFloatValue::Binary32(1.5f32.to_bits()),
        maximum_inclusive: false,
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidScalarFloatRange { .. })
    ));
}

fn empty_contract(raw: u64) -> MachineContract {
    MachineContract {
        erased_scalar_formals: Vec::new(),
        id: ContractId::new(raw).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn declaration(id: ValueId, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    }
}

fn f64() -> ScalarType {
    ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
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

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}
