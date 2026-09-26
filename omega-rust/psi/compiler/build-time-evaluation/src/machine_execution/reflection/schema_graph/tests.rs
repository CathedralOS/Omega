//! Semantic schema graph tests.

use super::{
    FieldDescription, SchemaNode, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
    construct_semantic_schema_graph, replay_semantic_schema_graph,
};
use crate::machine_execution::reflection::schema_graph::graph_replay::graph_revision;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

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

fn field<'graph>(graph: &'graph SemanticSchemaGraph, name: &str) -> &'graph FieldDescription {
    graph
        .fields()
        .find(|(_, field)| field.name == name)
        .unwrap_or_else(|| panic!("field `{name}` exists"))
        .1
}

#[test]
fn record_graph_describes_qualified_members() {
    let typed = crate::front_end::typed_program("data Player { health: u32; speed: f32; }");
    let graph = construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
        .expect("record schema constructs");

    let declaration = graph.declaration();
    assert_eq!(declaration.name, "Player");
    assert_eq!(declaration.shape, SchemaShape::Record);
    assert_eq!(declaration.members.len(), 2);

    let health = field(&graph, "health");
    assert_eq!(health.member_identity, "Player::health");
    assert_eq!(health.declaration_position, 0);
    assert!(!health.erased);
    assert!(health.qualified_type.contains("u32"));
    // `u32` is a nominal domain declaration, so the field carries one
    // interned edge node rather than expanding the domain's members.
    assert_eq!(health.nominal_references.len(), 1);
    let SchemaNode::NominalReference(reference) = graph.node(health.nominal_references[0]) else {
        panic!("the `u32` edge resolves to a nominal reference node")
    };
    assert_eq!(reference.name, "u32");
    let speed = field(&graph, "speed");
    assert_eq!(speed.declaration_position, 1);
    assert_eq!(speed.nominal_references.len(), 1);

    replay_semantic_schema_graph(&typed, &graph).expect("constructed graph replays");
}

#[test]
fn erased_and_empty_members_remain_described() {
    let typed =
        crate::front_end::typed_program("data Rec { proof [erased]: i32; tag: u8; } data Empty {}");
    let graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), owning()).expect("ok");
    let proof = field(&graph, "proof");
    assert!(proof.erased, "semantic schema retains erased members");
    let empty =
        construct_semantic_schema_graph(&typed, subject(&typed, "Empty"), owning()).expect("ok");
    assert_eq!(empty.declaration().shape, SchemaShape::Empty);
    assert!(empty.declaration().members.is_empty());
    replay_semantic_schema_graph(&typed, &graph).expect("replays");
    replay_semantic_schema_graph(&typed, &empty).expect("empty replays");
}

#[test]
fn sum_graph_keeps_nullary_cases_distinct_and_owns_payloads() {
    let typed = crate::front_end::typed_program(
        "data Outcome { case Ready(value: u32); case Empty; case Done; }",
    );
    let graph = construct_semantic_schema_graph(&typed, subject(&typed, "Outcome"), owning())
        .expect("sum schema constructs");
    let cases: Vec<_> = graph.cases().collect();
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[0].1.name, "Ready");
    assert_eq!(cases[1].1.name, "Empty");
    assert_eq!(cases[2].1.name, "Done");
    assert_ne!(
        cases[1].1.member_identity, cases[2].1.member_identity,
        "two nullary cases stay distinguishable"
    );
    assert_eq!(cases[0].1.payload_fields.len(), 1);
    assert!(cases[1].1.payload_fields.is_empty());
    let payload_handle = cases[0].1.payload_fields[0];
    let SchemaNode::Field(payload) = graph.node(payload_handle) else {
        panic!("payload member is a field node")
    };
    assert_eq!(payload.name, "value");
    assert_eq!(payload.owner_case, cases[0].0);
    assert_eq!(payload.declaration_position, 0);
    replay_semantic_schema_graph(&typed, &graph).expect("replays");
}

#[test]
fn recursive_reference_is_an_edge_not_expansion() {
    let typed = crate::front_end::typed_program(
        "data List { case Nil; case Cons(head: u32, tail: List); }",
    );
    let graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "List"), owning()).expect("ok");
    let cases: Vec<_> = graph.cases().collect();
    let cons = cases
        .iter()
        .find(|(_, case)| case.name == "Cons")
        .expect("Cons");
    assert_eq!(cons.1.payload_fields.len(), 2);
    let SchemaNode::Field(tail) = graph.node(cons.1.payload_fields[1]) else {
        panic!("tail is a field")
    };
    assert_eq!(
        tail.nominal_references,
        vec![graph.root()],
        "a member typed as the selected application reuses the pending root identity"
    );
    replay_semantic_schema_graph(&typed, &graph).expect("replays");
}

#[test]
fn foreign_references_deduplicate_into_shared_nodes() {
    let typed = crate::front_end::typed_program(
        "data Inner { x: u8; } data Outer { inner: Inner; items: [Inner; 2]; tag: u8; }",
    );
    let graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Outer"), owning()).expect("ok");
    let inner = field(&graph, "inner");
    let items = field(&graph, "items");
    assert_eq!(inner.nominal_references.len(), 1);
    assert_eq!(
        inner.nominal_references, items.nominal_references,
        "the same application interns one reference node"
    );
    let SchemaNode::NominalReference(reference) = graph.node(inner.nominal_references[0]) else {
        panic!("nominal edge resolves to a reference node")
    };
    assert_eq!(reference.name, "Inner");
    assert!(reference.owner_identity.contains("Inner"));
    replay_semantic_schema_graph(&typed, &graph).expect("replays");
}

#[test]
fn foreign_scope_requires_a_public_subject() {
    let typed = crate::front_end::typed_program("data Secret { x: u8; } pub data Open { x: u8; }");
    let denied = construct_semantic_schema_graph(&typed, subject(&typed, "Secret"), foreign());
    let error = denied.expect_err("private subject must reject under a foreign scope");
    assert!(error.contains("not public"), "{error}");
    let open = construct_semantic_schema_graph(&typed, subject(&typed, "Open"), foreign())
        .expect("public subject admits a foreign scope");
    replay_semantic_schema_graph(&typed, &open).expect("replays");
}

#[test]
fn quotient_subjects_reject() {
    let typed = crate::front_end::typed_program(
        "data Carrier { case Unit; } proposition same(left: Carrier, right: Carrier) = left == right; data Bucket = Carrier % same;",
    );
    let error = construct_semantic_schema_graph(&typed, subject(&typed, "Bucket"), owning())
        .expect_err("a quotient does not expose its representative");
    assert!(error.contains("quotient"), "{error}");
}

#[test]
fn non_data_subjects_reject() {
    let typed = crate::front_end::typed_program("machine helper() -> u8 { 7 } data Rec { x: u8; }");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "helper")
        .expect("machine")
        .symbol;
    let error = construct_semantic_schema_graph(&typed, machine, owning())
        .expect_err("a machine is not a schema subject");
    assert!(error.contains("data declaration"), "{error}");
}

#[test]
fn replay_rejects_forged_member_identity() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    let handle = graph.declaration().members[0];
    let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
        panic!("member is a field")
    };
    stored.member_identity = "Player::stamina".to_owned();
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("a forged member identity must reject");
    assert!(error.contains("does not match its bound field"), "{error}");
}

#[test]
fn replay_rejects_wrong_qualified_type() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    let handle = graph.declaration().members[0];
    let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
        panic!("member is a field")
    };
    stored.qualified_type = "f32".to_owned();
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("a wrong qualified type must reject");
    assert!(error.contains("does not match its bound field"), "{error}");
}

#[test]
fn replay_rejects_dropped_member() {
    let typed = crate::front_end::typed_program("data Player { health: u32; speed: f32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    graph.nodes[graph.root.index()] = {
        let SchemaNode::Declaration(mut root) = graph.node(graph.root()).clone() else {
            panic!("root is a declaration")
        };
        root.members.pop();
        SchemaNode::Declaration(root)
    };
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error =
        replay_semantic_schema_graph(&typed, &graph).expect_err("a dropped member must reject");
    assert!(
        error.contains("top-level members") || error.contains("orphan"),
        "{error}"
    );
}

#[test]
fn replay_rejects_stale_stable_number() {
    let typed = crate::front_end::typed_program("data Rec { #3 x: u8; #7 y: u8; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), owning()).expect("ok");
    let handle = graph.declaration().members[0];
    let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
        panic!("member is a field")
    };
    stored.stable_number = Some(9);
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("a stale stable number must reject");
    assert!(error.contains("does not match its bound field"), "{error}");
}

#[test]
fn replay_rejects_misowned_payload() {
    let typed =
        crate::front_end::typed_program("data Outcome { case Ready(value: u32); case Empty; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Outcome"), owning()).expect("ok");
    let empty_case = graph
        .cases()
        .find(|(_, case)| case.name == "Empty")
        .expect("Empty case")
        .0;
    let payload_handle = graph
        .cases()
        .find(|(_, case)| case.name == "Ready")
        .expect("Ready case")
        .1
        .payload_fields[0];
    let SchemaNode::Field(stored) = &mut graph.nodes[payload_handle.index()] else {
        panic!("payload is a field")
    };
    stored.owner_case = empty_case;
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error =
        replay_semantic_schema_graph(&typed, &graph).expect_err("a mis-owned payload must reject");
    assert!(error.contains("does not match its bound field"), "{error}");
}

#[test]
fn replay_rejects_extra_orphan_node() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    let SchemaNode::Field(orphan) = graph.node(graph.declaration().members[0]).clone() else {
        panic!("member is a field")
    };
    graph.nodes.push(SchemaNode::Field(orphan));
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("an orphan member node must reject");
    assert!(error.contains("orphan"), "{error}");
}

#[test]
fn replay_rejects_tampered_revision() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    graph.revision = graph.revision.wrapping_add(1);
    let error =
        replay_semantic_schema_graph(&typed, &graph).expect_err("a tampered revision must reject");
    assert!(error.contains("fingerprint"), "{error}");
}

#[test]
fn replay_rejects_unresolvable_owner() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    let SchemaNode::Declaration(root) = &mut graph.nodes[graph.root.index()] else {
        panic!("root is a declaration")
    };
    root.owner_identity = "Forged::Owner".to_owned();
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error =
        replay_semantic_schema_graph(&typed, &graph).expect_err("an unbound owner must reject");
    assert!(error.contains("does not resolve"), "{error}");
}

#[test]
fn replay_rejects_forged_root_presentation() {
    let typed = crate::front_end::typed_program("data Player { health: u32; }");
    let mut graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning()).expect("ok");
    let SchemaNode::Declaration(root) = &mut graph.nodes[graph.root.index()] else {
        panic!("root is a declaration")
    };
    root.name = "Impostor".to_owned();
    root.identity_is_hermetic = !root.identity_is_hermetic;
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("forged root presentation facts must reject");
    assert!(error.contains("name or identity strength"), "{error}");
}

#[test]
fn replay_rejects_foreign_scope_over_private_subject() {
    let typed = crate::front_end::typed_program("data Secret { x: u8; }");
    let mut graph = construct_semantic_schema_graph(&typed, subject(&typed, "Secret"), owning())
        .expect("owning scope admits a private subject");
    graph.authority = foreign();
    graph.revision = graph_revision(&graph.authority, &graph.nodes);
    let error = replay_semantic_schema_graph(&typed, &graph)
        .expect_err("a foreign claim over a private subject must reject");
    assert!(error.contains("non-public"), "{error}");
}

#[test]
fn requester_derives_owning_scope_in_source_free_trees() {
    let typed = crate::front_end::typed_program("machine helper() -> u8 { 7 } data Rec { x: u8; }");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "helper")
        .expect("machine")
        .symbol;
    let authority = SchemaQueryAuthority::for_requester(&typed, machine, subject(&typed, "Rec"))
        .expect("requester resolves");
    assert!(
        matches!(authority, SchemaQueryAuthority::OwningScope { .. }),
        "source-free trees model one package"
    );
    let graph =
        construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), authority).expect("ok");
    replay_semantic_schema_graph(&typed, &graph).expect("replays");
}
