//! Installed scalar-result provider dispatch.
//!
//! A boundary whose declared result is scalar dispatches to the admitted
//! provider machine through the same suspended scalar frame an ordinary call
//! uses: exact scalar arguments bind to the callee parameters and the returned
//! scalar commits under the caller's declared result value.
//!
//! The fixtures carry the canonical provider-candidate row itself:
//! `start_verified_module` computes the Terminal-Psi identity through
//! canonical encoding, whose representation validation routes the row through
//! the verifier's scalar result conformance, and `provider_candidates` is
//! then the verified module's own catalog. The installation selection —
//! artifact-level state an `AdmittedProviderInstallation` carries — is bound
//! exactly rather than seeded into the execution.

use super::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue,
};
use crate::{AcceptTerminalEffects, AdmittedProviderInstallation, ProviderInstallationSelection};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, IeeeFloatComparisonOperation, IeeeFloatFormat,
    IeeeFloatValue, MachineId, OperationId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, MachineContract, Operation,
    OperationKind, OperationResult, ProviderCandidateConformance, ProviderRefinement,
    ProviderSignature, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
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

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
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
    // Conformance requires a nominal provider attachment; the empty record
    // supplies it without specializing any erased field.
    provider.attachment = Some(structural_type_id(1));
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
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "test::Provider".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
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
        provider_candidates: vec![ProviderCandidateConformance {
            boundary: boundary_id(1),
            requirement_identity: "test::combine".into(),
            provider_identity: "test::Provider".into(),
            candidate_identity: "test::Provider::combine".into(),
            candidate: machine_id(2),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }],
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

/// Encode the module into its canonical artifact sections and admit the
/// provider selection through `admit_provider_installation_from_artifact`:
/// the scalar row must survive decode-side representation validation and
/// verification before the installation can bind.
fn scalar_provider_installation(module: &TerminalModule) -> AdmittedProviderInstallation {
    let semantic =
        terminal_codec::encode_module(module).expect("verified scalar provider module encodes");
    let proof =
        terminal_codec::encode_proof_section(module, &terminal_verifier::ProofBundle::default())
            .expect("proof section seals");
    super::admit_provider_installation_from_artifact(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: boundary_id(1),
            provider_identity: "test::Provider".into(),
            candidate: machine_id(2),
        }],
    )
    .expect("scalar provider row admits from the artifact")
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
    // The scalar provider candidate row admits through the same verification
    // every validation policy and the codec representation check share.
    terminal_verifier::verify_module(
        &module,
        &terminal_verifier::ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("scalar provider fixture verifies");
    let installation = scalar_provider_installation(&module);
    let execution = TerminalExecution::start_verified_module(
        module,
        &[],
        &[],
        &[],
        &[],
        &[],
        Some(&installation),
    )
    .expect("scalar provider module starts");
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
    .expect("scalar provider fixture verifies");
    let installation = scalar_provider_installation(&module);
    let execution = TerminalExecution::start_verified_module(
        module,
        &[],
        &[],
        &[],
        &[],
        &[],
        Some(&installation),
    )
    .expect("scalar provider module starts");
    let (execution, result) = run_to_completion(execution);
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert!(execution.effects().is_empty());
}

#[test]
fn scalar_provider_candidate_without_installation_still_rejects() {
    // The verified catalog row alone is not an installation: dispatch must
    // still find the boundary's selected candidate.
    let mut execution =
        TerminalExecution::start_verified_module(float_fma_module(), &[], &[], &[], &[], &[], None)
            .unwrap();
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
fn scalar_provider_result_type_drift_rejects_at_verification_and_dispatch() {
    // An internally consistent F64 provider selected on an F32 boundary: the
    // verifier's provider-result conformance is the first fence — the drifted
    // row cannot enter the verified catalog.
    let mut module = scalar_provider_module(
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
    assert!(matches!(
        terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::Module(
            terminal_verifier::ModuleError::InvalidProviderCandidate { .. }
        ))
    ));
    // If a foreign or stale installation still names the drifted machine,
    // dispatch keeps its own defense: scalar arguments bind, but the callee's
    // declared result type must equal the operation's before the frame
    // transfers.
    module.provider_candidates.clear();
    let mut execution =
        TerminalExecution::start_verified_module(module, &[], &[], &[], &[], &[], None).unwrap();
    execution.provider_candidates.insert(boundary_id(1));
    execution
        .provider_installation
        .insert(boundary_id(1), machine_id(2));
    assert!(matches!(
        execution.resume(
            &mut TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects
        ),
        Err(TerminalInterpretError::VerifiedOperationMalformed)
    ));
}
