//! Authenticated one-field mutation coverage for installed-provider Unit
//! scalar-call custody rows.
//!
//! Each `installed_provider_unit_scalar_calls` record joins the caller's
//! retained parameter ABI, the selected provider's conformance row, the
//! candidate's retained parameter ABI, one internal-call relocation, one
//! replayed Unit call stack, and one exact semantic-attribution interval. The
//! object boundary accepts the family only in the zero-result shape: a
//! forwarded scalar parameter with a zero-byte argument interval inside one
//! 8-byte call-area adjustment. A substitution in any field must therefore be
//! rejected by canonical construction — none of the fields can drift without
//! breaking one of those joins.

use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValuePlacement, ValueShape, evaluate_call_plan,
};
use machine_code::{
    InstalledProviderUnitScalarCallRecord, InternalCallRelocation,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    MachineCodeFunction, MachineCodePlan, ParameterFunctionAbiRecord, SemanticCodeAttribution,
    SemanticCodeSite, StackAdjustmentPair, UnitAffineCleanupRecord, UnitCallStackEvidence,
    UnitScalarParameterLocationRecord, UnitStackEvidence,
};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    CallSiteOwner, CompletionClaimSource, ScalarAbiValue, TerminalPsiProvenance,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, EntryClaim, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    TerminalPsiIdentity, VocabularyMarker,
};

use crate::{ObjectError, build_object_artifact};

#[test]
fn installed_provider_unit_scalar_call_row_survives_object_replay() {
    let plan = installed_provider_scalar_call_plan();
    build_object_artifact(&plan).expect("exact installed-provider custody row replays");
}

/// Every representable field of an installed-provider Unit scalar-call row is
/// an authenticated custody axis: a one-field substitution is rejected by
/// object construction, and dropping or duplicating the row breaks the
/// relocation-identity join.
#[test]
fn installed_provider_unit_scalar_call_row_rejects_every_one_field_substitution() {
    let caller = MachineId::new(1).expect("caller");
    let custody_join = ObjectError::InvalidInternalUnitCallEvidence(caller);
    let record_join = ObjectError::InvalidInstalledProviderUnitScalarCallEvidence(caller);

    type Mutation = (&'static str, Box<dyn Fn(&mut MachineCodePlan)>, ObjectError);
    let mutations: Vec<Mutation> = vec![
        (
            "owner",
            Box::new(|plan| {
                provider_call(plan).owner = CallSiteOwner::Operation(OperationId::new(7).unwrap());
            }),
            custody_join.clone(),
        ),
        (
            "owner::cleanup_action",
            Box::new(|plan| {
                provider_call(plan).owner = CallSiteOwner::CleanupAction {
                    edge: EdgeId::new(1).unwrap(),
                    action_ordinal: 0,
                };
            }),
            custody_join.clone(),
        ),
        (
            "boundary",
            Box::new(|plan| {
                provider_call(plan).boundary = BoundaryMachineId::new(9).unwrap();
            }),
            record_join.clone(),
        ),
        (
            "provider.boundary",
            Box::new(|plan| {
                provider_call(plan).provider.boundary = BoundaryMachineId::new(9).unwrap();
            }),
            record_join.clone(),
        ),
        (
            "provider.requirement_identity",
            Box::new(|plan| {
                provider_call(plan).provider.requirement_identity.clear();
            }),
            record_join.clone(),
        ),
        (
            "provider.provider_identity",
            Box::new(|plan| {
                provider_call(plan).provider.provider_identity.clear();
            }),
            record_join.clone(),
        ),
        (
            "provider.candidate_identity",
            Box::new(|plan| {
                provider_call(plan).provider.candidate_identity.clear();
            }),
            record_join.clone(),
        ),
        (
            "provider.candidate",
            Box::new(|plan| {
                provider_call(plan).provider.candidate = MachineId::new(5).unwrap();
            }),
            custody_join.clone(),
        ),
        (
            "provider.candidate::caller",
            Box::new(|plan| {
                provider_call(plan).provider.candidate = MachineId::new(1).unwrap();
            }),
            custody_join.clone(),
        ),
        (
            "provider.signature.parameters",
            Box::new(|plan| {
                provider_call(plan).provider.signature.parameters.push(
                    ProviderSignatureParameter {
                        position: 0,
                        is_self: false,
                        structural_type: StructuralTypeId::new(7).unwrap(),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                );
            }),
            record_join.clone(),
        ),
        (
            "provider.refinement.positional_parameters",
            Box::new(|plan| {
                provider_call(plan)
                    .provider
                    .refinement
                    .positional_parameters
                    .push(ProviderParameterRefinement {
                        boundary_index: 0,
                        candidate_index: 0,
                    });
            }),
            record_join.clone(),
        ),
        (
            "call_plan",
            Box::new(|plan| {
                provider_call(plan).call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(plan.target),
                    &CallSignature {
                        parameters: vec![ValueShape::integer(4, 4)],
                        result: Some(ValueShape::integer(4, 4)),
                    },
                )
                .expect("scalar-result call plan");
            }),
            record_join.clone(),
        ),
        (
            "arguments::drop",
            Box::new(|plan| {
                provider_call(plan).arguments.clear();
            }),
            record_join.clone(),
        ),
        (
            "arguments::duplicate",
            Box::new(|plan| {
                let duplicate = provider_call(plan).arguments[0].clone();
                provider_call(plan).arguments.push(duplicate);
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].parameter_index",
            Box::new(|plan| {
                provider_call(plan).arguments[0].parameter_index = 1;
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].source.parameter_index",
            Box::new(|plan| {
                let InternalUnitScalarArgumentSourceRecord::Parameter {
                    parameter_index, ..
                } = &mut provider_call(plan).arguments[0].source
                else {
                    unreachable!("provider scalar argument is parameter-sourced")
                };
                *parameter_index = 1;
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].source.source_value",
            Box::new(|plan| {
                let InternalUnitScalarArgumentSourceRecord::Parameter { source_value, .. } =
                    &mut provider_call(plan).arguments[0].source
                else {
                    unreachable!("provider scalar argument is parameter-sourced")
                };
                *source_value = ValueId::new(12).unwrap();
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].source.scalar_type",
            Box::new(|plan| {
                let InternalUnitScalarArgumentSourceRecord::Parameter { scalar_type, .. } =
                    &mut provider_call(plan).arguments[0].source
                else {
                    unreachable!("provider scalar argument is parameter-sourced")
                };
                *scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).expect("i64"));
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].source.location",
            Box::new(|plan| {
                let InternalUnitScalarArgumentSourceRecord::Parameter { location, .. } =
                    &mut provider_call(plan).arguments[0].source
                else {
                    unreachable!("provider scalar argument is parameter-sourced")
                };
                *location = UnitScalarParameterLocationRecord::Register(MachineRegister::X86Rsi);
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].destination",
            Box::new(|plan| {
                provider_call(plan).arguments[0].destination = ValuePlacement {
                    shape: ValueShape::integer(0, 1),
                    locations: Vec::new(),
                };
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].code_offset",
            Box::new(|plan| {
                provider_call(plan).arguments[0].code_offset = 7;
            }),
            record_join.clone(),
        ),
        (
            "arguments[0].byte_count",
            Box::new(|plan| {
                provider_call(plan).arguments[0].byte_count = 4;
            }),
            record_join.clone(),
        ),
        (
            "source_arguments",
            Box::new(|plan| {
                provider_call(plan)
                    .source_arguments
                    .push(StructuralArgument {
                        place: PlaceId::new(9).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    });
            }),
            record_join.clone(),
        ),
        (
            "claim_transfers",
            Box::new(|plan| {
                provider_call(plan).claim_transfers.push(ClaimTransfer {
                    claim: ClaimId::new(9).unwrap(),
                    argument_index: 0,
                });
            }),
            record_join.clone(),
        ),
        (
            "completion_claim_sources",
            Box::new(|plan| {
                provider_call(plan)
                    .completion_claim_sources
                    .push(CompletionClaimSource {
                        claim: ClaimId::new(9).unwrap(),
                        entry: Some(EntryClaim {
                            claim: ClaimId::new(9).unwrap(),
                            input: PlaceId::new(9).unwrap(),
                            path: Vec::new(),
                        }),
                        content: None,
                    });
            }),
            record_join.clone(),
        ),
        (
            "completion_receipts",
            Box::new(|plan| {
                provider_call(plan)
                    .completion_receipts
                    .push(CompletionReceipt {
                        claim: ClaimId::new(9).unwrap(),
                        argument_index: 0,
                    });
            }),
            record_join.clone(),
        ),
        (
            "operation_ordinal",
            Box::new(|plan| {
                provider_call(plan).operation_ordinal = 1;
            }),
            record_join.clone(),
        ),
        (
            "code_offset",
            Box::new(|plan| {
                provider_call(plan).code_offset = 5;
            }),
            record_join.clone(),
        ),
        (
            "byte_count",
            Box::new(|plan| {
                provider_call(plan).byte_count = 12;
            }),
            record_join.clone(),
        ),
        (
            "roster::drop",
            Box::new(|plan| {
                plan.functions[0]
                    .installed_provider_unit_scalar_calls
                    .clear();
            }),
            custody_join.clone(),
        ),
        (
            "roster::duplicate",
            Box::new(|plan| {
                let duplicate = plan.functions[0].installed_provider_unit_scalar_calls[0].clone();
                plan.functions[0]
                    .installed_provider_unit_scalar_calls
                    .push(duplicate);
            }),
            custody_join.clone(),
        ),
    ];

    for (name, mutate, expected) in mutations {
        let mut plan = installed_provider_scalar_call_plan();
        mutate(&mut plan);
        assert_eq!(
            build_object_artifact(&plan),
            Err(expected),
            "{name}: substituted custody row must be rejected"
        );
    }
}

fn provider_call(plan: &mut MachineCodePlan) -> &mut InstalledProviderUnitScalarCallRecord {
    &mut plan.functions[0].installed_provider_unit_scalar_calls[0]
}

/// One caller-owned installed-provider scalar call: the caller forwards its
/// own signed-i32 register parameter to the selected provider's candidate,
/// whose only bytes are `ret`. The 8-byte call area sits inside the caller's
/// 16-byte frame; the record's interval is the pair `sub`+`call`+`add`.
fn installed_provider_scalar_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let i32_scalar = ScalarType::Integer(i32_type);
    let i32_shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![i32_shape],
            result: None,
        },
    )
    .expect("provider call plan");
    let parameter_placement = call_plan.parameters[0].clone();

    let boundary = BoundaryMachineId::new(7).expect("boundary");
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: "timer.tick".into(),
        provider_identity: "timer.host".into(),
        candidate_identity: "timer.host.tick".into(),
        candidate: machine_id(2),
        signature: ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };

    MachineCodePlan {
        psi: identity(),
        target,
        entry: machine_id(1),
        functions: vec![
            MachineCodeFunction {
                machine: machine_id(1),
                attachment: Some(StructuralTypeId::new(1).expect("attachment type")),
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: Some(ParameterFunctionAbiRecord {
                    call_plan: call_plan.clone(),
                    parameters: vec![ScalarAbiValue {
                        value: value_id(11),
                        scalar_type: i32_scalar,
                        placement: parameter_placement.clone(),
                    }],
                    entry_register_spills: Vec::new(),
                }),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: vec![
                    0x48, 0x83, 0xec, 0x10, // 0: sub rsp, 16 — frame allocation
                    0x48, 0x83, 0xec, 0x08, // 4: sub rsp, 8 — call area
                    0xe8, 0, 0, 0, 0, // 8: call rel32 — relocation at 9
                    0x48, 0x83, 0xc4, 0x08, // 13: add rsp, 8 — call area release
                    0x48, 0x83, 0xc4, 0x10, // 17: add rsp, 16 — frame release
                    0xc3, // 21: ret
                ],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: Some(StackAdjustmentPair {
                        byte_size: 16,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 17,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    target: machine_id(2),
                    unit_stack: Some(UnitCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 8,
                            allocation_offset: 4,
                            allocation_byte_count: 4,
                            release_offset: 13,
                            release_byte_count: 4,
                        }),
                    }),
                    scalar_stack: None,
                    offset: 9,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: vec![InstalledProviderUnitScalarCallRecord {
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    boundary,
                    provider,
                    call_plan: call_plan.clone(),
                    arguments: vec![InternalUnitScalarCallArgumentRecord {
                        parameter_index: 0,
                        source: InternalUnitScalarArgumentSourceRecord::Parameter {
                            parameter_index: 0,
                            source_value: value_id(11),
                            scalar_type: i32_scalar,
                            location: UnitScalarParameterLocationRecord::Register(
                                MachineRegister::X86Rdi,
                            ),
                        },
                        destination: parameter_placement.clone(),
                        code_offset: 8,
                        byte_count: 0,
                    }],
                    source_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    operation_ordinal: 0,
                    code_offset: 4,
                    byte_count: 13,
                }],
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    psi_edge: edge_id(1),
                    structural_types: Vec::new().into(),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 21,
                    byte_count: 1,
                }),
                unit_continuations: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 4,
                        byte_count: 13,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 1,
                        code_offset: 21,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                machine: machine_id(2),
                attachment: None,
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: Some(ParameterFunctionAbiRecord {
                    call_plan: call_plan.clone(),
                    parameters: vec![ScalarAbiValue {
                        value: value_id(21),
                        scalar_type: i32_scalar,
                        placement: parameter_placement.clone(),
                    }],
                    entry_register_spills: Vec::new(),
                }),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(9)],
                    edges: vec![edge_id(9)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                unit_affine_cleanup: None,
                unit_continuations: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("value")
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
