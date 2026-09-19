//! Fixtures shared by the unit interpretation tests: effect artifact
//! sections, structural places, call modules and identities.
//! `effect_modules.rs`, `call_modules.rs` and `nominal_affine_modules.rs` hold
//! the module fixtures; this file keeps the handlers, structural helpers and
//! identity helpers.

use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
#[path = "unit/affine_cleanups.rs"]
mod affine_cleanups;
#[path = "unit/affine_identity_calls.rs"]
mod affine_identity_calls;
#[path = "unit/borrowed_storage_windows.rs"]
mod borrowed_storage_windows;
#[path = "unit/boundary_borrows.rs"]
mod boundary_borrows;
#[path = "unit/bounded_fields.rs"]
mod bounded_fields;
#[path = "unit/byte_sequence_forwarding.rs"]
mod byte_sequence_forwarding;
#[path = "unit/byte_sequence_length.rs"]
mod byte_sequence_length;
#[path = "unit/byte_sequence_read.rs"]
mod byte_sequence_read;
#[path = "unit/byte_sequence_scalar_calls.rs"]
mod byte_sequence_scalar_calls;
#[path = "unit/byte_sequence_scalar_view_transfers.rs"]
mod byte_sequence_scalar_view_transfers;
#[path = "unit/byte_sequence_subslice.rs"]
mod byte_sequence_subslice;
#[path = "unit/call_modules.rs"]
mod call_modules;
#[path = "unit/case_membership.rs"]
mod case_membership;
#[path = "unit/claims_and_effects.rs"]
mod claims_and_effects;
#[path = "unit/cyclic_receiver.rs"]
mod cyclic_receiver;
#[path = "unit/effect_modules.rs"]
mod effect_modules;
#[path = "unit/ieee_float_comparisons.rs"]
mod ieee_float_comparisons;
#[path = "unit/indexed_primitive_stores.rs"]
mod indexed_primitive_stores;
#[path = "unit/indexed_structural_store.rs"]
mod indexed_structural_store;
#[path = "unit/nominal_affine_modules.rs"]
mod nominal_affine_modules;
#[path = "unit/primitive_arrays.rs"]
mod primitive_arrays;
#[path = "unit/primitive_locals.rs"]
mod primitive_locals;
#[path = "unit/records.rs"]
mod records;
#[path = "unit/reference_records.rs"]
mod reference_records;
#[path = "unit/result_residuals.rs"]
mod result_residuals;
#[path = "unit/scalar_arrays.rs"]
mod scalar_arrays;
#[path = "unit/scalar_cases.rs"]
mod scalar_cases;
#[path = "unit/scalar_qualifications.rs"]
mod scalar_qualifications;
#[path = "unit/scalar_returns_and_nominal_modules.rs"]
mod scalar_returns_and_nominal_modules;
#[path = "unit/unit_results_and_fuel.rs"]
mod unit_results_and_fuel;

use call_modules::{
    internal_structural_call_module, joined_parameter_dynamic_scalar_call_module,
    multi_claim_internal_structural_call_module, parameter_dynamic_scalar_call_module,
    rebound_dynamic_scalar_call_module, structural_scalar_field_call_module,
    write_only_boolean_call_module, write_only_primitive_call_module,
};
use effect_modules::{
    byte_sequence_literal_module, effect_artifact_sections, effect_module, nearest_fma_module,
    payloadless_call_module, payloadless_case_module, reference_release_module,
    scalar_boundary_effect_module, structural_boundary_effect_module, unit_module,
};
use nominal_affine_modules::{
    executable_nominal_affine_module, nominal_affine_module, ordered_empty_nominal_affine_module,
    ordered_one_executable_nominal_affine_module, ordered_shared_executable_nominal_affine_module,
    ordered_two_distinct_executable_nominal_affine_module, partial_affine_field_module,
    three_helper_nominal_affine_module, three_ordered_empty_nominal_affine_module,
    three_ordered_shared_executable_nominal_affine_module, two_helper_nominal_affine_module,
};

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, EvidenceIdentity, IeeeFloatFormat,
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimTransfer,
    CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError, verify_module,
};

fn assert_write_only_store_atomic(
    module: TerminalModule,
    opaque_identity: u64,
    initial_value: TerminalScalarValue,
    written_value: TerminalScalarValue,
) {
    let semantic = encode_module(&module).expect("primitive-store semantics encode");
    let proof =
        encode_proof_section(&module, &ProofBundle::default()).expect("empty proof encodes");
    let structural = TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type_id(91),
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: initial_value,
    };
    let written = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: written_value,
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[structural],
            primitive_values: &[initial],
            ..Default::default()
        },
    )
    .expect("verified Boolean store starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);

    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Operation(operation_id(93)),
            required_units: 1,
            remaining_units: 0,
        })
    );
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    assert_eq!(meter.usage().total_units(), 2);

    meter.replenish(3).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.structural_primitive_values(), vec![written]);
    assert_eq!(meter.usage().total_units(), 5);
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation_id(93)))
            .unwrap()
            .executions(),
        1,
        "the primitive store commits exactly once after replenishment"
    );
}

struct BoundaryResultHandler {
    result: Result<TerminalEffectResult, TerminalEffectRejection>,
    requests: usize,
    effects: Vec<TerminalEffect>,
}

impl TerminalEffectHandler for BoundaryResultHandler {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-bearing boundary must use the explicit result handler");
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.requests += 1;
        let result = self.result.clone()?;
        self.effects.push(effect.clone());
        Ok(result)
    }
}

#[derive(Default)]
struct RecordingHandler {
    effects: Vec<TerminalEffect>,
}

impl TerminalEffectHandler for RecordingHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.effects.push(effect.clone());
        Ok(())
    }
}

struct RejectingHandler;

impl TerminalEffectHandler for RejectingHandler {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new("mock rejection"))
    }
}

struct RejectScalarBoundaryArguments;

impl TerminalEffectHandler for RejectScalarBoundaryArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { arguments, .. }
                if arguments
                    == &[
                        TerminalScalarValue::Boolean(true),
                        TerminalScalarValue::Boolean(false),
                    ] =>
            {
                Err(TerminalEffectRejection::new(
                    "mock policy rejects the scalar boundary argument",
                ))
            }
            _ => Err(TerminalEffectRejection::new(
                "scalar boundary arguments were not resolved in declaration order",
            )),
        }
    }
}

fn structural_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
    domain: StructuralDomainId,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
    }
}

fn structural_place(id: PlaceId) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }
}

fn structural_value(opaque_identity: u64) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type: structural_type_id(1),
        qualifications: vec![structural_domain_id(1)],
        path: Vec::new(),
    }
}

fn empty_contract(id: ContractId) -> MachineContract {
    MachineContract {
        id,
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn artifact_sections() -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(&unit_module()).expect("unit semantics encode"),
        encode_proof_section(&unit_module(), &ProofBundle::default()).expect("empty proof encodes"),
    )
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_field_id(raw: u64) -> StructuralFieldId {
    StructuralFieldId::new(raw).unwrap()
}

fn structural_case_id(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
