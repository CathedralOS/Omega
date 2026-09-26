//! Boundary, candidate, fuel, stack demand, interrupt completion and
//! provider execution fixtures.

use super::installed_code_fixtures::{
    constraints, entry_id, fuel_schedule, installed_code, installed_program_storage_wrapper,
    root_id,
};
use super::{TestObject, TestStackDemand};
use crate::{
    AcknowledgementPolicyId, AdmittedOpaqueArrivalContextSet, BoundEpochStackComposition,
    BoundEpochStackCompositionInput, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPin, ComponentVersionPinId, ComposedFuelDemand, ExternalRootCandidate,
    ExternalRootId, FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId,
    FuelValidationReceiptId, GeneratedProgramStorageAdapterLiveFrameDemand,
    LogicalFuelResourceColumn, MachineStateResourceColumn, NestingRelationId,
    OpaqueProviderExitAssurance, ProviderExecution, ProviderExecutionId, ProviderFuelSummaryId,
    ProviderFuelValidationReceiptId, ProviderPlanId, ProviderStackSummary,
    ResolvedRootServiceReach, RootEffectId, RootProviderId, RootSlotAuthority, RootSlotId,
    RootSlotOwnerId, StackNestingRelation, StackResourceColumn, StackValidationReceiptId,
    StateValidationReceiptId, TrustReceiptId, ValidatedExternalRoot,
    X86_64GeneratedProgramStorageAdapterEmission, admit_opaque_arrival_context_set,
    bind_installed_entry_stack, bind_opaque_adapter_stack_realization,
    bind_x86_64_generated_program_storage_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel,
    derive_generated_program_storage_adapter_live_frame_demand,
};
use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, CallSignature,
    CallingPolicy, EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineState,
    MachineStateSet, Preemption, RegisterSet, StackDomainRef, ValidatedEntryStackDomainClosure,
    ValueShape, evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_entry_stack_domain_closure, validate_entry_stack_realization,
};
use calling_conventions::{
    EntryStack, MachineRegister, ProviderExitRealization, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan,
};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use executable_installation::InstalledCode;
use isa_x86_64::{
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
};
use layout_plans::EntryStubId;
use layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementPhase, PlacementSite,
    PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};
use std::collections::BTreeSet;

pub(crate) fn boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated boundary")
}

pub(super) fn two_parameter_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated two-parameter boundary")
}

pub(super) fn provider_selected_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary boundary");
    let mut plan = ordinary.plan().clone();
    plan.state.stack = EntryStack::ProviderSelected;
    validate_boundary_entry_plan(plan, &signature).expect("provider-selected boundary")
}

pub(super) fn provider_selected_masked_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let mut plan = provider_selected_boundary().plan().clone();
    plan.state.preemption = Preemption::Masked;
    validate_boundary_entry_plan(plan, &signature).expect("provider-selected masked boundary")
}

pub(super) fn interrupted_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary boundary");
    let mut plan = ordinary.plan().clone();
    plan.state.stack = EntryStack::Interrupted;
    validate_boundary_entry_plan(plan, &signature).expect("interrupted boundary")
}

pub(super) fn generated_program_storage_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8), ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("receiver-free ProgramStorage semantic continuation boundary")
}

pub(crate) fn generated_program_storage_adapter_live_frame_demand()
-> GeneratedProgramStorageAdapterLiveFrameDemand {
    derive_generated_program_storage_adapter_live_frame_demand(
        &generated_program_storage_adapter_bound_input(),
    )
    .expect("canonical installed generated wrapper derives its live frame demand")
}

pub(super) fn generated_program_storage_adapter_bound_input() -> BoundEpochStackCompositionInput {
    let entry = entry_id(0x8f1);
    let boundary = generated_program_storage_boundary();
    let request =
        canonical_x86_64_semantic_unit_wrapper_encoding_request(target::NativeTarget::uefi_x64());
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved private continuation call");
    let (code, installed_image) = installed_program_storage_wrapper(0x8f2, entry, resolved.bytes());
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x8f; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x8f3, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x8f4, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect("generated adapter binds exact installed entry and body evidence")
}

pub(super) fn body_domains(
    boundary: &ValidatedBoundaryEntryPlan,
    contexts: &[(u64, StackDomainRef)],
) -> ValidatedEntryStackDomainClosure {
    validate_entry_stack_domain_closure(
        boundary.plan().state.stack,
        contexts
            .iter()
            .map(|(context, domain)| ArrivalContextStackDomain {
                context: ArrivalContextId::new(*context).expect("arrival context"),
                domain: *domain,
            })
            .collect(),
    )
    .expect("test stack-domain closure")
}

pub(super) fn admitted_arrival_contexts(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    contexts: &[u64],
    receipt: StackValidationReceiptId,
) -> AdmittedOpaqueArrivalContextSet {
    admit_opaque_arrival_context_set(
        summary,
        boundary,
        code,
        entry,
        contexts
            .iter()
            .map(|context| ArrivalContextId::new(*context).expect("arrival context"))
            .collect(),
        receipt,
    )
    .expect("admitted opaque arrival-context closure")
}

pub(crate) fn fixed_fuel() -> ComposedFuelDemand {
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(31, ProviderFuelSummaryId::from_normalized_identity),
        root_id(12, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        5,
        BTreeSet::new(),
        root_id(
            41,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(30, ProviderFuelSummaryId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        2,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 1,
        }]),
        root_id(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
}

/// One member's bound epoch input. Interrupt-table tests compose several of
/// these into the single artifact-wide composition every installed root of
/// one artifact must carry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_demand_input(
    root: ExternalRootId,
    provider: RootProviderId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
) -> BoundEpochStackCompositionInput {
    let active_domain = StackDomainRef::from(resolved_stack);
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain,
                occupancy_by_domain: Vec::new(),
                nesting: boundary.plan().state.preemption,
            }],
        }],
    })
    .expect("test epoch realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        local_wcsu_bytes,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts = admitted_arrival_contexts(
        &summary,
        boundary,
        code,
        entry,
        &[1],
        root_id(48, StackValidationReceiptId::from_normalized_identity),
    );
    bind_opaque_adapter_stack_realization(&summary, boundary, code, entry, realization, contexts)
        .expect("test epoch evidence binding")
}

/// One member's bound epoch input with an explicit arrival-context roster:
/// each `(context, epochs)` row admits that context whose epochs run on the
/// resolved stack domain with the given `(stage, nesting)` dispositions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_epoch_input(
    root: ExternalRootId,
    provider: RootProviderId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
    contexts: &[(u64, &[(EntryStackStage, Preemption)])],
) -> BoundEpochStackCompositionInput {
    let active_domain = StackDomainRef::from(resolved_stack);
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: contexts
            .iter()
            .map(|(context, epochs)| ArrivalContextRealization {
                context: ArrivalContextId::new(*context).expect("arrival context"),
                epochs: epochs
                    .iter()
                    .map(|(stage, nesting)| EntryStackEpoch {
                        stage: *stage,
                        active_domain,
                        occupancy_by_domain: Vec::new(),
                        nesting: *nesting,
                    })
                    .collect(),
            })
            .collect(),
    })
    .expect("test epoch realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        local_wcsu_bytes,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let admitted = admitted_arrival_contexts(
        &summary,
        boundary,
        code,
        entry,
        &contexts
            .iter()
            .map(|(context, _)| *context)
            .collect::<Vec<_>>(),
        root_id(48, StackValidationReceiptId::from_normalized_identity),
    );
    bind_opaque_adapter_stack_realization(&summary, boundary, code, entry, realization, admitted)
        .expect("test epoch evidence binding")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_demand(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
) -> BoundEpochStackComposition {
    let bound = stack_demand_input(
        root,
        provider,
        boundary,
        code,
        entry,
        resolved_stack,
        local_wcsu_bytes,
    );
    compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("bound epoch stack composition")
}

pub(super) fn candidate(entry: EntryStubId) -> ExternalRootCandidate {
    candidate_for_code(entry, &installed_code(1, entry))
}

pub(super) fn candidate_for_code(
    entry: EntryStubId,
    code: &InstalledCode,
) -> ExternalRootCandidate {
    candidate_for_code_with_root(entry, code, 1)
}

pub(crate) fn candidate_for_code_with_root(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
) -> ExternalRootCandidate {
    candidate_for_code_with_root_on_boundary(entry, code, root_identity, &boundary())
}

pub(crate) fn candidate_for_code_with_root_on_boundary(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
    boundary: &ValidatedBoundaryEntryPlan,
) -> ExternalRootCandidate {
    let root = root_id(root_identity, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let nesting_relation = root_id(6, NestingRelationId::from_normalized_identity);
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &effects::SelectedProviderPlanFacts::default(),
        )
        .expect("empty root service reach"),
        effects: [root_id(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation,
        acknowledgement_policy: Some(root_id(
            7,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: stack_demand(
                root,
                provider,
                nesting_relation,
                boundary,
                code,
                entry,
                EntryStack::Interrupted,
                2048,
            ),
            validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(53, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: fixed_fuel(),
            validation_receipt: root_id(51, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(52, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: [ComponentVersionPin {
            contract: root_id(8, ComponentContractId::from_normalized_identity),
            artifact: root_id(9, ComponentArtifactId::from_normalized_identity),
            provider: root_id(10, ComponentProviderId::from_normalized_identity),
            version: root_id(11, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    }
}

pub(super) fn selected_interrupt_completion_for(
    name: &str,
    provider_type: &str,
    machine_identity: &str,
    resolved_row: &[&str],
) -> effects::SelectedProviderPlanFacts {
    let requirement_identity = "InterruptCompletion::complete".to_owned();
    let plan = ProviderPlan {
        name: name.into(),
        provider_type: provider_type.into(),
        provider_type_package_identity: None,
        target: "x86_64-unknown-none".into(),
        schema: ServiceSchema {
            trait_name: "InterruptCompletion".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "complete".into(),
                requirement_owner: "InterruptCompletion".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement_identity.clone(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["InterruptCompletion".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "complete".into(),
            requirement_identity: requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: machine_identity.into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    };
    let identity = plan.report_fingerprint();
    effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected interrupt completion provider")
    .with_installation_reach_resolutions(vec![effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: identity,
        upper_bound: vec!["PortIo".into(), "MachineControl".into()],
        resolved_row: resolved_row
            .iter()
            .map(|service| (*service).to_owned())
            .collect(),
    }])
    .expect("provider reach refines the interrupt completion bound")
}

pub(super) fn selected_interrupt_completion() -> effects::SelectedProviderPlanFacts {
    selected_interrupt_completion_for(
        "LegacyPic",
        "LegacyPicController",
        "LegacyPicController::complete",
        &["PortIo"],
    )
}

pub(super) fn slot() -> RootSlotAuthority {
    RootSlotAuthority::from_admitted_owner(
        root_id(20, RootSlotId::from_normalized_identity),
        root_id(21, RootSlotOwnerId::from_normalized_identity),
    )
}

pub(crate) fn provider_execution(root: &ValidatedExternalRoot) -> ProviderExecution {
    provider_execution_for(root, 54)
}

/// Provider execution fixture with a caller-chosen identity so one ledger can
/// hold several roots whose executions stay distinct under the
/// invocation/acknowledgement replay keys.
pub(crate) fn provider_execution_for(
    root: &ValidatedExternalRoot,
    identity: u64,
) -> ProviderExecution {
    ProviderExecution::from_admitted_provider(
        root_id(identity, ProviderExecutionId::from_normalized_identity),
        root,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: root.boundary().call.entry_control,
                restored_state: root.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("admitted provider exit")
}

pub(super) fn entry_writer(entry: EntryStubId) -> PostHandoffWriterPlan {
    let target = RelocationTarget::Entry(entry);
    PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: constraints(),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "entry".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    }
}

pub(super) fn writer_site(base_address: u64) -> PlacementSite {
    PlacementSite {
        base_address,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    }
}
