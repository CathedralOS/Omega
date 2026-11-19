use super::validate_task_activations;
use effects::SelectedProviderPlanFacts;
use provider_plans::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod};
use language_core::{CarryCpu, CarryPolicy};
use target::TargetProfile;
use terminal_psi::*;

fn provider() -> SelectedProviderPlanFacts {
    let mut plan = ProviderPlan {
        name: "runtime".into(),
        provider_type: "TaskRunner".into(),
        provider_declaration: Some("SelectedRunner".into()),
        row_numeric_realizations: vec![None],
        target: "linux_x86_64".into(),
        ..Default::default()
    };
    let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    schema.trait_name = schema.identities.intern("TaskRuntime");
    schema.methods.push(ServiceMethod {
        name: "start".into(),
        requirement_owner: "TaskRuntime".into(),
        requirement_identity: schema.identities.intern("TaskRuntime::start"),
        ..Default::default()
    });
    let start = plan.identities.intern("TaskRuntime::start");
    plan.rows.push(ProviderPlanRow {
        method: "start".into(),
        requirement_identity: start,
        declared_requirement_identity: start,
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 0,
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: plan.identities.intern("TaskRunner::start"),
            machine_package_identity: None,
        },
    });
    SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap()
}

fn demands() -> (TerminalTaskActivationDemands, TerminalIdentityTable) {
    let mut table = TerminalIdentityTableBuilder::default();
    let start = table.intern("TaskRuntime::start");
    let owner = table.intern("TaskRuntime");
    let job = table.intern("Worker::run");
    let unit = table.intern("unit");
    let frame = TerminalTaskFrame {
        identity: 7,
        machine_callable_identity: job,
        entry: job,
        states: vec![],
        calls: vec![],
        unresolved_calls: vec![],
    };
    let demand = TerminalTaskActivationDemand {
        operation: TerminalTaskStartOperation::Start,
        report_fingerprint: 1,
        requirement_callable_identity: start,
        requirement_owner_path: owner,
        requirement_owner_package: None,
        requirement_in_scope: true,
        authored_requirement_identity: start,
        machine_callable_identity: job,
        entry: job,
        entry_report_fingerprint: 2,
        contract_report_fingerprint: 3,
        specialization_commitment: [4; 32],
        specialization_report_fingerprint: 4,
        may_suspend: true,
        may_block: false,
        cancellation_required: false,
        signature_arguments: vec![],
        signature_outcome: TerminalTaskSignatureValue {
            value_type: TerminalTaskValueType::Unit,
            normalized_identity: unit,
            is_mutable: false,
        },
        root_frame: frame.identity,
        frames: vec![frame],
        crossings: vec![TerminalTaskSuspensionCrossing {
            identity: 11,
            suspension_allowed: true,
            preserve_cpu: true,
            preserve_host_thread: false,
            live_carry: vec![TerminalTaskLiveCarry {
                place: 12,
                value_type: 13,
                storage: TerminalTaskCarryStorage::Local,
                claims: vec![14],
                effective: CarryPolicy {
                    cpu: CarryCpu::Origin,
                    ..CarryPolicy::PERMISSIVE
                },
            }],
        }],
        carry_obligations: TerminalTaskCarryObligations {
            preserve_cpu: true,
            preserve_host_thread: false,
        },
    };
    (
        TerminalTaskActivationDemands {
            demands: vec![demand],
            ..Default::default()
        },
        table.build(),
    )
}

#[test]
fn the_selected_task_runtime_must_cover_the_published_requirement() {
    let (input, identities) = demands();
    assert!(
        validate_task_activations(&input, &identities, &provider(), TargetProfile::LinuxX64)
            .is_ok()
    );
    let diagnostics = validate_task_activations(
        &input,
        &identities,
        &SelectedProviderPlanFacts::default(),
        TargetProfile::LinuxX64,
    )
    .unwrap_err();
    assert_eq!(
        diagnostics[0].message,
        "TaskRuntime boundary slot has no retained selected provider plan"
    );
}

#[test]
fn activation_checks_keep_the_frame_graph_and_suspension_obligations() {
    let (input, identities) = demands();
    let check = |input| {
        validate_task_activations(input, &identities, &provider(), TargetProfile::LinuxX64)
            .unwrap_err()[0]
            .message
            .clone()
    };
    let mut cyclic = input.clone();
    cyclic.demands[0].frames[0]
        .calls
        .push(TerminalTaskFrameCall { callee: 7 });
    assert!(check(&cyclic).contains("non-lowered call cycle"));
    let mut missing = input.clone();
    missing.demands[0].frames[0]
        .calls
        .push(TerminalTaskFrameCall { callee: 99 });
    assert!(check(&missing).contains("calls missing checked frame"));
    let mut uncaught = input.clone();
    uncaught.demands[0].carry_obligations.preserve_cpu = false;
    assert!(check(&uncaught).contains("understates a canonical crossing"));
    let mut duplicate = input;
    duplicate.demands[0].crossings[0].live_carry[0]
        .claims
        .push(14);
    assert!(check(&duplicate).contains("duplicate claim"));
}
