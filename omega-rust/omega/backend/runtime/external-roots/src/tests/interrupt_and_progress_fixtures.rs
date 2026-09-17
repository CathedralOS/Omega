//! Interrupt boundary, candidate, table and progress installation fixtures.

use super::InterruptTableMemberFixture;
use super::boundary_fixtures::{
    candidate_for_code, candidate_for_code_with_root, selected_interrupt_completion, stack_demand,
    stack_demand_input,
};
use super::installed_code_fixtures::{installed_code, root_id};
use crate::{
    AcknowledgementPolicyId, AdmittedProgressProfileEstablishment, ExternalRootCandidate,
    ExternalRootEntryClaim, ExternalRootResultClaim, InstalledExternalRoot,
    InstalledProviderOccurrenceId, InstalledRootLedger, InterruptAcknowledgementId,
    InterruptEntryReceipt, InterruptEntryReceiptId, InterruptInvocationId, InterruptMaskControlId,
    InterruptMaskStateId, InterruptPreemptionReport, ProgressProfileEstablishmentAttestation,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrenceInstallationReceiptId,
    ProviderOccurrencePlanBinding, ProviderPlanId, ResolvedRootServiceReach, StackNestingRelation,
    ValidatedExternalRoot, compose_bound_entry_stack_epochs, validate_external_root,
};
use calling_conventions::{
    ArrivalContextId, BoundaryEntryPlan, EntryControl, EntryStack, MachineRegister,
    ValidatedBoundaryEntryPlan,
};
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegime, MachineState, MachineStateSet, Preemption,
    RegisterSet, StatePlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan,
};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceSchema,
};
use effects::{
    CheckedComponentProgressDemand, ComponentProgressManifest, SelectedProviderPlanFacts,
};
use executable_installation::InstalledCode;
use layout_plans::EntryStubId;
use std::collections::BTreeSet;

pub(super) fn interrupt_boundary() -> ValidatedBoundaryEntryPlan {
    interrupt_boundary_on(EntryStack::Dedicated { class: 1 })
}

/// Interrupt-return boundary fixture arriving on `stack`. Table members use
/// distinct dedicated classes; the ordinary interrupt tests keep class 1.
pub(crate) fn interrupt_boundary_on(stack: EntryStack) -> ValidatedBoundaryEntryPlan {
    interrupt_boundary_shaped(stack, Preemption::Masked)
}

/// Interrupt-return boundary fixture with a caller-chosen stack disposition
/// and declared nesting ceiling.
pub(crate) fn interrupt_boundary_shaped(
    stack: EntryStack,
    preemption: Preemption,
) -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary x86 plan");
    let mut call = ordinary.plan().call.clone();
    call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    call.entry_control = EntryControl::InterruptReturn;
    let interrupted_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::VectorRegisters,
    ]);
    let saved_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state,
                saved_state,
                restored_state: saved_state,
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack,
                preemption,
            },
        },
        &signature,
    )
    .expect("interrupt boundary")
}

pub(super) fn interrupt_candidate(entry: EntryStubId) -> ExternalRootCandidate {
    interrupt_candidate_for_code(entry, &installed_code(1, entry))
}

pub(super) fn interrupt_candidate_for_code(
    entry: EntryStubId,
    code: &InstalledCode,
) -> ExternalRootCandidate {
    let selected_completion = selected_interrupt_completion();
    interrupt_candidate_for_code_with_completion(entry, code, &selected_completion)
}

pub(super) fn interrupt_candidate_for_code_with_completion(
    entry: EntryStubId,
    code: &InstalledCode,
    selected_completion: &SelectedProviderPlanFacts,
) -> ExternalRootCandidate {
    let mut candidate = candidate_for_code(entry, code);
    candidate.requirement_identity = "TimerRoot::tick".into();
    candidate.entry_claims = vec![ExternalRootEntryClaim {
        parameter_index: 0,
        domain: "InterruptAcknowledgement::Pending".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }];
    candidate.acknowledgement_parameter_index = Some(0);
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    });
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["InterruptCompletion::complete".into()],
        selected_completion,
    )
    .expect("selected completion closes the installed interrupt reach");
    let boundary = interrupt_boundary();
    candidate.stack.realization = stack_demand(
        candidate.identity,
        candidate.provider,
        candidate.nesting_relation,
        &boundary,
        code,
        entry,
        EntryStack::Dedicated { class: 1 },
        2048,
    );
    candidate
}

/// Interrupt-root candidate shaped for one interrupt-table member: `stack` is
/// the declared arrival disposition and `acknowledged` selects whether the
/// root mints a settle-able `Pending` acknowledgement. The stack-realization
/// column is assigned by the caller so several members can share the one
/// artifact-wide composition the ledger requires.
pub(crate) fn interrupt_candidate_shaped(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
    acknowledged: bool,
) -> ExternalRootCandidate {
    let mut candidate = candidate_for_code_with_root(entry, code, root_identity);
    candidate.requirement_identity = if acknowledged {
        "TimerRoot::tick".into()
    } else {
        "FatalExceptionRoot::enter".into()
    };
    candidate.entry_claims = if acknowledged {
        vec![ExternalRootEntryClaim {
            parameter_index: 0,
            domain: "InterruptAcknowledgement::Pending".into(),
            effective_carry: language_semantics::CarryPolicy::STRICT,
        }]
    } else {
        Vec::new()
    };
    candidate.acknowledgement_parameter_index = acknowledged.then_some(0);
    candidate.acknowledgement_policy =
        acknowledged.then(|| root_id(7, AcknowledgementPolicyId::from_normalized_identity));
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    });
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["InterruptCompletion::complete".into()],
        &selected_interrupt_completion(),
    )
    .expect("selected completion closes the installed interrupt reach");
    candidate
}

/// Build validated interrupt roots sharing the one artifact-wide stack
/// composition `InstalledRootLedger::install` requires. Each member's own
/// bound input enters the single composition; assigning any per-member
/// realization would be rejected as a foreign nesting aggregate.
pub(crate) fn interrupt_table_candidates(
    code: &InstalledCode,
    members: &[InterruptTableMemberFixture],
) -> Vec<(ValidatedExternalRoot, ValidatedBoundaryEntryPlan)> {
    let mut inputs = Vec::with_capacity(members.len());
    let mut shaped = Vec::with_capacity(members.len());
    for member in members {
        let stack = EntryStack::Dedicated {
            class: member.stack_class,
        };
        let boundary = interrupt_boundary_on(stack);
        let candidate = interrupt_candidate_shaped(
            member.entry,
            code,
            member.root_identity,
            member.acknowledged,
        );
        let input = stack_demand_input(
            candidate.identity,
            candidate.provider,
            &boundary,
            code,
            member.entry,
            stack,
            2048,
        );
        inputs.push(input);
        shaped.push((candidate, boundary));
    }
    let relation = StackNestingRelation {
        identity: shaped
            .first()
            .expect("at least one table member")
            .0
            .nesting_relation,
        edges: BTreeSet::new(),
    };
    let composition =
        compose_bound_entry_stack_epochs(&relation, inputs.iter()).expect("shared composition");
    shaped
        .into_iter()
        .map(|(mut candidate, boundary)| {
            candidate.stack.realization = composition.clone();
            (
                validate_external_root(candidate, &boundary).expect("table member root plan"),
                boundary,
            )
        })
        .collect()
}

pub(crate) fn interrupt_entry_receipt(
    root: &InstalledExternalRoot<'_>,
    invocation: u64,
    acknowledgement_policy: Option<u64>,
    acknowledgement: Option<u64>,
) -> InterruptEntryReceipt {
    interrupt_entry_receipt_in_context(
        root,
        // The fixture realizations admit arrival context 1 for every root.
        ArrivalContextId::new(1).expect("fixture arrival context"),
        None,
        invocation,
        acknowledgement_policy,
        acknowledgement,
    )
}

/// The full receipt shape: the provider reports the exact arrival context the
/// invocation fired under and, for a nested arrival, the live invocation it
/// preempts with that invocation's epoch stage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn interrupt_entry_receipt_in_context(
    root: &InstalledExternalRoot<'_>,
    arrival_context: ArrivalContextId,
    preemption: Option<InterruptPreemptionReport>,
    invocation: u64,
    acknowledgement_policy: Option<u64>,
    acknowledgement: Option<u64>,
) -> InterruptEntryReceipt {
    InterruptEntryReceipt::from_provider(
        root_id(
            60 + invocation,
            InterruptEntryReceiptId::from_normalized_identity,
        ),
        root,
        root_id(invocation, InterruptInvocationId::from_normalized_identity),
        arrival_context,
        preemption,
        root_id(
            70 + invocation,
            InterruptMaskControlId::from_normalized_identity,
        ),
        root_id(80, InterruptMaskStateId::from_normalized_identity),
        acknowledgement_policy
            .map(|identity| root_id(identity, AcknowledgementPolicyId::from_normalized_identity)),
        acknowledgement.map(|identity| {
            root_id(
                identity,
                InterruptAcknowledgementId::from_normalized_identity,
            )
        }),
    )
}

pub(super) fn progress_installation_fixture() -> (
    SelectedProviderPlanFacts,
    ComponentProgressManifest,
    u64,
    u64,
    ServiceProgressEstablishmentRoute,
) {
    let route = ServiceProgressEstablishmentRoute {
        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
        requirement_identity: "SchedulerAdmission::grant_weak_fair#exact".into(),
    };
    let scheduler = ProviderPlan {
        name: "scheduler-plan".into(),
        provider_type: "SchedulerProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "wait".into(),
                requirement_owner: "Scheduler".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                service_reach: vec!["Scheduler".into()],
                may_suspend: true,
                terminates_guarantee: true,
                termination_premises: vec![ServiceProgressPremise {
                    profile: "SchedulerHandle::WeakFair".into(),
                    subject: ServiceProgressSubject::ProviderReceiver,
                    subject_projections: vec!["queue".into()],
                    establishment_routes: vec![route.clone()],
                }],
                ..ServiceMethod::default()
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "wait".into(),
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "TestScheduler::wait".into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "omega::test".into(),
    };
    let admission = ProviderPlan {
        name: "scheduler-admission-plan".into(),
        provider_type: "SchedulerAdmissionProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "SchedulerAdmission".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "grant_weak_fair".into(),
                requirement_owner: "SchedulerAdmission".into(),
                requirement_identity: route.requirement_identity.clone(),
                parameter_count: 1,
                parameter_type_identities: vec!["SchedulerHandle".into()],
                has_result: true,
                result_type_identity: Some("SchedulerHandle in WeakFair".into()),
                service_reach: vec!["SchedulerAdmission".into()],
                terminates_guarantee: true,
                ..ServiceMethod::default()
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "grant_weak_fair".into(),
            requirement_identity: route.requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "TestSchedulerAdmission::grant_weak_fair".into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "omega::test".into(),
    };
    let scheduler_identity = scheduler.report_fingerprint();
    let admission_identity = admission.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        &[scheduler, admission],
        &["scheduler-plan".into(), "scheduler-admission-plan".into()],
    )
    .expect("exact selected provider closure");
    let manifest = ComponentProgressManifest::bind(
        "Application::start".into(),
        &selected,
        vec![CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            provider_service_package_identity: None,
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_owner_package_identity: None,
            profile_identity: "SchedulerHandle::WeakFair".into(),
            subject_projections: vec!["queue".into()],
            origin_callable_identity: "Application::start".into(),
            origin_state_identity: "Application::start::entry".into(),
            statement_ordinal: 4,
            call_ordinal: 1,
        }],
    )
    .expect("component progress manifest");
    (
        selected,
        manifest,
        scheduler_identity,
        admission_identity,
        route,
    )
}

pub(super) fn provider_occurrence_binding(
    code: &InstalledCode,
    selected: &SelectedProviderPlanFacts,
    plan: u64,
    receipt: u64,
    occurrence: u64,
    provider: &str,
) -> ProviderOccurrencePlanBinding {
    ProviderOccurrencePlanBinding::new(
        plan,
        selected
            .plan_by_report_fingerprint(plan)
            .expect("fixture plan belongs to the selected closure")
            .clone(),
        ProviderOccurrenceInstallationReceipt::from_provider(
            root_id(
                receipt,
                ProviderOccurrenceInstallationReceiptId::from_normalized_identity,
            ),
            code,
            root_id(
                occurrence,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            provider,
        ),
    )
}

pub(super) fn admitted_progress_receipt(
    ledger: &mut InstalledRootLedger,
    code: &InstalledCode,
    subject: u64,
    issuer: u64,
    issuer_plan: u64,
    seed: u64,
    route: ServiceProgressEstablishmentRoute,
) -> AdmittedProgressProfileEstablishment {
    ledger
        .admit_progress_profile_establishment(
            ProgressProfileEstablishmentAttestation::from_provider(
                root_id(
                    seed,
                    ProgressProfileEstablishmentReceiptId::from_normalized_identity,
                ),
                code,
                root_id(
                    subject,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                root_id(
                    issuer,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                issuer_plan,
                root_id(
                    seed + 1,
                    ProgressProfileGrantInvocationId::from_normalized_identity,
                ),
                "SchedulerHandle::WeakFair",
                vec!["queue".into()],
                route,
            ),
        )
        .expect("admitted establishment receipt")
}
