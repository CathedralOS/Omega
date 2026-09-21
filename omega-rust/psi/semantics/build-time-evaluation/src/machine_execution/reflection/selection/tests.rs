//! Reflection selection tests.

use super::{
    ScopedSelectionReceiver, SelectionChoice, SelectionCoverage, SelectionProjection,
    SelectionRequirement, SelectionSnapshot, SymbolHandle, TypedTrees, replay_selection_snapshot,
};
use crate::machine_execution::reflection::construct_semantic_schema_graph;
use crate::machine_execution::reflection::schema_graph::{
    SchemaQueryAuthority, SemanticSchemaGraph,
};
use crate::machine_execution::reflection::selection::snapshots::snapshot_revision;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

/// A program exercising both choice families: authored `satisfies`
/// machines, closed conformance applications naming realizations, and a
/// second trait so an insufficient-contract choice has something to be.
const ENCODE_PROGRAM: &str = "
    trait Encode { machine encode(&self) -> u64; }
    trait Hash { machine hash(&self) -> u64; }
    data Health { v: u32; }
    data Speed { v: f32; }
    machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
    machine Speed::encode(&self) -> u64 satisfies Encode::encode { 0 }
    machine Health::hash(&self) -> u64 satisfies Hash::hash { 0 }
    HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
    SpeedEnc: Speed satisfies Encode { Encode::encode = Speed::encode; }
    HealthHash: Health satisfies Hash { Hash::hash = Health::hash; }
    data Player { health: Health; speed: Speed; }
";

/// A pinned-requirement program: `Codec` carries two requirements so one
/// exact row can be demanded while a sibling satisfies a different one.
const CODEC_PROGRAM: &str = "
    trait Codec { machine encode(&self) -> u64; machine decode(&self) -> u64; }
    data Blob { bytes: [u8; 4]; }
    machine Blob::encode(&self) -> u64 satisfies Codec::encode { 0 }
    machine Blob::decode(&self) -> u64 satisfies Codec::decode { 0 }
    data Wrap { blob: Blob; }
";

/// A closed conformance whose realization machine carries no authored
/// `satisfies` edge: the closed row alone must admit the machine choice.
const CLOSED_PROGRAM: &str = "
    trait Shape { machine code(&self) -> i32; }
    data Circle { r: u32; }
    machine Circle::code(&self) -> i32 { 9 }
    CircleShape: Circle satisfies Shape { Shape::code = Circle::code; }
    data Board { c: Circle; }
";

/// A bodyless conformance: the attached `Carrier::requirement` machine
/// name binding alone must admit both the conformance and the machine.
const ATTACHED_PROGRAM: &str = "
    trait Shape { machine code(&self) -> i32; }
    data Circle { r: u32; }
    machine Circle::code(&self) -> i32 { 9 }
    CircleShape: Circle satisfies Shape;
    data Board { c: Circle; }
";

/// A parameterized contract family: `Encode<Json>` and `Encode<Cbor>` are
/// different applications of one trait, and each machine's `satisfies`
/// clause carries only the application it names.
const APPLICATION_PROGRAM: &str = "
    trait Encode<P> { machine encode(&self, policy: &P) -> u64; }
    data Json { marker: u8; }
    data Cbor { marker: u8; }
    data Health { v: u32; }
    machine Health::encode(&self, policy: &Json) -> u64 satisfies Encode<Json>::encode { 0 }
    machine Health::encode_cbor(&self, policy: &Cbor) -> u64 satisfies Encode<Cbor>::encode { 0 }
    HealthJson: Health satisfies Encode<Json> { Encode::encode = Health::encode; }
    HealthCbor: Health satisfies Encode<Cbor> { Encode::encode = Health::encode_cbor; }
    data Player { health: Health; }
";

/// A sum subject whose members include cases and their payloads.
const SUM_PROGRAM: &str = "
    trait Encode { machine encode(&self) -> u64; }
    data Health { v: u32; }
    machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 }
    HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; }
    data Outcome { case Ready(value: Health); case Empty; }
    machine Outcome::encode(&self) -> u64 satisfies Encode::encode { 0 }
    OutcomeEnc: Outcome satisfies Encode { Encode::encode = Outcome::encode; }
";

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

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

fn foreign() -> SchemaQueryAuthority {
    SchemaQueryAuthority::ForeignScope {
        requester_identity: "pkg::consumer".to_owned(),
    }
}

fn graph(typed: &TypedTrees, name: &str) -> SemanticSchemaGraph {
    construct_semantic_schema_graph(typed, subject(typed, name), owning())
        .unwrap_or_else(|error| panic!("schema for `{name}` constructs: {error}"))
}

fn requirement(
    typed: &TypedTrees,
    trait_name: &str,
    requirement_name: Option<&str>,
) -> SelectionRequirement {
    let definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name)
        .unwrap_or_else(|| panic!("trait `{trait_name}` exists"));
    let requirement = requirement_name.map(|name| {
        typed
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap_or_else(|| panic!("requirement `{trait_name}::{name}` exists"))
            .symbol
    });
    SelectionRequirement::for_trait(typed, definition.symbol, requirement)
        .expect("requirement resolves")
}

/// The conformance whose carried application becomes the demand: its exact
/// type-argument handles and lifetime ordinals pin the required
/// `trait_name<args>` application.
fn application_requirement(
    typed: &TypedTrees,
    trait_name: &str,
    requirement_name: Option<&str>,
    conformance_name: &str,
) -> SelectionRequirement {
    let definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name)
        .unwrap_or_else(|| panic!("trait `{trait_name}` exists"));
    let requirement = requirement_name.map(|name| {
        typed
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap_or_else(|| panic!("requirement `{trait_name}::{name}` exists"))
            .symbol
    });
    let conformance = typed
        .conformances()
        .iter()
        .find(|conformance| typed.symbols.name(conformance.symbol) == conformance_name)
        .unwrap_or_else(|| panic!("conformance `{conformance_name}` exists"));
    let arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    SelectionRequirement::for_trait_application(
        typed,
        definition.symbol,
        requirement,
        arguments,
        &conformance.trait_lifetime_arguments,
    )
    .expect("requirement resolves")
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

fn receiver<'program>(
    typed: &'program TypedTrees,
    graph: &SemanticSchemaGraph,
    trait_name: &str,
) -> ScopedSelectionReceiver<'program> {
    ScopedSelectionReceiver::new(
        typed,
        graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(typed, trait_name, None),
    )
    .expect("receiver opens")
}

/// A valid snapshot is mutated, resealed so only the mutation is under
/// test, then replayed — the witness that replay checks content, not the
/// fingerprint.
fn reseal(snapshot: &mut SelectionSnapshot) {
    snapshot.revision = snapshot_revision(snapshot);
}

#[test]
fn complete_machine_selection_freezes_and_replays() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("health key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("health selects");
    receiver
        .select(
            &receiver.member("speed").expect("speed key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("speed selects");
    let snapshot = receiver.freeze().expect("complete coverage freezes");
    assert_eq!(snapshot.records().len(), 2);
    assert_eq!(snapshot.records()[0].member_identity, "Player::health");
    assert_eq!(snapshot.records()[1].member_identity, "Player::speed");
    replay_selection_snapshot(&typed, &snapshot).expect("a valid snapshot replays");
}

#[test]
fn conformance_choices_refine_and_replay() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("health key"),
            conformance_choice(&typed, "HealthEnc"),
        )
        .expect("health selects");
    receiver
        .select(
            &receiver.member("speed").expect("speed key"),
            conformance_choice(&typed, "SpeedEnc"),
        )
        .expect("speed selects");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn equal_selections_freeze_to_equal_snapshots() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let snapshot = {
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        receiver.freeze().expect("freezes")
    };
    // The same policy in a different authored order seals identically:
    // canonical order is a property of the snapshot, not of the calls.
    let reordered = {
        let mut receiver = receiver(&typed, &graph, "Encode");
        receiver
            .select(
                &receiver.member("speed").expect("key"),
                machine_choice(&typed, "Speed::encode"),
            )
            .expect("selects");
        receiver
            .select(
                &receiver.member("health").expect("key"),
                machine_choice(&typed, "Health::encode"),
            )
            .expect("selects");
        receiver.freeze().expect("freezes")
    };
    assert_eq!(snapshot, reordered);
    replay_selection_snapshot(&typed, &reordered).expect("replays");
}

#[test]
fn closed_conformance_row_admits_machine_choice() {
    let typed = typed(CLOSED_PROGRAM);
    let graph = graph(&typed, "Board");
    let mut receiver = receiver(&typed, &graph, "Shape");
    // `Circle::code` carries no authored `satisfies` edge; only the
    // closed conformance row realizes `Shape::code` on `Circle`.
    receiver
        .select(
            &receiver.member("c").expect("key"),
            machine_choice(&typed, "Circle::code"),
        )
        .expect("the closed row realizes the requirement");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn attached_conformance_admits_its_machines() {
    let typed = typed(ATTACHED_PROGRAM);
    let graph = graph(&typed, "Board");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(&typed, "Shape", Some("code")),
    )
    .expect("receiver opens");
    // The bodyless conformance's `Circle::code` name binding realizes the
    // pinned requirement; both the conformance and the machine itself are
    // admissible choices.
    receiver
        .select(
            &receiver.member("c").expect("key"),
            machine_choice(&typed, "Circle::code"),
        )
        .expect("the attached machine binds the pinned requirement");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");

    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(&typed, "Shape", Some("code")),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("c").expect("key"),
            conformance_choice(&typed, "CircleShape"),
        )
        .expect("the conformance realizes the pinned requirement");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn pinned_requirement_accepts_the_exact_row_and_rejects_siblings() {
    let typed = typed(CODEC_PROGRAM);
    let graph = graph(&typed, "Wrap");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(&typed, "Codec", Some("encode")),
    )
    .expect("receiver opens");
    let blob = receiver.member("blob").expect("key");
    let error = receiver
        .select(&blob, machine_choice(&typed, "Blob::decode"))
        .expect_err("a machine satisfying only `Codec::decode` cannot fill `Codec::encode`");
    assert!(error.contains("does not realize"), "{error}");
    receiver
        .select(&blob, machine_choice(&typed, "Blob::encode"))
        .expect("the pinned row selects");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn missing_selection_rejects_at_freeze() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    let error = receiver
        .freeze()
        .expect_err("an unrecorded member rejects rather than fabricating coverage");
    assert!(error.contains("no recorded selection"), "{error}");
}

#[test]
fn selecting_twice_rejects() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    let health = receiver.member("health").expect("key");
    receiver
        .select(&health, machine_choice(&typed, "Health::encode"))
        .expect("selects");
    let error = receiver
        .select(&health, machine_choice(&typed, "Health::encode"))
        .expect_err("a second selection is not last-write-wins");
    assert!(error.contains("not last-write-wins"), "{error}");

    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
    )
    .expect("partial receiver opens");
    let health = receiver.member("health").expect("key");
    receiver
        .select(&health, machine_choice(&typed, "Health::encode"))
        .expect("selects");
    let error = receiver
        .exclude(&health)
        .expect_err("an exclusion over a recorded selection also rejects");
    assert!(error.contains("not last-write-wins"), "{error}");
}

#[test]
fn stale_key_rejects() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    let mut health = receiver.member("health").expect("key");
    health.qualified_type = Some("stale type".to_owned());
    let error = receiver
        .select(&health, machine_choice(&typed, "Health::encode"))
        .expect_err("a key minted against a drifted member rejects");
    assert!(error.contains("stale"), "{error}");
}

#[test]
fn erased_member_is_outside_the_runtime_scope() {
    let typed = typed(
        "trait Encode { machine encode(&self) -> u64; } \
         machine Rec::encode(&self) -> u64 satisfies Encode::encode { 0 } \
         RecEnc: Rec satisfies Encode { Encode::encode = Rec::encode; } \
         data Rec { secret [erased]: i32; tag: u8; }",
    );
    let graph = graph(&typed, "Rec");
    let declared = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
    )
    .expect("declared scope opens");
    let secret = declared
        .member("secret")
        .expect("the erased member is described");
    assert!(secret.is_erased());
    let mut runtime = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::RuntimeMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
    )
    .expect("runtime scope opens");
    let error = runtime
        .member("secret")
        .expect_err("the erased member is outside the runtime scope");
    assert!(error.contains("scoped projection"), "{error}");
    let error = runtime
        .select(&secret, conformance_choice(&typed, "RecEnc"))
        .expect_err("a key minted in the declared scope cannot select in the runtime scope");
    assert!(error.contains("scoped projection"), "{error}");
    let tag = runtime.member("tag").expect("tag key");
    runtime
        .exclude(&tag)
        .expect("the only runtime member may exclude");
    let snapshot = runtime.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn insufficient_contract_rejects_at_selection() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    let health = receiver.member("health").expect("key");
    let error = receiver
        .select(&health, machine_choice(&typed, "Health::hash"))
        .expect_err("a `Hash` machine cannot satisfy an `Encode` requirement");
    assert!(error.contains("does not realize"), "{error}");
    let error = receiver
        .select(&health, conformance_choice(&typed, "HealthHash"))
        .expect_err("a `Hash` conformance cannot satisfy an `Encode` requirement");
    assert!(error.contains("different trait"), "{error}");
    let error = receiver
        .select(&health, conformance_choice(&typed, "SpeedEnc"))
        .expect_err("an `Encode` conformance on the wrong carrier cannot cover `health`");
    assert!(error.contains("does not cover"), "{error}");
}

#[test]
fn unresolvable_requirement_rejects_at_open() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let error = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        SelectionRequirement {
            trait_identity: "No::SuchTrait".to_owned(),
            requirement_identity: None,
            trait_application: None,
        },
    )
    .expect_err("a requirement naming no trait cannot open a receiver");
    assert!(error.contains("does not resolve"), "{error}");
}

#[test]
fn partial_coverage_retains_exclusions() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
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
    assert_eq!(
        snapshot.records()[1].choice,
        SelectionChoice::Excluded,
        "the exclusion is a retained record"
    );
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn complete_coverage_rejects_exclusions() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    let error = receiver
        .exclude(&receiver.member("speed").expect("key"))
        .expect_err("complete coverage admits no exclusions");
    assert!(error.contains("complete-coverage"), "{error}");

    // Forge the same shape past the receiver: a partial snapshot with an
    // exclusion relabeled as complete must still reject on replay.
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
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
    let mut snapshot = receiver.freeze().expect("freezes");
    snapshot.coverage = SelectionCoverage::Complete;
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("an exclusion under forged complete coverage must reject");
    assert!(error.contains("complete-coverage"), "{error}");
}

#[test]
fn explicit_member_projection_scopes_the_selection() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::Members(vec!["Player::speed".to_owned()]),
        SelectionCoverage::Complete,
        requirement(&typed, "Encode", None),
    )
    .expect("projection opens");
    let error = receiver
        .member("health")
        .expect_err("`health` is outside the projected member list");
    assert!(error.contains("scoped projection"), "{error}");
    receiver
        .select(
            &receiver.member("speed").expect("speed key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("selects");
    let snapshot = receiver.freeze().expect("one-member coverage freezes");
    assert_eq!(snapshot.records().len(), 1);
    replay_selection_snapshot(&typed, &snapshot).expect("replays");

    let error = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::Members(vec!["Player::missing".to_owned()]),
        SelectionCoverage::Complete,
        requirement(&typed, "Encode", None),
    )
    .expect_err("a member outside the schema cannot be projected");
    assert!(error.contains("not a member"), "{error}");
    let error = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::Members(vec![
            "Player::health".to_owned(),
            "Player::health".to_owned(),
        ]),
        SelectionCoverage::Complete,
        requirement(&typed, "Encode", None),
    )
    .expect_err("a duplicated projected member rejects");
    assert!(error.contains("twice"), "{error}");
}

#[test]
fn sum_cases_and_payloads_select_operations() {
    let typed = typed(SUM_PROGRAM);
    let graph = graph(&typed, "Outcome");
    let mut receiver = receiver(&typed, &graph, "Encode");
    let keys: Vec<_> = receiver.members().collect();
    assert_eq!(
        keys.iter()
            .map(|key| key.member_identity())
            .collect::<Vec<_>>(),
        vec!["Outcome::Ready", "Outcome::Ready::value", "Outcome::Empty"],
        "cases and their payloads are the declared members"
    );
    receiver
        .select(&keys[0], conformance_choice(&typed, "OutcomeEnc"))
        .expect("a case's carrier is the enclosing sum");
    receiver
        .select(&keys[1], conformance_choice(&typed, "HealthEnc"))
        .expect("the payload member's carrier is its own type");
    receiver
        .select(&keys[2], conformance_choice(&typed, "OutcomeEnc"))
        .expect("a nullary case selects like any case");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn receiver_is_storage_independent_of_the_graph() {
    let typed = typed(ENCODE_PROGRAM);
    // The receiver and its frozen snapshot carry owned data only: the
    // graph is dropped before any selection is recorded.
    let mut receiver = {
        let graph = graph(&typed, "Player");
        receiver(&typed, &graph, "Encode")
    };
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .select(
            &receiver.member("speed").expect("key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("selects");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays without the graph");
}

#[test]
fn empty_subject_records_nothing() {
    let typed = typed("trait Encode { machine encode(&self) -> u64; } data Empty {}");
    let graph = graph(&typed, "Empty");
    let receiver = receiver(&typed, &graph, "Encode");
    assert_eq!(receiver.members().count(), 0);
    let snapshot = receiver.freeze().expect("an empty scope freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");
}

#[test]
fn foreign_scope_snapshot_replays_under_its_claim() {
    let typed = typed(
        "trait Encode { machine encode(&self) -> u64; } \
         data Health { v: u32; } \
         machine Health::encode(&self) -> u64 satisfies Encode::encode { 0 } \
         HealthEnc: Health satisfies Encode { Encode::encode = Health::encode; } \
         pub data Open { health: Health; } \
         data Private { health: Health; }",
    );
    let open_graph = construct_semantic_schema_graph(&typed, subject(&typed, "Open"), foreign())
        .expect("a public subject admits a foreign scope");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &open_graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(&typed, "Encode", None),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            conformance_choice(&typed, "HealthEnc"),
        )
        .expect("selects");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot).expect("replays");

    // The frozen authority is a claim, not a grant: replaying the same
    // snapshot under an owning claim over the same public subject
    // re-checks cleanly, while a foreign claim over a private subject
    // cannot be manufactured past replay.
    let mut forged = snapshot.clone();
    forged.authority = owning();
    reseal(&mut forged);
    replay_selection_snapshot(&typed, &forged)
        .expect("the same public subject satisfies an owning claim");

    let private_graph = graph(&typed, "Private");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &private_graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        requirement(&typed, "Encode", None),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            conformance_choice(&typed, "HealthEnc"),
        )
        .expect("selects");
    let mut snapshot = receiver.freeze().expect("freezes");
    snapshot.authority = foreign();
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a foreign claim over a private subject must reject");
    assert!(error.contains("non-public"), "{error}");
}

#[test]
fn replay_rejects_forged_member_identity() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .select(
            &receiver.member("speed").expect("key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("selects");
    let mut snapshot = receiver.freeze().expect("freezes");
    snapshot.records[0].member_identity = "Player::stamina".to_owned();
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a forged member identity must reject");
    assert!(error.contains("does not match the bound member"), "{error}");
}

#[test]
fn replay_rejects_stale_qualified_type() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .select(
            &receiver.member("speed").expect("key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("selects");
    let mut snapshot = receiver.freeze().expect("freezes");
    snapshot.records[0].qualified_type = Some("stale".to_owned());
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a stale key/type pair must reject");
    assert!(error.contains("does not match the bound member"), "{error}");
}

#[test]
fn replay_rejects_dropped_and_duplicated_records() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .select(
            &receiver.member("speed").expect("key"),
            machine_choice(&typed, "Speed::encode"),
        )
        .expect("selects");
    let mut dropped = receiver_dummy(&typed, &graph);
    dropped.records.pop();
    reseal(&mut dropped);
    let error =
        replay_selection_snapshot(&typed, &dropped).expect_err("a dropped record must reject");
    assert!(error.contains("records 1 members"), "{error}");

    let mut duplicated = receiver_dummy(&typed, &graph);
    let extra = duplicated.records[0].clone();
    duplicated.records.push(extra);
    reseal(&mut duplicated);
    let error = replay_selection_snapshot(&typed, &duplicated)
        .expect_err("a duplicated record must reject");
    assert!(error.contains("records 3 members"), "{error}");
}

fn receiver_dummy(typed: &TypedTrees, graph: &SemanticSchemaGraph) -> SelectionSnapshot {
    let mut receiver = receiver(typed, graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("key"),
            machine_choice(typed, "Health::encode"),
        )
        .expect("selects");
    receiver
        .select(
            &receiver.member("speed").expect("key"),
            machine_choice(typed, "Speed::encode"),
        )
        .expect("selects");
    receiver.freeze().expect("freezes")
}

#[test]
fn replay_rejects_forged_choice_identities() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");

    // An identity that resolves nowhere.
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.records[0].choice = SelectionChoice::Machine {
        machine_identity: "No::such_machine".to_owned(),
    };
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("an unresolvable choice must reject");
    assert!(error.contains("does not resolve"), "{error}");

    // An `Encode` conformance on `Speed` cannot cover `health: Health`.
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.records[0].choice = conformance_choice(&typed, "SpeedEnc");
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a conformance on the wrong carrier must reject");
    assert!(error.contains("does not cover"), "{error}");

    // A `Hash` machine cannot satisfy the frozen `Encode` requirement.
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.records[0].choice = machine_choice(&typed, "Health::hash");
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a machine outside the required contract must reject");
    assert!(error.contains("does not realize"), "{error}");
}

#[test]
fn replay_rejects_forged_requirement() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.requirement = requirement(&typed, "Hash", None);
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("choices that realize only `Encode` cannot satisfy a forged `Hash` claim");
    assert!(error.contains("does not realize"), "{error}");
}

#[test]
fn replay_rejects_projection_drift() {
    let typed = typed(
        "trait Encode { machine encode(&self) -> u64; } \
         machine Rec::encode(&self) -> u64 satisfies Encode::encode { 0 } \
         RecEnc: Rec satisfies Encode { Encode::encode = Rec::encode; } \
         data Rec { secret [erased]: i32; tag: u8; }",
    );
    let graph = graph(&typed, "Rec");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::RuntimeMembers,
        SelectionCoverage::Partial,
        requirement(&typed, "Encode", None),
    )
    .expect("receiver opens");
    receiver
        .exclude(&receiver.member("tag").expect("key"))
        .expect("exclusion records");
    let mut snapshot = receiver.freeze().expect("freezes");
    // Widening the frozen projection reintroduces the erased member the
    // runtime scope deliberately excludes — replay re-derives the scope
    // and rejects the now-uncovered member.
    snapshot.projection = SelectionProjection::DeclaredMembers;
    reseal(&mut snapshot);
    let error =
        replay_selection_snapshot(&typed, &snapshot).expect_err("a widened projection must reject");
    assert!(
        error.contains("records 1 members but its projection covers 2"),
        "{error}"
    );
}

#[test]
fn replay_rejects_unresolvable_subject() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.subject_owner_identity = "Forged::Owner".to_owned();
    reseal(&mut snapshot);
    let error =
        replay_selection_snapshot(&typed, &snapshot).expect_err("an unbound subject must reject");
    assert!(error.contains("does not resolve"), "{error}");
}

#[test]
fn replay_rejects_tampered_revision() {
    let typed = typed(ENCODE_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut snapshot = receiver_dummy(&typed, &graph);
    snapshot.revision = snapshot.revision.wrapping_add(1);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("a tampered report fingerprint must reject");
    assert!(error.contains("fingerprint"), "{error}");
}

#[test]
fn demanded_application_accepts_the_pinned_application() {
    let typed = typed(APPLICATION_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        application_requirement(&typed, "Encode", None, "HealthJson"),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("health key"),
            machine_choice(&typed, "Health::encode"),
        )
        .expect("the machine at `Encode<Json>` satisfies the `Encode<Json>` demand");
    let snapshot = receiver.freeze().expect("freezes");
    replay_selection_snapshot(&typed, &snapshot)
        .expect("replay re-checks the pinned application and passes");
}

#[test]
fn demanded_application_rejects_realizations_at_another_application() {
    let typed = typed(APPLICATION_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        application_requirement(&typed, "Encode", None, "HealthJson"),
    )
    .expect("receiver opens");
    let error = receiver
        .select(
            &receiver.member("health").expect("health key"),
            machine_choice(&typed, "Health::encode_cbor"),
        )
        .expect_err("a `satisfies` edge at `Encode<Cbor>` must not satisfy `Encode<Json>`");
    assert!(error.contains("does not realize"), "{error}");
    let error = receiver
        .select(
            &receiver.member("health").expect("health key"),
            conformance_choice(&typed, "HealthCbor"),
        )
        .expect_err("a conformance at `Encode<Cbor>` must not satisfy `Encode<Json>`");
    assert!(error.contains("application"), "{error}");
}

#[test]
fn undemanded_application_accepts_any_application() {
    let typed = typed(APPLICATION_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = receiver(&typed, &graph, "Encode");
    receiver
        .select(
            &receiver.member("health").expect("health key"),
            machine_choice(&typed, "Health::encode_cbor"),
        )
        .expect("without a pinned application any `Encode` realization satisfies");
    receiver.freeze().expect("freezes");
}

#[test]
fn replay_rechecks_the_demanded_application() {
    let typed = typed(APPLICATION_PROGRAM);
    let graph = graph(&typed, "Player");
    let mut receiver = ScopedSelectionReceiver::new(
        &typed,
        &graph,
        SelectionProjection::DeclaredMembers,
        SelectionCoverage::Complete,
        application_requirement(&typed, "Encode", None, "HealthJson"),
    )
    .expect("receiver opens");
    receiver
        .select(
            &receiver.member("health").expect("health key"),
            conformance_choice(&typed, "HealthJson"),
        )
        .expect("the conformance at `Encode<Json>` selects");
    let mut snapshot = receiver.freeze().expect("freezes");
    snapshot.records[0].choice = conformance_choice(&typed, "HealthCbor");
    reseal(&mut snapshot);
    let error = replay_selection_snapshot(&typed, &snapshot)
        .expect_err("replay must re-check the demanded application, not trust the freeze");
    assert!(error.contains("application"), "{error}");
}

#[test]
fn demanded_application_checks_arity_at_construction() {
    let typed = typed(APPLICATION_PROGRAM);
    let definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Encode")
        .expect("trait `Encode` exists");
    let error =
        SelectionRequirement::for_trait_application(&typed, definition.symbol, None, &[], &[])
            .expect_err("an application missing declared parameters must not construct");
    assert!(error.contains("type parameter"), "{error}");
}
