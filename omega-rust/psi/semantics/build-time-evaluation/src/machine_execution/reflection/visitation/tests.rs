//! Reflection visitation tests.

use super::{
    MemberKind, RuntimeVisitationPlan, SelectionChoice, SelectionRequirement, SelectionSnapshot,
    TypedTrees, VisitationOperation, plan_revision, replay_runtime_visitation_plan,
};
use crate::machine_execution::reflection::{
    SchemaQueryAuthority, ScopedSelectionReceiver, SelectionCoverage, SelectionProjection,
    SemanticSchemaGraph, construct_semantic_schema_graph,
};
use symbols::SymbolHandle;

const ENCODE_PROGRAM: &str = "
        trait Encode { machine encode(&self) -> u64; }
        trait Hash { machine hash(&self) -> u64; }
        data Health { v: u32; }
        data Speed { v: f32; }
        machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
        machine Speed::encode(&self) -> u64 satisfies Encode::encode { 0 }
        HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
        SpeedEnc: Speed satisfies Encode { Encode::encode = Speed::encode; }
        data Player { health: Health; speed: Speed; }
    ";

/// A sum subject: two cases, one with a payload, plus the operations its
/// members select. `Outcome`'s own encoder covers the case members, whose
/// carrier is the enclosing sum.
const SUM_PROGRAM: &str = "
        trait Encode { machine encode(&self) -> u64; }
        data Health { v: u32; }
        machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
        HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
        data Outcome { case Ready(value: Health); case Empty; }
        machine Outcome::encode(&self) -> u64 satisfies Encode::encode { 0 }
        OutcomeEnc: Outcome satisfies Encode { Encode::encode = Outcome::encode; }
    ";

/// An erased member beside a runtime one; both field types carry an
/// `Encode` conformance so the erased member's selection is recordable.
const ERASED_PROGRAM: &str = "
        trait Encode { machine encode(&self) -> u64; }
        data Health { v: u32; }
        data Tag { v: u8; }
        machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
        machine Tag::encode(&self) -> u64 satisfies Encode::encode { 0 }
        HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
        TagEnc: Tag satisfies Encode { Encode::encode = Tag::encode; }
        data Rec { secret [erased]: Health; tag: Tag; }
    ";

fn subject(typed: &TypedTrees, name: &str) -> SymbolHandle {
    typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
        .unwrap_or_else(|| panic!("data `{name}` exists"))
        .symbol
}

fn owning() -> SchemaQueryAuthority {
    SchemaQueryAuthority::OwningScope {
        requester_identity: "test::requester".to_owned(),
    }
}

fn graph(typed: &TypedTrees, name: &str) -> SemanticSchemaGraph {
    construct_semantic_schema_graph(typed, subject(typed, name), owning())
        .unwrap_or_else(|error| panic!("schema for `{name}` constructs: {error}"))
}

fn requirement(typed: &TypedTrees, trait_name: &str) -> SelectionRequirement {
    let definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name)
        .unwrap_or_else(|| panic!("trait `{trait_name}` exists"));
    SelectionRequirement::for_trait(typed, definition.symbol, None).expect("requirement resolves")
}

fn machine_choice(typed: &TypedTrees, name: &str) -> SelectionChoice {
    let symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine `{name}` exists"))
        .symbol;
    SelectionChoice::machine(typed, symbol).expect("machine choice canonicalizes")
}

fn conformance_choice(typed: &TypedTrees, name: &str) -> SelectionChoice {
    let symbol = typed
        .conformances()
        .iter()
        .find(|conformance| typed.symbols.name(conformance.symbol) == name)
        .unwrap_or_else(|| panic!("conformance `{name}` exists"))
        .symbol;
    SelectionChoice::conformance(typed, symbol).expect("conformance choice canonicalizes")
}

/// A complete DeclaredMembers snapshot over `subject_name`, selecting
/// `choice_for(member_name)` per member in authored order.
fn snapshot(
    typed: &TypedTrees,
    graph: &SemanticSchemaGraph,
    trait_name: &str,
    choice_for: &dyn Fn(&str) -> SelectionChoice,
) -> SelectionSnapshot {
    snapshot_scoped(
        typed,
        graph,
        trait_name,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        choice_for,
    )
}

fn snapshot_scoped(
    typed: &TypedTrees,
    graph: &SemanticSchemaGraph,
    trait_name: &str,
    projection: SelectionProjection,
    coverage: SelectionCoverage,
    choice_for: &dyn Fn(&str) -> SelectionChoice,
) -> SelectionSnapshot {
    let mut receiver = ScopedSelectionReceiver::new(
        typed,
        graph,
        projection,
        coverage,
        requirement(typed, trait_name),
    )
    .expect("receiver opens");
    let keys: Vec<_> = receiver.members().collect();
    for key in &keys {
        receiver
            .select(key, choice_for(key.member_identity()))
            .unwrap_or_else(|error| panic!("member `{}` selects: {error}", key.member_identity()));
    }
    receiver.freeze().expect("complete coverage freezes")
}

/// A valid plan is mutated, resealed so only the mutation is under test,
/// then replayed — the witness that replay checks content, not the
/// fingerprint.
fn reseal(plan: &mut RuntimeVisitationPlan) {
    plan.revision = plan_revision(plan);
}

fn player_snapshot(typed: &TypedTrees, graph: &SemanticSchemaGraph) -> SelectionSnapshot {
    snapshot(typed, graph, "Encode", &|member| match member {
        "Player::health" => machine_choice(typed, "Health::encode"),
        "Player::speed" => machine_choice(typed, "Speed::encode"),
        other => panic!("unexpected member {other}"),
    })
}

fn rec_snapshot(typed: &TypedTrees, graph: &SemanticSchemaGraph) -> SelectionSnapshot {
    snapshot(typed, graph, "Encode", &|member| match member {
        "Rec::secret" => conformance_choice(typed, "HealthEnc"),
        "Rec::tag" => conformance_choice(typed, "TagEnc"),
        other => panic!("unexpected member {other}"),
    })
}

#[test]
fn record_plan_composes_calls_in_declaration_order() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    let plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");

    assert_eq!(plan.calls().len(), 2);
    let health = &plan.calls()[0];
    assert_eq!(health.field().member_identity(), "Player::health");
    assert_eq!(health.field().name(), "health");
    assert_eq!(health.field().kind(), MemberKind::Field);
    assert_eq!(health.field().declaration_position(), 0);
    assert!(
        health
            .field()
            .qualified_type()
            .is_some_and(|ty| ty.contains("Health"))
    );
    assert_eq!(health.field().owner_case_identity(), None);
    assert!(
        health
            .field()
            .carrier_identity()
            .is_some_and(|carrier| carrier.contains("Health")),
        "the carrier is the member type's head nominal"
    );
    assert_eq!(
        health.operation(),
        &VisitationOperation::Machine {
            machine_identity: machine_choice_identity(&typed, "Health::encode"),
        }
    );
    let speed = &plan.calls()[1];
    assert_eq!(speed.field().member_identity(), "Player::speed");
    assert_eq!(speed.field().declaration_position(), 1);

    replay_runtime_visitation_plan(&typed, &snapshot, &plan).expect("a composed plan replays");
}

fn machine_choice_identity(typed: &TypedTrees, name: &str) -> String {
    match machine_choice(typed, name) {
        SelectionChoice::Machine { machine_identity } => machine_identity,
        _ => panic!("machine choice"),
    }
}

#[test]
fn sum_plan_covers_cases_and_payloads_with_member_kinds() {
    let typed = crate::front_end::typed_program(SUM_PROGRAM);
    let graph = graph(&typed, "Outcome");
    let snapshot = snapshot(&typed, &graph, "Encode", &|member| match member {
        "Outcome::Ready" | "Outcome::Empty" => conformance_choice(&typed, "OutcomeEnc"),
        "Outcome::Ready::value" => conformance_choice(&typed, "HealthEnc"),
        other => panic!("unexpected member {other}"),
    });
    let plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");

    assert_eq!(
        plan.calls()
            .iter()
            .map(|call| call.field().member_identity())
            .collect::<Vec<_>>(),
        vec!["Outcome::Ready", "Outcome::Ready::value", "Outcome::Empty"],
        "calls follow authored order: each case, then its payloads"
    );
    let ready = &plan.calls()[0];
    assert_eq!(ready.field().kind(), MemberKind::Case);
    assert_eq!(ready.field().qualified_type(), None);
    assert_eq!(ready.field().owner_case_identity(), None);
    assert!(
        ready
            .field()
            .carrier_identity()
            .is_some_and(|carrier| carrier.contains("Outcome")),
        "a case's carrier is the enclosing sum"
    );
    let payload = &plan.calls()[1];
    assert_eq!(payload.field().kind(), MemberKind::PayloadField);
    assert_eq!(
        payload.field().owner_case_identity(),
        Some(ready.field().member_identity())
    );
    assert_eq!(plan.calls()[2].field().kind(), MemberKind::Case);

    replay_runtime_visitation_plan(&typed, &snapshot, &plan).expect("replays");
}

#[test]
fn erased_member_with_a_selection_rejects_at_compose() {
    let typed = crate::front_end::typed_program(ERASED_PROGRAM);
    let graph = graph(&typed, "Rec");
    // A declared-scope selection may legitimately select an operation for
    // the erased member; a runtime visitation plan over it cannot emit
    // the call and rejects rather than silently dropping the choice.
    let snapshot = rec_snapshot(&typed, &graph);
    let error = RuntimeVisitationPlan::compose(&typed, &snapshot)
        .expect_err("an erased member's selection cannot produce a call");
    assert!(error.contains("erased"), "{error}");
    assert!(error.contains("Rec::secret"), "{error}");
}

#[test]
fn erased_member_under_runtime_scope_visits_the_rest() {
    let typed = crate::front_end::typed_program(ERASED_PROGRAM);
    let graph = graph(&typed, "Rec");
    let snapshot = snapshot_scoped(
        &typed,
        &graph,
        "Encode",
        SelectionProjection::RuntimeMembers,
        SelectionCoverage::Complete,
        &|member| match member {
            "Rec::tag" => conformance_choice(&typed, "TagEnc"),
            other => panic!("unexpected member {other}"),
        },
    );
    let plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    assert_eq!(plan.calls().len(), 1);
    assert_eq!(plan.calls()[0].field().member_identity(), "Rec::tag");
    replay_runtime_visitation_plan(&typed, &snapshot, &plan).expect("replays");
}

#[test]
fn excluded_member_produces_no_call() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode"),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .exclude(&receiver.member("speed").expect("key"))
        .expect("exclusion records");
    let snapshot = receiver.freeze().expect("freezes");

    let plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    assert_eq!(
        plan.calls().len(),
        1,
        "the excluded member produces no call; the snapshot retains the exclusion"
    );
    assert_eq!(plan.calls()[0].field().member_identity(), "Player::health");
    replay_runtime_visitation_plan(&typed, &snapshot, &plan).expect("replays");
}

#[test]
fn empty_subject_composes_an_empty_plan() {
    let typed = crate::front_end::typed_program(
        "trait Encode { machine encode(&self) -> u64; } data Empty {}",
    );
    let graph = graph(&typed, "Empty");
    let snapshot = snapshot(&typed, &graph, "Encode", &|_| unreachable!("no members"));
    let plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    assert!(plan.calls().is_empty());
    replay_runtime_visitation_plan(&typed, &snapshot, &plan).expect("replays");
}

#[test]
fn compose_rejects_a_snapshot_that_does_not_replay() {
    let program = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&program, "Player");
    let snapshot = player_snapshot(&program, &graph);
    // The same snapshot presented against a program without the subject
    // fails the snapshot replay inside compose before any call is made.
    let other = crate::front_end::typed_program(
        "trait Encode { machine encode(&self) -> u64; } data Other { x: u8; }",
    );
    let error = RuntimeVisitationPlan::compose(&other, &snapshot)
        .expect_err("an unbound subject cannot compose a plan");
    assert!(error.contains("does not resolve"), "{error}");
}

#[test]
fn replay_rejects_header_mismatch() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.requirement = requirement(&typed, "Hash");
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a plan claiming a different requirement must reject");
    assert!(error.contains("header"), "{error}");

    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.authority = SchemaQueryAuthority::ForeignScope {
        requester_identity: "pkg::forged".to_owned(),
    };
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a plan claiming a different authority must reject");
    assert!(error.contains("header"), "{error}");
}

#[test]
fn replay_rejects_dropped_reordered_and_extra_calls() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);

    let mut dropped = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    dropped.calls.pop();
    reseal(&mut dropped);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &dropped)
        .expect_err("a dropped call must reject");
    assert!(error.contains("does not match"), "{error}");

    let mut reordered = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    reordered.calls.swap(0, 1);
    reseal(&mut reordered);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &reordered)
        .expect_err("reordered calls must reject — canonical order is checked");
    assert!(error.contains("does not match"), "{error}");

    let mut extra = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    let duplicate = extra.calls[0].clone();
    extra.calls.push(duplicate);
    reseal(&mut extra);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &extra)
        .expect_err("an extra call must reject");
    assert!(error.contains("does not match"), "{error}");
}

#[test]
fn replay_rejects_stale_field_info() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.calls[0].field.stable_number = Some(42);
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a forged FieldInfo fact must reject");
    assert!(error.contains("does not match"), "{error}");

    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.calls[0].field.kind = MemberKind::Case;
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a forged member kind must reject");
    assert!(error.contains("does not match"), "{error}");
}

#[test]
fn replay_rejects_forged_operation() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.calls[0].operation = VisitationOperation::Machine {
        machine_identity: machine_choice_identity(&typed, "Speed::encode"),
    };
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("an operation other than the recorded choice must reject");
    assert!(error.contains("does not match"), "{error}");
}

#[test]
fn replay_rejects_a_snapshot_the_plan_cannot_expand() {
    let typed = crate::front_end::typed_program(ERASED_PROGRAM);
    let graph = graph(&typed, "Rec");
    // The declared-scope snapshot selecting the erased member's operation
    // replays as a selection but has no valid runtime expansion; a plan
    // presented beside it must reject the same way compose does.
    let snapshot = rec_snapshot(&typed, &graph);
    let mut plan = RuntimeVisitationPlan {
        subject_owner_identity: snapshot.subject_owner_identity().to_owned(),
        subject_application_identity: None,
        authority: snapshot.authority().clone(),
        requirement: snapshot.requirement().clone(),
        calls: Vec::new(),
        revision: 0,
    };
    reseal(&mut plan);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("the erased member's selection has no runtime expansion");
    assert!(error.contains("erased"), "{error}");
}

#[test]
fn replay_rejects_tampered_revision() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    let mut plan = RuntimeVisitationPlan::compose(&typed, &snapshot).expect("plan composes");
    plan.revision = plan.revision.wrapping_add(1);
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a tampered report fingerprint must reject");
    assert!(error.contains("fingerprint"), "{error}");
}

#[test]
fn replay_rejects_plan_against_the_wrong_snapshot() {
    let typed = crate::front_end::typed_program(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = player_snapshot(&typed, &graph);
    // A plan composed over a partial-coverage snapshot cannot be presented
    // beside a complete one over the same subject: the header matches but
    // the expansion differs.
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode"),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .exclude(&receiver.member("speed").expect("key"))
        .expect("exclusion records");
    let partial = receiver.freeze().expect("freezes");
    let plan = RuntimeVisitationPlan::compose(&typed, &partial).expect("plan composes");
    let error = replay_runtime_visitation_plan(&typed, &snapshot, &plan)
        .expect_err("a plan expanded from a different snapshot must reject");
    assert!(error.contains("does not match"), "{error}");
}
