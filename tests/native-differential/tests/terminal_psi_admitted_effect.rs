use std::collections::BTreeSet;

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult, CompletionClaimSource,
};
use omega_abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStack,
    EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegister, MachineState,
    MachineStateSet, Preemption, ProviderExitRealization, RegisterSet, StackDomainRef,
    StateFootprintEvidence, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_entry_stack_realization,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, DestinationPreparationReceipt,
    DestinationPreparationReceiptId, EntrySetId, FinalValidationCertificate, FinalValidationId,
    InstallAuthority, InstallationAudience, InstallationReceipt, InstallationScopeId,
    InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, PreparedPostHandoffWriterDestination, RelocationSetId,
    WxEnforcement, admit_executable, install_validated, materialize_admitted_artifact,
    materialize_and_freeze, validate_final_placement,
};
use omega_external_roots::*;
use omega_image_emission::{
    InstallationError, build_installation_record,
    build_installation_record_with_provider_executions, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_program_entry_plan::lower_post_handoff_writer_fragment;
use omega_provider_planning::plans::{
    SelectedExternalRootProviderPlan, bind_external_root_post_handoff_writer_invocation,
};
use omega_target::NativeTarget;
use omega_target_operations::{
    BoundaryRealization, DirectPortReadU8Realization, MetadataOnlyPortRealization, TargetOperation,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ProfileDecisionId, ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId, MappingGrant, MappingGrantId, MappingId, MappingSourceMode,
    TranslationActivationFactId, TranslationActivationReceipt, TranslationInstallObligations,
    TranslationReleaseObligations, map_owned,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, EntryStubId, MaterializationWrite,
    PlacementAddressRange, PlacementConstraints, PlacementPhase, PlacementSite,
    PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};
use psi_terminal::{
    BindingRelevance, BoundaryMachineDeclaration, ClaimContentProjection, CompletionReceipt,
    ContentEntryClaim, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

#[test]
fn admitted_provider_execution_flows_through_lowering_and_installation() {
    let boundary_plan = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("boundary plan");
    let root = root_id(1, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let relation = root_id(6, NestingRelationId::from_normalized_identity);
    let entry = EntryStubId::from_normalized_identity(12).expect("entry stub");
    let installed_code = install_entry_artifact(entry);
    let stack_summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        EntryStack::ProviderSelected,
        64,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let stack_for = |boundary: &omega_calling_conventions::ValidatedBoundaryEntryPlan| {
        let realization = validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain: StackDomainRef::Interrupted,
                    occupancy_by_domain: Vec::new(),
                    nesting: Preemption::NotApplicable,
                }],
            }],
        })
        .expect("entry stack realization");
        let arrival_contexts = admit_opaque_arrival_context_set(
            &stack_summary,
            boundary,
            &installed_code,
            entry,
            vec![ArrivalContextId::new(1).expect("arrival context")],
            root_id(48, StackValidationReceiptId::from_normalized_identity),
        )
        .expect("arrival-context admission");
        let bound = bind_opaque_adapter_stack_realization(
            &stack_summary,
            boundary,
            &installed_code,
            entry,
            realization,
            arrival_contexts,
        )
        .expect("bound stack realization");
        compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: relation,
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("stack composition")
    };
    let stack = stack_for(&boundary_plan);
    let fuel_summary = FixedFuelProviderSummary::from_admitted_provider(
        root_id(30, ProviderFuelSummaryId::from_normalized_identity),
        provider,
        fuel_schedule(),
        3,
        BTreeSet::new(),
        root_id(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let fuel =
        compose_fixed_fuel(fuel_summary.identity, [&fuel_summary]).expect("fuel composition");
    let selected_providers = omega_effects::SelectedProviderPlanFacts::from_selection(&[], &[])
        .expect("empty provider closure");
    let mut exact_provider_plan = omega_effects::provider_plan::ProviderPlan::default();
    exact_provider_plan.schema.trait_name = "TimerRoot".into();
    exact_provider_plan.schema.methods.push(Default::default());
    let selected_method = &mut exact_provider_plan.schema.methods[0];
    selected_method.name = "tick".into();
    selected_method.requirement_owner = "TimerRoot".into();
    selected_method.requirement_identity = "TimerRoot::tick".into();
    selected_method.parameter_count = 1;
    selected_method.parameter_type_identities = vec!["Test::BoundaryWord".into()];
    selected_method.calling_plan_fingerprint = Some(boundary_plan.contract_fingerprint());
    let selected_provider = SelectedExternalRootProviderPlan::from_exact_plan(exact_provider_plan)
        .expect("exact selected provider plan");
    let candidate = ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: selected_provider.identity,
        provider_plan_digest: selected_provider.digest,
        requirement_identity: "TimerRoot::tick".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &selected_providers,
        )
        .expect("empty root service reach"),
        effects: [root_id(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation: relation,
        acknowledgement_policy: None,
        stack: StackResourceColumn {
            ceiling_bytes: 128,
            realization: stack,
            validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(53, FuelProvisionId::from_normalized_identity),
            ceiling_units: 8,
            realization: fuel,
            validation_receipt: root_id(51, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(52, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: BTreeSet::new(),
    };
    let result_boundary_plan = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(1, 1)),
        },
    )
    .expect("result boundary plan");
    let mut result_candidate = candidate.clone();
    result_candidate.requirement_identity = "KeyboardController::read_status".into();
    result_candidate.stack.realization = stack_for(&result_boundary_plan);
    let result_validated = validate_external_root(result_candidate, &result_boundary_plan)
        .expect("result root validation");
    let result_execution = ProviderExecution::from_admitted_provider(
        root_id(57, ProviderExecutionId::from_normalized_identity),
        &result_validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: result_validated.boundary().call.entry_control,
                restored_state: result_validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("result provider execution admission");
    let validated = validate_external_root(candidate, &boundary_plan).expect("root validation");
    let execution = ProviderExecution::from_admitted_provider(
        root_id(54, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("provider execution admission");

    let writer = entry_writer(entry);
    let lowered_writer = lower_post_handoff_writer_fragment(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        &writer,
    )
    .expect("AOT writer lowering");
    let mut wrong_exact_plan = selected_provider.exact_plan().clone();
    wrong_exact_plan.schema.trait_name = "WrongTimerRoot".into();
    let wrong_selected_provider =
        SelectedExternalRootProviderPlan::from_exact_plan(wrong_exact_plan)
            .expect("different exact selected provider plan");
    let mut wrong_provider_bytes = [0u8; 16];
    let error = wrong_selected_provider
        .prepare_post_handoff_entry_writer(
            lowered_writer.clone(),
            &execution,
            &installed_code,
            &writer,
            prepared_writer_destination(0x8000, &mut wrong_provider_bytes),
        )
        .expect_err("selected provider closure drift must reject");
    assert!(error.diagnostic().0.contains("selected provider plan"));
    let mut drifted_selected_schema = selected_provider.clone();
    drifted_selected_schema.schema.methods[0].parameter_count = 2;
    drifted_selected_schema.schema.methods[0]
        .parameter_type_identities
        .push("Test::ExtraBoundaryWord".into());
    let drifted_selected_snapshot = drifted_selected_schema.clone();
    let mut drifted_schema_bytes = [0u8; 16];
    let error = drifted_selected_schema
        .prepare_post_handoff_entry_writer(
            lowered_writer.clone(),
            &execution,
            &installed_code,
            &writer,
            prepared_writer_destination(0x8000, &mut drifted_schema_bytes),
        )
        .expect_err("selected source schema drift must reject before provider preparation");
    assert!(error.diagnostic().0.contains("selected provider plan"));
    let (returned_selected, returned_lowered, returned_destination) = error.into_parts();
    assert_eq!(returned_selected, drifted_selected_snapshot);
    assert_eq!(returned_lowered, lowered_writer);
    assert_eq!(returned_destination.site(), writer_site(0x8000));
    assert_eq!(returned_destination.len(), 16);
    let mut wrong_writer_bytes = [0u8; 16];
    assert!(
        selected_provider
            .clone()
            .prepare_post_handoff_entry_writer(
                lowered_writer.clone(),
                &execution,
                &installed_code,
                &entry_writer(EntryStubId::from_normalized_identity(13).unwrap()),
                prepared_writer_destination(0x8000, &mut wrong_writer_bytes),
            )
            .expect_err("writer entry drift must reject")
            .diagnostic()
            .0
            .contains("exact provider writer plan")
    );
    let selected_provider_snapshot = selected_provider.clone();
    let lowered_writer_snapshot = lowered_writer.clone();
    let mut drifted_placement_bytes = [0u8; 16];
    let error = selected_provider
        .clone()
        .prepare_post_handoff_entry_writer(
            lowered_writer.clone(),
            &execution,
            &installed_code,
            &writer,
            prepared_writer_destination(0x8008, &mut drifted_placement_bytes),
        )
        .expect_err("destination placement drift must reject");
    assert!(error.diagnostic().0.contains("align"));
    let (returned_selected, returned_lowered, returned_destination) = error.into_parts();
    assert_eq!(returned_selected, selected_provider_snapshot);
    assert_eq!(returned_lowered, lowered_writer_snapshot);
    assert_eq!(returned_destination.site(), writer_site(0x8008));
    assert_eq!(returned_destination.len(), 16);
    let drifted_lowering = lower_post_handoff_writer_fragment(
        NativeTarget::linux_x64(),
        MachineRegister::X86Rdi,
        &entry_writer(EntryStubId::from_normalized_identity(13).unwrap()),
    )
    .expect("same reusable geometry with a different symbolic entry");
    let drifted_lowering_snapshot = drifted_lowering.clone();
    let selected_provider_snapshot = selected_provider.clone();
    let mut drifted_lowering_bytes = [0u8; 16];
    let error = selected_provider
        .prepare_post_handoff_entry_writer(
            drifted_lowering,
            &execution,
            &installed_code,
            &writer,
            prepared_writer_destination(0x8000, &mut drifted_lowering_bytes),
        )
        .expect_err("lowered writer drift must reject before provider preparation");
    assert!(error.diagnostic().0.contains("exact provider writer plan"));
    let (selected_provider, returned_lowering, destination) = error.into_parts();
    assert_eq!(selected_provider, selected_provider_snapshot);
    assert_eq!(returned_lowering, drifted_lowering_snapshot);
    assert_eq!(destination.site(), writer_site(0x8000));
    assert_eq!(destination.len(), 16);
    let colliding_code = install_entry_artifact(
        EntryStubId::from_normalized_identity(13).expect("colliding artifact entry"),
    );
    let error = selected_provider
        .prepare_post_handoff_entry_writer(
            lowered_writer,
            &execution,
            &colliding_code,
            &writer,
            destination,
        )
        .expect_err("installed resolver substitution must reject before source resolution");
    assert!(error.diagnostic().0.contains("admitted installed artifact"));
    let (selected_provider, lowered_writer, destination) = error.into_parts();
    let preparation = selected_provider
        .prepare_post_handoff_entry_writer(
            lowered_writer,
            &execution,
            &installed_code,
            &writer,
            destination,
        )
        .expect("selected provider closure should prepare its exact AOT writer");
    assert_eq!(
        preparation.lowered().invocation(),
        preparation.prepared().invocation()
    );
    assert_eq!(preparation.destination().site(), writer_site(0x8000));
    assert_eq!(preparation.destination().len(), 16);
    let bound_writer = bind_external_root_post_handoff_writer_invocation(preparation)
        .expect("lowered fragment, selected source schema, and provider preparation should bind");
    assert_eq!(
        bound_writer.prepared().provider_execution(),
        execution.binding()
    );
    assert_eq!(
        bound_writer.selected_provider().schema.methods[0].requirement_identity,
        "TimerRoot::tick"
    );
    assert_eq!(
        bound_writer.installed_code().identity(),
        installed_code.identity()
    );
    let bound_lowered = bound_writer.lowered().clone();
    let bound_provider = bound_writer.prepared().provider_execution();
    let written = bound_writer
        .execute()
        .expect("bound writer and its exact destination remain usable");
    let substitute_installed_code = install_entry_artifact(entry);
    let error = written
        .into_validated_for_consumer(&substitute_installed_code)
        .expect_err("an equal-looking installed realization cannot replace the retained borrow");
    assert!(error.diagnostic().0.contains("exact installed realization"));
    let written = error.into_written();
    let validated_written = written
        .into_validated_for_consumer(&installed_code)
        .expect("written bound carrier replays its retained installed realization");
    assert_eq!(
        validated_written.installed_code().identity(),
        installed_code.identity()
    );
    assert_eq!(validated_written.provider_execution(), bound_provider);
    assert_eq!(validated_written.selected_entry(), entry);
    assert_eq!(validated_written.selected_entry_source_slot(), 0);
    assert_eq!(
        validated_written.selected_requirement_identity(),
        "TimerRoot::tick"
    );
    assert_eq!(
        u64::from_le_bytes(validated_written.bytes()[..8].try_into().unwrap()),
        0x1010
    );
    let bound_writer = validated_written
        .recover_for_retry()
        .expect("exact written carrier returns to its sealed retry state");
    assert_eq!(bound_writer.lowered(), &bound_lowered);
    assert_eq!(bound_writer.prepared().provider_execution(), bound_provider);
    assert_eq!(bound_writer.destination().site(), writer_site(0x8000));
    assert_eq!(bound_writer.destination().len(), 16);
    let written = bound_writer
        .execute()
        .expect("recovered writer and destination execute again");
    let written = written
        .into_validated_for_consumer(&installed_code)
        .expect("recovered execution validates before its bytes or parts are exposed");
    let (retained_selected_provider, retained_lowered, retained_installed, written) =
        written.into_parts();
    assert_eq!(
        retained_selected_provider.schema.methods[0].requirement_identity,
        "TimerRoot::tick"
    );
    assert_eq!(retained_lowered, bound_lowered);
    assert_eq!(retained_installed.identity(), installed_code.identity());
    let (
        provider_execution,
        provider_execution_evidence,
        root_evidence,
        selected_entry,
        selected_entry_source_slot,
        architecture,
        invocation,
        _writer,
        written,
    ) = written.into_parts();
    assert_eq!(provider_execution, bound_provider);
    assert_eq!(provider_execution_evidence.binding(), bound_provider);
    assert_eq!(
        root_evidence.candidate().requirement_identity,
        "TimerRoot::tick"
    );
    assert_eq!(selected_entry, entry);
    assert_eq!(selected_entry_source_slot, 0);
    assert_eq!(architecture, installed_code.architecture());
    assert_eq!(&invocation, retained_lowered.invocation());
    written
        .validate_for_consumer(&colliding_code)
        .expect_err("retained installation validation must reject artifact identity drift");
    written
        .validate_for_consumer(&installed_code)
        .expect("retained installation validation supports corrected borrowed retry");
    let (_mapping, _receipt, _site, _bytes) = written.into_parts();

    let machine = MachineId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let port_operation = OperationId::new(1).unwrap();
    let settlement_operation = OperationId::new(2).unwrap();
    let service = ServiceId::new(1).unwrap();
    let realization = MetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let element_type = StructuralTypeId::new(1).unwrap();
    let array_type = StructuralTypeId::new(2).unwrap();
    let custody_place = PlaceId::new(1).unwrap();
    let boundary_place = PlaceId::new(2).unwrap();
    let settlement_arguments = vec![StructuralArgument {
        access: StructuralAccess::Owned,
        place: custody_place,
        path: vec![StructuralPathSegment::FixedIndex(3)],
    }];
    let abstract_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
        },
        entry: machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: element_type,
                identity: "Acknowledgement".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: array_type,
                identity: "Acknowledgements".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 5,
                },
            },
        ],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "TimerRoot::tick".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: boundary_place,
                position: 0,
                is_self: false,
                structural_type: element_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
            }],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: custody_place,
                position: 0,
                is_self: false,
                structural_type: array_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
            }],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service],
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::PortWrite {
                    psi_operation: port_operation,
                    service,
                    port: 0x20,
                    value: 0x20,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: None,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: settlement_arguments.clone(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let target = lower_to_target_operations_with_provider_executions(
        &abstract_plan,
        NativeTarget::linux_x64(),
        &[AdmittedBoundarySettlement {
            boundary,
            provider_execution: &execution,
            realization: realization.into(),
        }],
    )
    .expect("admitted effect lowering");
    let assigned = assign_registers(&target).expect("register assignment");
    let machine_code = emit_machine_code(&assigned).expect("machine emission");
    assert_eq!(
        machine_code.functions[0].boundary_settlements[0].arguments,
        settlement_arguments
    );
    let object = build_object_artifact(&machine_code).expect("object artifact");
    assert_eq!(
        object.boundary_settlements()[0].settlement.arguments,
        settlement_arguments
    );
    let image = emit_executable_image(&object, 3).expect("executable image");
    assert_eq!(
        image.boundary_settlements()[0].settlement.arguments,
        settlement_arguments
    );
    let profile = ProfileDecisionId::new(1).unwrap();

    assert_eq!(
        build_installation_record(&image, profile),
        Err(InstallationError::ProviderExecutionClosureMismatch)
    );
    assert_eq!(
        build_installation_record_with_provider_executions(
            &image,
            profile,
            [&execution, &execution],
        ),
        Err(InstallationError::DuplicateProviderExecution)
    );
    let installation =
        build_installation_record_with_provider_executions(&image, profile, [&execution])
            .expect("admitted installation composition");
    assert_eq!(
        installation.selected_provider_plans(),
        [omega_image_emission::SelectedProviderPlanIdentity::new(
            execution.provider_plan().normalized_identity()
        )
        .unwrap()]
    );
    assert_eq!(installation.fuel_attribution(), image.fuel_attribution());
    validate_installation_record(&installation, &image).expect("image binding");
    let encoded = encode_installation_record(&installation).expect("installation bytes");
    let decoded = decode_installation_record(&encoded).expect("installation decoding");
    assert_eq!(
        decoded.boundary_settlements()[0].settlement.arguments,
        settlement_arguments
    );
    assert_eq!(decoded, installation);

    let mut argument_prefix = Vec::new();
    argument_prefix.extend_from_slice(&custody_place.get().to_le_bytes());
    argument_prefix.push(1);
    argument_prefix.extend_from_slice(&[0; 3]);
    argument_prefix.extend_from_slice(&1_u32.to_le_bytes());
    argument_prefix.push(2);
    argument_prefix.extend_from_slice(&[0; 3]);
    let argument_offset = encoded
        .windows(argument_prefix.len())
        .rposition(|window| window == argument_prefix)
        .expect("encoded structural argument");
    let mut malformed = encoded;
    malformed[argument_offset + 16] = 0xff;
    assert_eq!(
        decode_installation_record(&malformed),
        Err(InstallationError::InvalidSettlementArgumentPathTag(0xff))
    );

    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: u8_type,
    };
    let direct_realization = DirectPortReadU8Realization {
        service,
        port: 0x60,
    };
    let direct_claim = ClaimId::new(1).unwrap();
    let direct_content_source = CompletionClaimSource {
        claim: direct_claim,
        entry: None,
        content: Some(ContentEntryClaim {
            claim: direct_claim,
            input: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: custody_place,
                segments: vec![ContentPlaceSegment::Field("payload".into())],
            },
            projections: vec![ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(1).expect("content domain"),
                    projection_fingerprint: 0x63f4_9912,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::CountedQuantity,
                    parameter: "KeyboardStatusBytes".into(),
                },
            }],
        }),
    };
    let direct_arguments = vec![StructuralArgument {
        access: StructuralAccess::Owned,
        place: custody_place,
        path: Vec::new(),
    }];
    let direct_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([10; 32]),
        },
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: element_type,
            identity: "Acknowledgement".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "KeyboardController::read_status".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: boundary_place,
                position: 0,
                is_self: false,
                structural_type: element_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            result: Some(u8_type),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: custody_place,
                position: 0,
                is_self: false,
                structural_type: element_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            result: AbstractFunctionResult::Scalar(result),
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service],
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: Some(result),
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: direct_arguments.clone(),
                    completion_claim_sources: vec![direct_content_source.clone()],
                    completion_receipts: vec![CompletionReceipt {
                        claim: direct_claim,
                        argument_index: 0,
                    }],
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    result: result.value,
                    value: result.value,
                    scalar_type: result.scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let direct_settlement = AdmittedBoundarySettlement {
        boundary,
        provider_execution: &result_execution,
        realization: BoundaryRealization::DirectPortReadU8(direct_realization),
    };
    let direct_target = lower_to_target_operations_with_provider_executions(
        &direct_plan,
        NativeTarget::linux_x64(),
        &[direct_settlement],
    )
    .expect("direct result lowering");
    let TargetOperation::ReturnBoundaryPortReadU8 {
        completion_claim_sources,
        call_plan: direct_call_plan,
        ..
    } = &direct_target.functions[0].operation
    else {
        panic!("direct result target operation")
    };
    assert_eq!(
        completion_claim_sources,
        std::slice::from_ref(&direct_content_source)
    );
    assert!(
        lower_to_target_operations_with_provider_executions(
            &direct_plan,
            NativeTarget::linux_arm64(),
            &[direct_settlement],
        )
        .is_err()
    );
    let direct_assigned = assign_registers(&direct_target).expect("direct result assignment");
    let direct_machine = emit_machine_code(&direct_assigned).expect("direct result emission");
    let mut expected_bytes = omega_x86_encoding::encode_immediate_port_read_u8(0x60).to_vec();
    expected_bytes.push(0xc3);
    assert_eq!(direct_machine.functions[0].bytes, expected_bytes);
    assert_eq!(
        direct_machine.functions[0].boundary_settlements[0].arguments,
        direct_arguments
    );
    assert_eq!(
        direct_machine.functions[0].boundary_settlements[0].completion_receipts,
        [CompletionReceipt {
            claim: direct_claim,
            argument_index: 0,
        }]
    );
    assert_eq!(
        direct_machine.functions[0].boundary_settlements[0].completion_claim_sources,
        completion_claim_sources.as_slice()
    );
    let direct_provider_custody =
        &direct_machine.functions[0].boundary_settlements[0].completion_provider_custody;
    assert_eq!(direct_provider_custody.len(), 1);
    assert_eq!(direct_provider_custody[0].source, direct_content_source);
    assert_eq!(
        direct_provider_custody[0].receipt,
        CompletionReceipt {
            claim: direct_claim,
            argument_index: 0,
        }
    );
    assert_eq!(
        direct_provider_custody[0].provider_execution,
        direct_machine.functions[0].boundary_settlements[0].provider_execution
    );
    let mut substituted_provider_custody = direct_machine.clone();
    substituted_provider_custody.functions[0].boundary_settlements[0]
        .completion_provider_custody[0]
        .provider_execution
        .provider_plan ^= 1;
    assert!(matches!(
        build_object_artifact(&substituted_provider_custody),
        Err(omega_image_emission::ObjectError::InvalidCompletionProviderCustody { .. })
    ));
    let mut dropped_content_source = direct_machine.clone();
    dropped_content_source.functions[0].boundary_settlements[0].completion_claim_sources[0]
        .content = None;
    assert!(matches!(
        build_object_artifact(&dropped_content_source),
        Err(omega_image_emission::ObjectError::InvalidCompletionReceiptCustody { .. })
    ));
    let native_result = direct_machine.functions[0].boundary_settlements[0]
        .native_result
        .as_ref()
        .expect("native result evidence");
    assert_eq!(native_result.value, result.value);
    assert_eq!(native_result.scalar_type, result.scalar_type);
    assert_eq!(
        native_result.return_edge,
        EdgeId::new(1).expect("return edge")
    );
    assert_eq!(
        &native_result.placement,
        direct_call_plan.result.as_ref().unwrap()
    );
    let mut missing_native_result = direct_machine.clone();
    missing_native_result.functions[0].boundary_settlements[0].native_result = None;
    assert!(matches!(
        build_object_artifact(&missing_native_result),
        Err(omega_image_emission::ObjectError::BoundaryRealizationMismatch { .. })
    ));
    let mut wrong_return_edge = direct_machine.clone();
    wrong_return_edge.functions[0].boundary_settlements[0]
        .native_result
        .as_mut()
        .expect("native result")
        .return_edge = EdgeId::new(2).expect("wrong return edge");
    assert!(matches!(
        build_object_artifact(&wrong_return_edge),
        Err(omega_image_emission::ObjectError::BoundaryRealizationMismatch { .. })
    ));
    let direct_object = build_object_artifact(&direct_machine).expect("direct result object");
    let direct_image = emit_executable_image(&direct_object, 3).expect("direct result image");
    let direct_installation = build_installation_record_with_provider_executions(
        &direct_image,
        profile,
        [&result_execution],
    )
    .expect("direct result installation");
    assert_eq!(
        direct_installation.boundary_settlements()[0]
            .settlement
            .completion_claim_sources
            .as_slice(),
        completion_claim_sources.as_slice()
    );
    assert_eq!(
        direct_installation.boundary_settlements()[0]
            .settlement
            .completion_provider_custody,
        direct_provider_custody.as_slice()
    );
    assert_eq!(
        &direct_installation.boundary_settlements()[0]
            .settlement
            .native_result,
        &direct_machine.functions[0].boundary_settlements[0].native_result
    );
    let direct_encoded =
        encode_installation_record(&direct_installation).expect("direct result encoding");
    let direct_decoded =
        decode_installation_record(&direct_encoded).expect("direct result installation decoding");
    assert_eq!(direct_decoded, direct_installation);

    let mut corrupted_direct_machine = direct_machine;
    corrupted_direct_machine.functions[0].bytes[12] ^= 1;
    assert!(build_object_artifact(&corrupted_direct_machine).is_err());
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("current fuel schedule")
}

fn entry_writer(entry: EntryStubId) -> PostHandoffWriterPlan {
    let target = RelocationTarget::Entry(entry);
    PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x8000, 0x9000).unwrap()),
            16,
            PlacementPhase::PostHandoff,
            None,
            None,
        )
        .unwrap(),
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

fn writer_site(base_address: u64) -> PlacementSite {
    PlacementSite {
        base_address,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    }
}

fn installation_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_identity<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn activated_writer_mapping(base: u64, length: u64) -> psi_extents::MappedExtent<'static> {
    let source_space = extent_identity(200, AddressSpaceId::from_normalized_identity);
    let destination_space = extent_identity(201, AddressSpaceId::from_normalized_identity);
    let source_rights = ExtentRights::from_normalized_identities([extent_identity(
        202,
        ExtentRightId::from_normalized_identity,
    )]);
    let destination_rights = ExtentRights::from_normalized_identities([extent_identity(
        203,
        ExtentRightId::from_normalized_identity,
    )]);
    let writer_rights = ExtentRights::from_normalized_identities([extent_identity(
        204,
        ExtentRightId::from_normalized_identity,
    )]);
    let source = ExtentRootGrant::from_admitted_provider(
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223,
        ])
        .unwrap(),
        extent_identity(205, ExtentLineageId::from_normalized_identity),
        source_space,
        source_rights.clone(),
        extent_identity(206, ExtentProvenanceId::from_normalized_identity),
        extent_identity(207, MappingEraId::from_normalized_identity),
    )
    .mint(0x20_000, length)
    .unwrap();
    let destination = ExtentRootGrant::from_admitted_provider(
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243,
        ])
        .unwrap(),
        extent_identity(208, ExtentLineageId::from_normalized_identity),
        destination_space,
        destination_rights.clone(),
        extent_identity(209, ExtentProvenanceId::from_normalized_identity),
        extent_identity(210, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .unwrap();
    let activation = extent_identity(244, TranslationActivationFactId::from_normalized_identity);
    let grant = MappingGrant::from_admitted_provider(
        extent_identity(245, MappingGrantId::from_normalized_identity),
        MappingSourceMode::Owned,
        source_space,
        destination_space,
        source_rights,
        destination_rights,
        writer_rights,
        extent_identity(246, ExtentProvenanceId::from_normalized_identity),
        extent_identity(247, MappingEraId::from_normalized_identity),
        TranslationInstallObligations::from_normalized_facts([activation]),
        TranslationReleaseObligations::default(),
    );
    let pending = map_owned(
        source,
        destination,
        extent_identity(248, MappingId::from_normalized_identity),
        &grant,
    )
    .unwrap();
    let receipt = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [activation],
    );
    pending.complete(receipt).unwrap()
}

fn prepared_writer_destination<'bytes>(
    base: u64,
    bytes: &'bytes mut [u8],
) -> PreparedPostHandoffWriterDestination<'static, 'bytes> {
    let mapping = activated_writer_mapping(base, bytes.len() as u64);
    let receipt = DestinationPreparationReceipt::from_admitted_provider(
        installation_id(
            210,
            DestinationPreparationReceiptId::from_normalized_identity,
        ),
        &mapping.receipt_context(),
        ExtentRights::from_normalized_identities([extent_identity(
            204,
            ExtentRightId::from_normalized_identity,
        )]),
        true,
        true,
    );
    PreparedPostHandoffWriterDestination::claim(mapping, receipt, writer_site(base), bytes)
        .expect("exact activated, pinned, writable, unpublished destination")
}

fn install_entry_artifact(entry: EntryStubId) -> InstalledCode {
    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    let constraints = PlacementConstraints::new(
        None,
        16,
        PlacementPhase::PostHandoff,
        None,
        Some(ArtifactInstallationScopeId::from_normalized_identity(61).unwrap()),
    )
    .unwrap();
    let contracts = install_id(120, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(121, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(100, ArtifactId::from_normalized_identity),
        omega_target::Architecture::X86_64,
        vec![0; 64],
        contracts,
        footprint,
        install_id(122, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(123, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(124, RelocationSetId::from_normalized_identity),
        Vec::new(),
        omega_executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"admitted-effect-machine-contracts-v1",
            footprint,
            b"admitted-effect-machine-footprint-v1",
            constraints
                .machine_regime()
                .map(|regime| (regime, b"admitted-effect-machine-regime-v1".as_slice())),
            constraints
                .installation_scope()
                .map(|scope| (scope, b"admitted-effect-installation-scope-v1".as_slice())),
        ),
    )
    .unwrap();
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(125, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .unwrap();
    let rights = ExtentRights::from_normalized_identities([extent_id(
        130,
        ExtentRightId::from_normalized_identity,
    )]);
    let issuance = psi_extents::ExtentProviderIssuance::from_normalized_identities([
        140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152,
    ])
    .unwrap();
    let extent = ExtentRootGrant::from_admitted_provider(
        issuance,
        extent_id(160, ExtentLineageId::from_normalized_identity),
        extent_id(161, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(162, ExtentProvenanceId::from_normalized_identity),
        extent_id(163, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .unwrap();
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(170, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(61).unwrap(),
            ),
        },
    )
    .claim(extent)
    .unwrap();
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None).unwrap();
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(171, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .unwrap();
    let certificate = FinalValidationCertificate::from_validator(
        install_id(172, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).unwrap();
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(173, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).unwrap()
}
