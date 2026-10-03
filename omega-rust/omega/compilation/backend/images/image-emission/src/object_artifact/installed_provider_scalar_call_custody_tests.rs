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
//!
//! The substitution legs run through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `InstalledProviderUnitScalarCallFieldForTest` inventory, so a new
//! representable axis extends coverage by one variant instead of a parallel
//! mutation list.

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
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
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

optimization_core::custody_field_inventory! {
    /// One substitutable custody axis of the installed-provider Unit
    /// scalar-call record or of its roster. Every declared axis is bound by
    /// exact construction joins — the caller's retained parameter ABI, the
    /// provider conformance row, the candidate's ABI, the internal-call
    /// relocation and call stack, and the attribution interval — so each
    /// substitution is rejected by canonical construction; none still builds
    /// to reach replay. `provider.refinement.required_domains` and
    /// `provider.refinement.realized_service_ceiling` are the family's
    /// unauthenticated fields: the checker admits them unexamined, so they
    /// cannot appear as rejecting substitution axes.
    enum InstalledProviderUnitScalarCallFieldForTest {
        Owner,
        OwnerCleanupAction,
        Boundary,
        ProviderBoundary,
        ProviderRequirementIdentity,
        ProviderProviderIdentity,
        ProviderCandidateIdentity,
        ProviderCandidate,
        ProviderCandidateCaller,
        ProviderSignatureParameters,
        ProviderRefinementPositionalParameters,
        CallPlan,
        ArgumentsDropped,
        ArgumentsDuplicated,
        ArgumentParameterIndex,
        ArgumentSourceParameterIndex,
        ArgumentSourceValue,
        ArgumentSourceScalarType,
        ArgumentSourceLocation,
        ArgumentDestination,
        ArgumentCodeOffset,
        ArgumentByteCount,
        SourceArguments,
        ClaimTransfers,
        CompletionClaimSources,
        CompletionReceipts,
        OperationOrdinal,
        CodeOffset,
        ByteCount,
        RosterDropped,
        RosterDuplicated,
    }
}

/// Every representable field of an installed-provider Unit scalar-call row is
/// an authenticated custody axis: a one-field substitution is rejected by
/// object construction, and dropping or duplicating the row breaks the
/// relocation-identity join.
#[test]
fn installed_provider_unit_scalar_call_row_rejects_every_one_field_substitution() {
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installed provider Unit scalar-call row",
        fields: InstalledProviderUnitScalarCallFieldForTest::INVENTORY,
        honest: &installed_provider_scalar_call_plan,
        donor: foreign_installed_provider_scalar_call_plan(),
        custody: &installed_provider_row_custody,
        substitute: &substitute_installed_provider_unit_scalar_call,
        check: &installed_provider_row_check,
        outcome: &installed_provider_unit_scalar_call_outcome,
        joined_replay: None,
    });
}

/// The retained custody view a substitution must move: the caller's
/// installed-provider roster itself.
fn installed_provider_row_custody(
    plan: &MachineCodePlan,
) -> Vec<InstalledProviderUnitScalarCallRecord> {
    plan.functions[0]
        .installed_provider_unit_scalar_calls
        .clone()
}

/// The family's independent checker: canonical object construction. On an
/// admitted plan the rebuilt custody is the roster the artifact itself
/// retains.
fn installed_provider_row_check(
    plan: &MachineCodePlan,
) -> Result<Vec<InstalledProviderUnitScalarCallRecord>, ObjectError> {
    let artifact = build_object_artifact(plan)?;
    Ok(artifact
        .functions
        .iter()
        .find(|function| function.machine == plan.functions[0].machine)
        .map(|function| function.installed_provider_unit_scalar_calls.clone())
        .unwrap_or_default())
}

/// Mutate exactly one custody axis of the caller's installed-provider row;
/// `donor` grants no new authority and is retained for signature uniformity.
fn substitute_installed_provider_unit_scalar_call(
    plan: &mut MachineCodePlan,
    field: InstalledProviderUnitScalarCallFieldForTest,
    _donor: &MachineCodePlan,
) {
    use InstalledProviderUnitScalarCallFieldForTest as Field;
    match field {
        Field::Owner => {
            provider_call(plan).owner = CallSiteOwner::Operation(OperationId::new(7).unwrap());
        }
        Field::OwnerCleanupAction => {
            provider_call(plan).owner = CallSiteOwner::CleanupAction {
                edge: EdgeId::new(1).unwrap(),
                action_ordinal: 0,
            };
        }
        Field::Boundary => {
            provider_call(plan).boundary = BoundaryMachineId::new(9).unwrap();
        }
        Field::ProviderBoundary => {
            provider_call(plan).provider.boundary = BoundaryMachineId::new(9).unwrap();
        }
        Field::ProviderRequirementIdentity => {
            provider_call(plan).provider.requirement_identity.clear();
        }
        Field::ProviderProviderIdentity => {
            provider_call(plan).provider.provider_identity.clear();
        }
        Field::ProviderCandidateIdentity => {
            provider_call(plan).provider.candidate_identity.clear();
        }
        Field::ProviderCandidate => {
            provider_call(plan).provider.candidate = MachineId::new(5).unwrap();
        }
        Field::ProviderCandidateCaller => {
            provider_call(plan).provider.candidate = MachineId::new(1).unwrap();
        }
        Field::ProviderSignatureParameters => {
            provider_call(plan)
                .provider
                .signature
                .parameters
                .push(ProviderSignatureParameter {
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(7).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
        }
        Field::ProviderRefinementPositionalParameters => {
            provider_call(plan)
                .provider
                .refinement
                .positional_parameters
                .push(ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                });
        }
        Field::CallPlan => {
            provider_call(plan).call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(plan.target),
                &CallSignature {
                    parameters: vec![ValueShape::integer(4, 4)],
                    result: Some(ValueShape::integer(4, 4)),
                },
            )
            .expect("scalar-result call plan");
        }
        Field::ArgumentsDropped => {
            provider_call(plan).arguments.clear();
        }
        Field::ArgumentsDuplicated => {
            let duplicate = provider_call(plan).arguments[0].clone();
            provider_call(plan).arguments.push(duplicate);
        }
        Field::ArgumentParameterIndex => {
            provider_call(plan).arguments[0].parameter_index = 1;
        }
        Field::ArgumentSourceParameterIndex => {
            let InternalUnitScalarArgumentSourceRecord::Parameter {
                parameter_index, ..
            } = &mut provider_call(plan).arguments[0].source
            else {
                unreachable!("provider scalar argument is parameter-sourced")
            };
            *parameter_index = 1;
        }
        Field::ArgumentSourceValue => {
            let InternalUnitScalarArgumentSourceRecord::Parameter { source_value, .. } =
                &mut provider_call(plan).arguments[0].source
            else {
                unreachable!("provider scalar argument is parameter-sourced")
            };
            *source_value = ValueId::new(12).unwrap();
        }
        Field::ArgumentSourceScalarType => {
            let InternalUnitScalarArgumentSourceRecord::Parameter { scalar_type, .. } =
                &mut provider_call(plan).arguments[0].source
            else {
                unreachable!("provider scalar argument is parameter-sourced")
            };
            *scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).expect("i64"));
        }
        Field::ArgumentSourceLocation => {
            let InternalUnitScalarArgumentSourceRecord::Parameter { location, .. } =
                &mut provider_call(plan).arguments[0].source
            else {
                unreachable!("provider scalar argument is parameter-sourced")
            };
            *location = UnitScalarParameterLocationRecord::Register(MachineRegister::X86Rsi);
        }
        Field::ArgumentDestination => {
            provider_call(plan).arguments[0].destination = ValuePlacement {
                shape: ValueShape::integer(0, 1),
                locations: Vec::new(),
            };
        }
        Field::ArgumentCodeOffset => {
            provider_call(plan).arguments[0].code_offset = 7;
        }
        Field::ArgumentByteCount => {
            provider_call(plan).arguments[0].byte_count = 4;
        }
        Field::SourceArguments => {
            provider_call(plan)
                .source_arguments
                .push(StructuralArgument {
                    place: PlaceId::new(9).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                });
        }
        Field::ClaimTransfers => {
            provider_call(plan).claim_transfers.push(ClaimTransfer {
                claim: ClaimId::new(9).unwrap(),
                argument_index: 0,
            });
        }
        Field::CompletionClaimSources => {
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
        }
        Field::CompletionReceipts => {
            provider_call(plan)
                .completion_receipts
                .push(CompletionReceipt {
                    claim: ClaimId::new(9).unwrap(),
                    argument_index: 0,
                });
        }
        Field::OperationOrdinal => {
            provider_call(plan).operation_ordinal = 1;
        }
        Field::CodeOffset => {
            provider_call(plan).code_offset = 5;
        }
        Field::ByteCount => {
            provider_call(plan).byte_count = 12;
        }
        Field::RosterDropped => {
            plan.functions[0]
                .installed_provider_unit_scalar_calls
                .clear();
        }
        Field::RosterDuplicated => {
            let duplicate = plan.functions[0].installed_provider_unit_scalar_calls[0].clone();
            plan.functions[0]
                .installed_provider_unit_scalar_calls
                .push(duplicate);
        }
    }
}

/// The expected checker outcome per field: owner, candidate and roster legs
/// orphan the internal-call relocation join first; every other leg surfaces
/// the family's own canonical-construction rejection.
fn installed_provider_unit_scalar_call_outcome(
    field: InstalledProviderUnitScalarCallFieldForTest,
) -> MutationOutcome<ObjectError> {
    use InstalledProviderUnitScalarCallFieldForTest as Field;
    let caller = MachineId::new(1).expect("caller");
    MutationOutcome::ExactError(match field {
        Field::Owner
        | Field::OwnerCleanupAction
        | Field::ProviderCandidate
        | Field::ProviderCandidateCaller
        | Field::RosterDropped
        | Field::RosterDuplicated => ObjectError::InvalidInternalUnitCallEvidence(caller),
        _ => ObjectError::InvalidInstalledProviderUnitScalarCallEvidence(caller),
    })
}

fn provider_call(plan: &mut MachineCodePlan) -> &mut InstalledProviderUnitScalarCallRecord {
    &mut plan.functions[0].installed_provider_unit_scalar_calls[0]
}

/// A second honestly produced plan whose retained custody roster differs: the
/// provider call targets a foreign candidate machine under a foreign
/// boundary, proving the matrix's custody view is not vacuous. Construction
/// accepts it, so the donor is authentic evidence of the same family.
fn foreign_installed_provider_scalar_call_plan() -> MachineCodePlan {
    let mut plan = installed_provider_scalar_call_plan();
    let candidate = machine_id(3);
    plan.functions[1].machine = candidate;
    plan.functions[0].internal_calls[0].target = candidate;
    let row = &mut plan.functions[0].installed_provider_unit_scalar_calls[0];
    row.provider.candidate = candidate;
    row.boundary = BoundaryMachineId::new(8).expect("foreign boundary");
    row.provider.boundary = row.boundary;
    build_object_artifact(&plan).expect("the foreign donor plan builds authentically");
    plan
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
