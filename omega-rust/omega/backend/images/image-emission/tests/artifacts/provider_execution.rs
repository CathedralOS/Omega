use super::{edge_id, identity, machine_id, operation_id, two_function_plan};
use image_emission::{
    InstallationError, ObjectError, build_installation_record, build_object_artifact,
    can_emit_executable_image, emit_direct_executable_image,
};
use machine_code::{
    BoundarySettlementRecord, MachineCodeFunction, MachineCodePlan, PortEffectRecord,
    SemanticCodeAttribution, SemanticCodeSite,
};
use semantic_vocabulary::{BoundaryMachineId, ClaimId, PlaceId, ProfileDecisionId, ServiceId};
use target::NativeTarget;
use target_operations::{
    CompletionClaimSource, MetadataOnlyPortRealization, ProviderExecutionBinding,
    ProviderPlanReportIdentity, TerminalPsiProvenance,
};
use terminal_psi::{CompletionReceipt, EntryClaim, StructuralAccess, StructuralArgument};

#[test]
fn privileged_effect_and_exact_provider_execution_survive_installation() {
    let port_operation = operation_id(1);
    let settlement_operation = operation_id(2);
    let service = ServiceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let provider_plan = ProviderPlanReportIdentity::new(7).unwrap();
    let provider_execution =
        ProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11).unwrap();
    let realization = MetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let mut bytes = x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    bytes.push(0xc3);
    let plan = MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![MachineCodeFunction {
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            parameter_abi: None,
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
            machine: machine_id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![port_operation, settlement_operation],
                edges: vec![edge_id(1)],
            },
            bytes,
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
            unit_continuations: Vec::new(),
            unit_affine_cleanup: None,
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(port_operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(settlement_operation),
                    operation_ordinal: 1,
                    code_offset: 27,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(1)),
                    operation_ordinal: 2,
                    code_offset: 27,
                    byte_count: 1,
                },
            ],
            port_effects: vec![PortEffectRecord {
                psi_operation: port_operation,
                service,
                port: 0x20,
                value: 0x20,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 27,
            }],
            boundary_settlements: vec![BoundarySettlementRecord {
                psi_operation: settlement_operation,
                boundary,
                execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    provider_execution.into(),
                ),
                realization: realization.into(),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: Vec::new(),
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: machine_code::BoundaryResultRecord::Unit,
                operation_ordinal: 1,
                code_offset: 27,
                byte_count: 0,
            }],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    };
    let artifact = build_object_artifact(&plan).expect("effect artifact");
    assert_eq!(artifact.semantic_code_attribution().len(), 3);
    assert_eq!(
        artifact.semantic_code_attribution()[1]
            .attribution
            .byte_count,
        0
    );
    assert_eq!(artifact.port_effects()[0].effect.service, service);
    assert_eq!(
        artifact.boundary_settlements()[0].settlement.realization,
        realization.into()
    );
    let image = emit_direct_executable_image(&artifact, 3).expect("effect image");
    assert_eq!(
        image.semantic_code_attribution(),
        artifact.semantic_code_attribution()
    );
    assert_eq!(
        build_installation_record(&image, ProfileDecisionId::new(1).unwrap()),
        Err(InstallationError::ProviderExecutionClosureMismatch)
    );

    let mut wrong_bytes = plan.clone();
    wrong_bytes.functions[0].bytes[0] ^= 1;
    assert!(matches!(
        build_object_artifact(&wrong_bytes),
        Err(ObjectError::PortEffectBytesMismatch { .. })
    ));
    let mut duplicate_completion_claim = plan.clone();
    duplicate_completion_claim.functions[0].boundary_settlements[0].arguments = vec![
        StructuralArgument {
            access: StructuralAccess::Owned,
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
        },
        StructuralArgument {
            access: StructuralAccess::Owned,
            place: PlaceId::new(2).unwrap(),
            path: Vec::new(),
        },
    ];
    duplicate_completion_claim.functions[0].boundary_settlements[0].completion_receipts = vec![
        CompletionReceipt {
            claim: ClaimId::new(1).unwrap(),
            argument_index: 0,
        },
        CompletionReceipt {
            claim: ClaimId::new(1).unwrap(),
            argument_index: 1,
        },
    ];
    duplicate_completion_claim.functions[0].boundary_settlements[0].completion_claim_sources =
        vec![CompletionClaimSource {
            claim: ClaimId::new(1).unwrap(),
            entry: Some(EntryClaim {
                claim: ClaimId::new(1).unwrap(),
                input: PlaceId::new(1).unwrap(),
                path: Vec::new(),
            }),
            content: None,
        }];
    assert_eq!(
        build_object_artifact(&duplicate_completion_claim),
        Err(ObjectError::InvalidCompletionReceiptCustody {
            machine: machine_id(1),
            operation: settlement_operation,
        })
    );
    let mut wrong_realization = plan;
    wrong_realization.functions[0].boundary_settlements[0].realization =
        MetadataOnlyPortRealization {
            value: 0x21,
            ..realization
        }
        .into();
    assert!(matches!(
        build_object_artifact(&wrong_realization),
        Err(ObjectError::BoundaryRealizationMismatch { .. })
    ));
}

#[test]
fn image_boundary_rejects_noncanonical_pointer_facts() {
    let mut plan = two_function_plan();
    plan.target.pointer_size = 4;
    assert!(!can_emit_executable_image(plan.target));
    let artifact = build_object_artifact(&plan).expect("owned artifact");
    assert!(emit_direct_executable_image(&artifact, 3).is_err());
}
