//! Installed scalar-result provider dispatch.
//!
//! A boundary whose declared result is scalar dispatches to the admitted
//! provider machine through the same suspended scalar frame an ordinary call
//! uses: exact scalar arguments bind to the callee parameters and the returned
//! scalar commits under the caller's declared result value.
//!
//! The fixtures keep `provider_candidates` empty and seed the interpreter's
//! post-verification runtime state directly — `start_verified_module` computes
//! the Terminal-Psi identity through canonical encoding, whose representation
//! validation routes every provider candidate row through the verifier's
//! result conformance. Scalar-result candidate rows are that gate's missing
//! arm, not the interpreter's dispatch decision; once verification admits
//! them, `provider_candidates`/`provider_installation` hold exactly the sets
//! seeded here.

use super::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue,
};
use crate::AcceptTerminalEffects;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, IeeeFloatComparisonOperation, IeeeFloatFormat,
    IeeeFloatValue, MachineId, OperationId, ScalarType, ValueId,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract, Operation,
    OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration,
};

const F32: ScalarType = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
const F64: ScalarType = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn scalar_declaration(id: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: value_id(id),
        scalar_type,
        qualifications: Default::default(),
    }
}

fn empty_machine(
    id: u64,
    result: TerminalMachineResult,
    parameters: Vec<ValueDeclaration>,
) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(id),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters,
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(id),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(id),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(id),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(id).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// One entry machine producing `operands` as float constants, then invoking
/// scalar boundary `boundary_id(1)` and returning its scalar result. Provider
/// machine `machine_id(2)` binds the same scalar parameters, runs
/// `provider_operation` once, and returns that operation's scalar result.
fn scalar_provider_module(
    operands: &[IeeeFloatValue],
    boundary_result: ScalarType,
    provider_result: ScalarType,
    provider_operation: OperationKind,
) -> TerminalModule {
    let mut entry = empty_machine(
        1,
        TerminalMachineResult::Scalar(scalar_declaration(100, boundary_result)),
        Vec::new(),
    );
    let scalar_arguments = operands
        .iter()
        .enumerate()
        .map(|(ordinal, _)| value_id(ordinal as u64 + 1))
        .collect::<Vec<_>>();
    entry.blocks[0].operations = operands
        .iter()
        .enumerate()
        .map(|(ordinal, value)| Operation {
            static_reach_binding: None,
            id: operation_id(ordinal as u64 + 1),
            result: OperationResult::Scalar(scalar_declaration(
                ordinal as u64 + 1,
                ScalarType::IeeeFloat(value.format()),
            )),
            kind: OperationKind::IeeeFloatConstant { value: *value },
        })
        .chain(std::iter::once(Operation {
            static_reach_binding: None,
            id: operation_id(10),
            result: OperationResult::Scalar(scalar_declaration(10, boundary_result)),
            kind: OperationKind::BoundaryCall {
                boundary: boundary_id(1),
                arguments: scalar_arguments,
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }))
        .collect();
    entry.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(10),
        cleanup_actions: Vec::new(),
    };

    let mut provider = empty_machine(
        2,
        TerminalMachineResult::Scalar(scalar_declaration(200, provider_result)),
        operands
            .iter()
            .enumerate()
            .map(|(ordinal, _)| scalar_declaration(ordinal as u64 + 11, F32))
            .collect(),
    );
    provider.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(20),
        result: OperationResult::Scalar(scalar_declaration(20, provider_result)),
        kind: provider_operation,
    }];
    provider.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(20),
        cleanup_actions: Vec::new(),
    };

    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary_id(1),
            identity: "test::combine".into(),
            attachment: None,
            scalar_parameters: operands
                .iter()
                .map(|value| ScalarType::IeeeFloat(value.format()))
                .collect(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Scalar(boundary_result),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
        }],
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
        machines: vec![entry, provider],
    }
}

fn float_fma_module() -> TerminalModule {
    scalar_provider_module(
        &[
            IeeeFloatValue::Binary32(0x4000_0000), // 2.0
            IeeeFloatValue::Binary32(0x4040_0000), // 3.0
            IeeeFloatValue::Binary32(0x4080_0000), // 4.0
        ],
        F32,
        F32,
        OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left: value_id(11),
            right: value_id(12),
            addend: value_id(13),
        },
    )
}

fn boolean_compare_module() -> TerminalModule {
    scalar_provider_module(
        &[
            IeeeFloatValue::Binary32(0x3f80_0000), // 1.0
            IeeeFloatValue::Binary32(0x4000_0000), // 2.0
        ],
        ScalarType::Boolean,
        ScalarType::Boolean,
        OperationKind::IeeeFloatCompare {
            comparison: IeeeFloatComparisonOperation::LessOrEqual,
            left: value_id(11),
            right: value_id(12),
        },
    )
}

/// Seed the exact runtime sets `start_verified_module` derives from the
/// verified provider rows and the admitted installation.
fn install_scalar_provider(execution: &mut TerminalExecution) {
    execution.provider_candidates.insert(boundary_id(1));
    execution
        .provider_installation
        .insert(boundary_id(1), machine_id(2));
}

fn run_to_completion(
    mut execution: TerminalExecution,
) -> (TerminalExecution, TerminalExecutionResult) {
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let result = loop {
        match execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("scalar provider execution")
        {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(result) => break result,
            status => panic!("unexpected {status:?}"),
        }
    };
    (execution, result)
}

#[test]
fn installed_scalar_provider_executes_its_machine_and_commits_the_float_result() {
    let module = float_fma_module();
    // The boundary/scalar-call fixture itself is a valid verified module; only
    // the scalar provider candidate row cannot yet be admitted or encoded.
    terminal_verifier::verify_module(
        &module,
        &terminal_verifier::ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("scalar boundary fixture verifies");
    let mut execution = TerminalExecution::start_verified_module(module, &[], &[], &[], &[], None)
        .expect("scalar provider module starts");
    install_scalar_provider(&mut execution);
    let (execution, result) = run_to_completion(execution);
    // round_nearest_even(2.0 * 3.0 + 4.0) = 10.0f32.
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(
            0x4120_0000
        )))
    );
    // The provider call is internal dispatch, not an observable effect.
    assert!(execution.effects().is_empty());
}

#[test]
fn installed_scalar_provider_executes_a_boolean_result() {
    let module = boolean_compare_module();
    terminal_verifier::verify_module(
        &module,
        &terminal_verifier::ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("scalar boundary fixture verifies");
    let mut execution = TerminalExecution::start_verified_module(module, &[], &[], &[], &[], None)
        .expect("scalar provider module starts");
    install_scalar_provider(&mut execution);
    let (execution, result) = run_to_completion(execution);
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert!(execution.effects().is_empty());
}

#[test]
fn scalar_provider_candidate_without_installation_still_rejects() {
    let mut execution =
        TerminalExecution::start_verified_module(float_fma_module(), &[], &[], &[], &[], None)
            .unwrap();
    execution.provider_candidates.insert(boundary_id(1));
    assert!(matches!(
        execution.resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects
        ),
        Err(TerminalInterpretError::ProviderInstallationMissing(boundary))
            if boundary == boundary_id(1)
    ));
    assert!(execution.effects().is_empty());
}

#[test]
fn scalar_provider_result_type_drift_rejects_at_dispatch() {
    // An internally consistent F64 provider installed on an F32 boundary:
    // scalar arguments still bind, but the callee's declared result type must
    // equal the operation's declared result type before the frame transfers.
    let module = scalar_provider_module(
        &[
            IeeeFloatValue::Binary32(0x4000_0000), // 2.0
            IeeeFloatValue::Binary32(0x4040_0000), // 3.0
            IeeeFloatValue::Binary32(0x4080_0000), // 4.0
        ],
        F32,
        F64,
        OperationKind::IeeeFloatConstant {
            value: IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        },
    );
    let mut execution =
        TerminalExecution::start_verified_module(module, &[], &[], &[], &[], None).unwrap();
    install_scalar_provider(&mut execution);
    assert!(matches!(
        execution.resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects
        ),
        Err(TerminalInterpretError::VerifiedOperationMalformed)
    ));
}
