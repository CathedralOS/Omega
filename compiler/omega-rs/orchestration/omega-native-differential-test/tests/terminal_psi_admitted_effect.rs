use std::collections::BTreeSet;

use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryStack, MachineRegister, MachineState, MachineStateSet,
    ProviderExitRealization, RegisterSet, StateFootprintEvidence, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use omega_external_roots::*;
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
    TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_terminal_image_emission::{
    TerminalInstallationError, build_terminal_installation_record,
    build_terminal_installation_record_with_provider_executions, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    encode_terminal_installation_record, validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_target_operations::TerminalMetadataOnlyPortRealization;
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, MachineId, OperationId,
    ProfileDecisionId, ServiceId,
};
use psi_layout_plans::EntryStubId;
use psi_terminal::{
    BoundaryMachineDeclaration, SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker,
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
    let stack_summary = ProviderStackSummary {
        root,
        provider,
        stack: EntryStack::ProviderSelected,
        local_wcsu_bytes: 64,
        wcsu_alignment: 16,
        validation_receipt: root_id(49, StackValidationReceiptId::from_normalized_identity),
    };
    let stack = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&stack_summary],
    )
    .expect("stack composition")
    .demand(root)
    .expect("root stack demand")
    .clone();
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
    let candidate = ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
        requirement_identity: "TimerRoot::tick".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
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

    let machine = MachineId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let port_operation = OperationId::new(1).unwrap();
    let settlement_operation = OperationId::new(2).unwrap();
    let service = ServiceId::new(1).unwrap();
    let realization = TerminalMetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let abstract_plan = TerminalAbstractOperationPlan {
        terminal_psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "TimerRoot::tick".into(),
            attachment: None,
            structural_parameters: Vec::new(),
            requires: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        functions: vec![TerminalAbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalAbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service],
            block_entries: vec![TerminalAbstractBlockEntry {
                block: BlockId::new(1).unwrap(),
                operation_offset: 0,
            }],
            operations: vec![
                TerminalAbstractOperation::PortWrite {
                    psi_operation: port_operation,
                    service,
                    port: 0x20,
                    value: 0x20,
                },
                TerminalAbstractOperation::BoundaryCallUnit {
                    psi_operation: settlement_operation,
                    boundary,
                    structural_arguments: Vec::new(),
                    claim_settlements: Vec::new(),
                },
                TerminalAbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                },
            ],
        }],
    };
    let target = lower_to_target_operations_with_provider_executions(
        &abstract_plan,
        NativeTarget::linux_x64(),
        &[AdmittedTerminalBoundarySettlement {
            boundary,
            provider_execution: &execution,
            realization,
        }],
    )
    .expect("admitted effect lowering");
    let assigned = assign_registers(&target).expect("register assignment");
    let machine_code = emit_machine_code(&assigned).expect("machine emission");
    let object = build_terminal_object_artifact(&machine_code).expect("object artifact");
    let image = emit_terminal_executable_image(&object, 3).expect("executable image");
    let profile = ProfileDecisionId::new(1).unwrap();

    assert_eq!(
        build_terminal_installation_record(&image, profile),
        Err(TerminalInstallationError::ProviderExecutionClosureMismatch)
    );
    assert_eq!(
        build_terminal_installation_record_with_provider_executions(
            &image,
            profile,
            [&execution, &execution],
        ),
        Err(TerminalInstallationError::DuplicateProviderExecution)
    );
    let installation =
        build_terminal_installation_record_with_provider_executions(&image, profile, [&execution])
            .expect("admitted installation composition");
    assert_eq!(
        installation.selected_provider_plans(),
        [omega_terminal_image_emission::SelectedProviderPlanIdentity::new(55).unwrap()]
    );
    assert_eq!(installation.fuel_attribution(), image.fuel_attribution());
    validate_terminal_installation_record(&installation, &image).expect("image binding");
    let encoded = encode_terminal_installation_record(&installation).expect("installation bytes");
    assert_eq!(
        decode_terminal_installation_record(&encoded),
        Ok(installation)
    );
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("current fuel schedule")
}
