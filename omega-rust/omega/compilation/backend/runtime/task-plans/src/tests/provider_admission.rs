//! Provider admission: the gate that binds receipts, spends bounded
//! provisioned backing under fresh lease eras, and conserves custody on
//! every rejection.

use super::{
    activation_fact_for, activation_set, candidate, canonical_crossing, id, marshal_arguments,
    moved_arguments, partial_wcsu_plan, receipt_candidate, runtime, stack_lease, wcsu_plan,
};
use crate::{
    ActivationCarryObligations, ActivationInstanceId, ActivationPlanCandidate, ActivationPlanId,
    CallTargetBinding, CanonicalSuspensionCrossing, ClaimId, LiveCarryDemand, LiveCarryPlaceId,
    LiveCarryStorage, LiveCarryTypeId, StackCallContribution, StackPlan, StackRepresentationId,
    SuspensionCrossingId, TaskActivationPlanSet, TaskArgumentCustodyId, TaskRuntimeAdmission,
    TaskRuntimeId, TaskRuntimeInstanceId, TaskSettlementOutcome, TaskStackFrameId,
    TaskStackFrameSummary, TaskStartOperation, TaskStartStorage, TaskStorageBinding,
    TaskStorageOwnerId, TaskStorageProvenance, UnresolvedCallKind, UnresolvedCallSite,
    ValidatedActivationPlan, ValidatedTaskStackFrameSummary, compose_task_stack_demand,
    establish_stack_lease, project_wcsu_stack_plan, task_stack_frame_validation_identity,
    validate_task_stack_frame_summary, validate_wcsu_activation_plan,
};
use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};

fn instance(identity: u64) -> TaskRuntimeInstanceId {
    id(identity, TaskRuntimeInstanceId::from_normalized_identity)
}

fn activation(identity: u64) -> ActivationInstanceId {
    id(identity, ActivationInstanceId::from_normalized_identity)
}

fn custody(identity: u64) -> TaskArgumentCustodyId {
    id(identity, TaskArgumentCustodyId::from_normalized_identity)
}

fn owner() -> TaskStorageOwnerId {
    id(300, TaskStorageOwnerId::from_normalized_identity)
}

fn gate(slots: &[StackPlan], instance_identity: u64) -> TaskRuntimeAdmission {
    TaskRuntimeAdmission::new(
        runtime(),
        instance(instance_identity),
        owner(),
        slots.to_vec(),
    )
    .expect("provisioned admission gate")
}

fn persistent_binding(record: &crate::TaskDependencyRecord) -> TaskStorageProvenance {
    let TaskStorageBinding::Persistent(provenance) = record.storage else {
        panic!("pending admission records persistent storage");
    };
    provenance
}

#[test]
fn pending_admission_establishes_a_fresh_provider_lease() {
    let plan = wcsu_plan(50);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 301);

    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(301), 302, 303, TaskStartOperation::Start),
            activation(304),
            moved_arguments(&plan, 305),
        )
        .expect("a satisfying provisioned slot admits the pending start");

    let record = gate.records().next().expect("one live dependency");
    let provenance = persistent_binding(record);
    assert_eq!(provenance.owner, owner());
    assert_eq!(
        provenance.lease.normalized_identity(),
        1,
        "the provider mints the first fresh lease era"
    );
    assert_eq!(record.operation, TaskStartOperation::Start);
    gate.request_cancellation(&claim)
        .expect("a live claim accepts a cancellation request");
    assert!(gate.cancellation_requested(claim.identity()));
    gate.observe_cancellation(&claim, canonical_crossing())
        .expect("the activation observes the request at a canonical safe point");

    let close = gate.close().expect_err("a live claim blocks close");
    let mut gate = close.into_admission();
    let settled = gate
        .settle(claim, TaskSettlementOutcome::Cancelled)
        .expect("terminal settlement");
    assert_eq!(settled.outcome(), TaskSettlementOutcome::Cancelled);
    assert_eq!(
        settled.released_storage(),
        TaskStorageBinding::Persistent(provenance),
        "settlement releases the exact pool lease"
    );
    gate.close().expect("an empty gate closes");
}

#[test]
fn unsatisfying_backing_rejects_without_spending_capacity() {
    let plan = wcsu_plan(52);
    let activations = activation_set(&plan);
    let required = plan.candidate().stack_plan;
    let undersized = StackPlan {
        bytes: required.bytes / 2,
        ..required
    };
    let mut gate = gate(&[undersized], 330);

    let rejected_custody = custody(333);
    let rejection = gate
        .admit_pending(
            &activations,
            receipt_candidate(&plan, instance(330), 331, 332, TaskStartOperation::Start),
            activation(334),
            marshal_arguments(&plan.candidate().argument_layout, 333),
        )
        .expect_err("undersized backing cannot satisfy the plan");
    assert!(
        rejection.diagnostic().0.contains("satisfying"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );
    assert_eq!(rejection.into_arguments().custody(), rejected_custody);

    // The failed admission spent nothing: a caller-supplied lease still
    // reaches the ledger through the same gate.
    let claim = gate
        .admit_with_storage(
            &activations,
            receipt_candidate(&plan, instance(330), 335, 336, TaskStartOperation::Start),
            activation(337),
            moved_arguments(&plan, 338),
            TaskStartStorage::Persistent(stack_lease(&plan, 339, 340)),
        )
        .expect("caller-supplied lease admits without touching the pool");
    gate.settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle caller-supplied storage");
}

#[test]
fn receipt_binding_rejects_before_custody_moves() {
    let plan = wcsu_plan(55);
    let activations = activation_set(&plan);
    let mut gate = gate(&[plan.candidate().stack_plan], 360);

    // A receipt naming an activation plan this provider does not serve
    // fails closed.
    let mut foreign_plan =
        receipt_candidate(&plan, instance(360), 361, 362, TaskStartOperation::Start);
    foreign_plan.activation_plan = id(363, ActivationPlanId::from_normalized_identity);
    let rejection = gate
        .admit_pending(
            &activations,
            foreign_plan,
            activation(364),
            moved_arguments(&plan, 365),
        )
        .expect_err("an unknown activation plan rejects");
    assert!(
        rejection.diagnostic().0.contains("no activation plan"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );

    // A receipt naming a different selected runtime fails binding even
    // though the plan is served.
    let mut foreign_runtime =
        receipt_candidate(&plan, instance(360), 366, 367, TaskStartOperation::Start);
    foreign_runtime.runtime = id(368, TaskRuntimeId::from_normalized_identity);
    let rejection = gate
        .admit_pending(
            &activations,
            foreign_runtime,
            activation(369),
            moved_arguments(&plan, 370),
        )
        .expect_err("runtime drift rejects");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("different selected runtime"),
        "unexpected diagnostic: {}",
        rejection.diagnostic().0
    );

    // Neither rejection spent the provisioned slot.
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(360), 371, 372, TaskStartOperation::Start),
        activation(373),
        moved_arguments(&plan, 374),
    )
    .expect("the untouched slot still admits");
}

#[test]
fn operation_binds_the_exact_activation_fact() {
    let plan = wcsu_plan(56);
    let activations = TaskActivationPlanSet {
        activations: vec![
            activation_fact_for(&plan, TaskStartOperation::Start),
            activation_fact_for(&plan, TaskStartOperation::TryStart),
        ],
    };
    let mut gate = gate(&[plan.candidate().stack_plan], 380);

    // Both facts share one plan identity; the receipt's operation selects
    // the exact one it binds.
    gate.admit_pending(
        &activations,
        receipt_candidate(&plan, instance(380), 381, 382, TaskStartOperation::TryStart),
        activation(383),
        moved_arguments(&plan, 384),
    )
    .expect("a try_start receipt binds the try_start fact");
    let record = gate.records().next().expect("one live dependency");
    assert_eq!(record.operation, TaskStartOperation::TryStart);
}

#[test]
fn malformed_provisioning_rejects_at_construction() {
    let plan = wcsu_plan(58);
    let mut malformed = plan.candidate().stack_plan;
    malformed.alignment = 3;
    let diagnostic = TaskRuntimeAdmission::new(runtime(), instance(410), owner(), vec![malformed])
        .expect_err("a malformed provisioned slot rejects at construction");
    assert!(diagnostic.0.contains("power of two"));
}

/// A partial WCSU plan whose root frame seals `sites` under a caller-chosen
/// candidate — like `partial_wcsu_plan` but with a caller-chosen unresolved
/// roster and operational shape.
fn partial_plan_with_candidate(
    mut candidate: ActivationPlanCandidate,
    sites: Vec<UnresolvedCallSite>,
) -> ValidatedActivationPlan {
    let root = id(30, TaskStackFrameId::from_normalized_identity);
    let frame = validate_task_stack_frame_summary(TaskStackFrameSummary {
        frame: root,
        local_bytes: 4096,
        alignment: 16,
        validation: task_stack_frame_validation_identity(root, 4096, 16, &[], &sites),
        calls: Vec::new(),
        unresolved_calls: sites,
    })
    .expect("validated partial WCSU frame");
    let demand = compose_task_stack_demand(root, [frame]).expect("composed partial WCSU demand");
    let projection = project_wcsu_stack_plan(
        &demand,
        id(6, StackRepresentationId::from_normalized_identity),
    );
    assert!(!projection.is_exact());
    candidate.stack_plan = projection.stack_plan();
    validate_wcsu_activation_plan(candidate, projection)
        .expect("partial-WCSU-backed activation plan")
}

/// A partial WCSU plan whose root frame seals `sites` — like
/// `partial_wcsu_plan` but with a caller-chosen unresolved roster.
fn multi_site_partial_plan(sites: Vec<UnresolvedCallSite>) -> ValidatedActivationPlan {
    partial_plan_with_candidate(candidate(), sites)
}

fn crossing(identity: u64) -> SuspensionCrossingId {
    SuspensionCrossingId::new(identity).expect("nonzero crossing identity")
}

/// The canonical suspension crossing a suspending bound subtree carries:
/// one live local retaining a CPU-pinned loan, so the crossing's own
/// preservation bits and the plan's checked CPU-preservation obligation are
/// both honestly exercised.
fn bound_subtree_crossing(identity: u64) -> CanonicalSuspensionCrossing {
    CanonicalSuspensionCrossing {
        identity: crossing(identity),
        suspension_allowed: true,
        preserve_cpu: true,
        preserve_host_thread: false,
        live_carry: vec![LiveCarryDemand {
            place: id(43, LiveCarryPlaceId::from_normalized_identity),
            ty: id(44, LiveCarryTypeId::from_normalized_identity),
            storage: LiveCarryStorage::Local,
            claims: vec![ClaimId::new(45).expect("nonzero claim identity")],
            effective: CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Any,
                address: CarryAddress::Stable,
            },
        }],
    }
}

/// A validated subtree frame carrying its canonical validation identity —
/// the shape `task_call_graph` produces for one checked callee.
fn subtree_frame(
    frame: u64,
    bytes: u64,
    alignment: u64,
    calls: Vec<StackCallContribution>,
) -> ValidatedTaskStackFrameSummary {
    let frame = id(frame, TaskStackFrameId::from_normalized_identity);
    validate_task_stack_frame_summary(TaskStackFrameSummary {
        frame,
        local_bytes: bytes,
        alignment,
        validation: task_stack_frame_validation_identity(frame, bytes, alignment, &calls, &[]),
        calls,
        unresolved_calls: Vec::new(),
    })
    .expect("validated subtree frame")
}

/// The provider's binding of a sealed site to a checked-body machine whose
/// validated subtree is one leaf frame — what `task_call_graph` produces for
/// a non-suspending callee.
fn leaf_binding(
    site: &UnresolvedCallSite,
    callee: u64,
    bytes: u64,
    alignment: u64,
) -> CallTargetBinding {
    let callee = id(callee, TaskStackFrameId::from_normalized_identity);
    CallTargetBinding {
        frame: site.frame,
        state: site.state.clone(),
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        callee,
        subtree: vec![
            validate_task_stack_frame_summary(TaskStackFrameSummary {
                frame: callee,
                local_bytes: bytes,
                alignment,
                validation: task_stack_frame_validation_identity(
                    callee,
                    bytes,
                    alignment,
                    &[],
                    &[],
                ),
                calls: Vec::new(),
                unresolved_calls: Vec::new(),
            })
            .expect("bound callee frame"),
        ],
        crossings: Vec::new(),
    }
}

/// A binding whose bound callee subtree suspends internally: `callee` calls
/// the checked `child` frame and `crossings` carries the canonical
/// suspension crossing that internal call parks at — the subtree rows
/// `task_call_graph` collects for a suspending checked-body callee.
fn suspending_binding(
    site: &UnresolvedCallSite,
    callee: u64,
    child: u64,
    crossings: Vec<CanonicalSuspensionCrossing>,
) -> CallTargetBinding {
    let callee_id = id(callee, TaskStackFrameId::from_normalized_identity);
    CallTargetBinding {
        frame: site.frame,
        state: site.state.clone(),
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        callee: callee_id,
        subtree: vec![
            subtree_frame(
                callee,
                128,
                16,
                vec![StackCallContribution::Checked {
                    callee: id(child, TaskStackFrameId::from_normalized_identity),
                }],
            ),
            subtree_frame(child, 64, 8, Vec::new()),
        ],
        crossings,
    }
}

#[test]
fn an_unbound_unresolved_site_keeps_the_plan_rejecting() {
    let root = id(30, TaskStackFrameId::from_normalized_identity);
    let covered_site = UnresolvedCallSite {
        frame: root,
        state: "run".into(),
        statement_index: 2,
        call_ordinal: 0,
        kind: UnresolvedCallKind::UnresolvedTarget,
    };
    let unbound_site = UnresolvedCallSite {
        frame: root,
        state: "run".into(),
        statement_index: 7,
        call_ordinal: 1,
        kind: UnresolvedCallKind::UnresolvedTarget,
    };
    let plan = multi_site_partial_plan(vec![covered_site.clone(), unbound_site.clone()]);

    // Admission binds one slot; the site it never bound keeps the covered
    // projection partial, so the lease still refuses it.
    let covered =
        TaskRuntimeAdmission::bind_call_targets(&plan, &[leaf_binding(&covered_site, 40, 128, 16)])
            .expect("the named site covers");
    let projection = covered.wcsu_stack_projection().expect("covered projection");
    assert!(!projection.is_exact());
    assert_eq!(
        projection.unresolved_calls().iter().collect::<Vec<_>>(),
        vec![&unbound_site],
        "only the bound site left the roster"
    );
    let error = establish_stack_lease(
        &covered,
        crate::StackLeaseBacking {
            provenance: TaskStorageProvenance {
                owner: owner(),
                lease: id(455, crate::TaskStorageLeaseId::from_normalized_identity),
            },
            backing: covered.candidate().stack_plan,
        },
    )
    .expect_err("an unbound site keeps the lease closed");
    assert!(
        error.0.contains("partial"),
        "unexpected diagnostic: {}",
        error.0
    );

    // And a binding naming a coordinate the plan never sealed covers nothing.
    let foreign_site = UnresolvedCallSite {
        frame: root,
        state: "run".into(),
        statement_index: 9,
        call_ordinal: 0,
        kind: UnresolvedCallKind::UnresolvedTarget,
    };
    TaskRuntimeAdmission::bind_call_targets(&plan, &[leaf_binding(&foreign_site, 40, 128, 16)])
        .expect_err("a binding must name a sealed unresolved site");
}

#[test]
fn bound_suspending_subtree_joins_its_crossing_and_parks_on_it() {
    let plan = partial_wcsu_plan(63);
    let projection = plan
        .wcsu_stack_projection()
        .expect("sealed WCSU projection");
    let site = projection
        .unresolved_calls()
        .iter()
        .next()
        .expect("one sealed site")
        .clone();

    // The bound callee's subtree suspends internally: its canonical
    // crossing must join the plan's roster for the activation to park
    // there.
    let bound_crossing = bound_subtree_crossing(42);
    let covered = TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[suspending_binding(
            &site,
            40,
            41,
            vec![bound_crossing.clone()],
        )],
    )
    .expect("the suspending bound subtree covers the sealed site");

    // The joined roster publishes in canonical identity order alongside the
    // graph-time row, and the re-sealed plan identity binds it.
    assert_eq!(
        covered
            .candidate()
            .canonical_suspension_crossings
            .iter()
            .map(|crossing| crossing.identity)
            .collect::<Vec<_>>(),
        vec![canonical_crossing(), bound_crossing.identity],
    );
    let covered_projection = covered.wcsu_stack_projection().expect("covered projection");
    assert!(covered_projection.is_exact());
    assert_ne!(
        covered.normalized_identity(),
        plan.normalized_identity(),
        "joining the bound subtree's crossing re-seals the activation plan"
    );

    // The covered plan admits, and the bound callee can park at a crossing
    // of its own subtree.
    let activations = activation_set(&covered);
    let mut gate = gate(&[covered.candidate().stack_plan], 460);
    let claim = gate
        .admit_pending(
            &activations,
            receipt_candidate(&covered, instance(460), 461, 462, TaskStartOperation::Start),
            activation(463),
            moved_arguments(&covered, 464),
        )
        .expect("the covered plan admits and leases");

    // A crossing the joined roster does not name — an unbound or foreign
    // subtree's coordinate — still rejects.
    let foreign = gate
        .park(&claim, crossing(999))
        .expect_err("a crossing outside the joined roster cannot park");
    assert!(
        foreign
            .0
            .contains("outside the activation plan's canonical roster"),
        "unexpected diagnostic: {}",
        foreign.0
    );
    assert!(gate.parked_crossing(claim.identity()).is_none());

    gate.park(&claim, bound_crossing.identity)
        .expect("the bound callee parks at its own canonical crossing");
    assert_eq!(
        gate.parked_crossing(claim.identity()),
        Some(bound_crossing.identity),
    );
    // The retained frontier is exactly the bound crossing's live carry:
    // the exact-live-frontier park semantics stay intact on a joined row.
    assert_eq!(
        gate.parked_frontier(claim.identity()),
        Some(bound_crossing.live_carry.as_slice()),
    );

    assert_eq!(
        gate.resume(&claim)
            .expect("resume continues the same invocation"),
        bound_crossing.identity,
    );
    assert!(gate.parked_frontier(claim.identity()).is_none());
    gate.settle(claim, TaskSettlementOutcome::Completed)
        .expect("settle after resume");
}

#[test]
fn bound_subtree_crossings_dedupe_and_conflicts_fail_closed() {
    let plan = partial_wcsu_plan(64);
    let projection = plan
        .wcsu_stack_projection()
        .expect("sealed WCSU projection");
    let site = projection
        .unresolved_calls()
        .iter()
        .next()
        .expect("one sealed site")
        .clone();
    let retained = plan.candidate().canonical_suspension_crossings.clone();

    // A bound subtree sharing a crossing the graph's own roster already
    // holds dedupes: the identical row joins nothing.
    let covered = TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[suspending_binding(&site, 40, 41, retained.clone())],
    )
    .expect("an identical crossing row dedupes");
    assert_eq!(
        covered.candidate().canonical_suspension_crossings,
        retained,
        "the deduplicated roster is unchanged"
    );

    // The same canonical identity carrying a different row is a coordinate
    // conflict and fails closed — the covered plan is never minted.
    let conflicting = bound_subtree_crossing(7);
    let diagnostic = TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[suspending_binding(&site, 40, 41, vec![conflicting])],
    )
    .expect_err("a conflicting crossing coordinate fails closed");
    assert!(
        diagnostic.0.contains("conflicts"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
}

#[test]
fn a_non_suspending_plan_cannot_gain_crossings_from_a_binding() {
    let mut quiet = candidate();
    quiet.may_suspend = false;
    quiet.canonical_suspension_crossings = Vec::new();
    quiet.carry_obligations = ActivationCarryObligations::none();
    let plan = partial_plan_with_candidate(
        quiet,
        vec![UnresolvedCallSite {
            frame: id(30, TaskStackFrameId::from_normalized_identity),
            state: "run".into(),
            statement_index: 2,
            call_ordinal: 0,
            kind: UnresolvedCallKind::UnresolvedTarget,
        }],
    );
    let site = plan
        .wcsu_stack_projection()
        .expect("sealed WCSU projection")
        .unresolved_calls()
        .iter()
        .next()
        .expect("one sealed site")
        .clone();

    // The joined crossing revalidates under the plan's own rules: a
    // non-suspending plan cannot publish crossings, so the binding's
    // suspending subtree cannot launder suspension into it.
    let diagnostic = TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[suspending_binding(
            &site,
            40,
            41,
            vec![bound_subtree_crossing(42)],
        )],
    )
    .expect_err("a non-suspending plan cannot gain a suspension crossing");
    assert!(
        diagnostic.0.contains("non-suspending"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );
}
