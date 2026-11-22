use crate::TypedTrees;
use crate::source_state::expression::ExpressionNode;
use crate::source_symbols::{SymbolTable, SymbolTableBuilder};
use std::sync::Arc;
use symbols::{SymbolKind, SymbolNameRef};

fn symbols(name: &str) -> SymbolTable {
    let mut builder = SymbolTableBuilder::new();
    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Borrowed(name));
    builder.finish()
}

#[test]
fn selected_continuation_retains_side_payloads_and_isolates_changes() {
    use crate::source_state::deferred_refusals::DeferredRequest;

    let mut builder = SymbolTableBuilder::new();
    let owner = builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
    let mut original = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    original.record_deferred_refusal(
        owner,
        DeferredRequest::ReceiverCallSelection,
        source_text::SourceSpan::default(),
    );
    original.authored_service_reach_rows.push(
        crate::source_state::signature::AuthoredServiceReachRow {
            owner,
            keyword_source_spans: vec![source_text::SourceSpan::default()],
            targets: Vec::new(),
            installation_bound: false,
        },
    );
    assert!(original.applied_domain_applications.insert(
        crate::source_state::domain::AppliedDomainApplication {
            declaration: owner,
            identity: semantic_vocabulary::SemanticDomainId(1),
            arguments: Vec::new(),
            is_closed: true,
            theory: None,
        }
    ));
    let invocation = original
        .access_policy_invocations
        .insert(source_text::SourceSpan::default());
    let original = Arc::new(original);
    let predecessor = Arc::clone(&original);
    let mut successor = original.with_replaced_symbols(predecessor.symbols.clone());
    let fork = Arc::get_mut(&mut successor).unwrap();

    assert!(std::ptr::eq(
        &predecessor.deferred_refusals[0] as *const _,
        &fork.deferred_refusals[0] as *const _
    ));
    assert!(std::ptr::eq(
        &predecessor.authored_service_reach_rows[0] as *const _,
        &fork.authored_service_reach_rows[0] as *const _
    ));
    assert!(std::ptr::eq(
        predecessor.applied_domain_applications.get(0).unwrap(),
        fork.applied_domain_applications.get(0).unwrap()
    ));
    assert!(std::ptr::eq(
        predecessor.access_policy_invocations.get(invocation),
        fork.access_policy_invocations.get(invocation)
    ));

    fork.record_deferred_refusal(
        owner,
        DeferredRequest::UndeclaredCallName,
        source_text::SourceSpan::default(),
    );
    fork.authored_service_reach_rows[0].installation_bound = true;
    fork.applied_domain_applications
        .set_theory(0, Err("new preparation refusal".into()));
    assert!(fork.access_policy_invocations.free(invocation));
    let replacement = fork
        .access_policy_invocations
        .insert(source_text::SourceSpan::default());

    assert_eq!(
        predecessor.deferred_refusal(owner).unwrap().request,
        DeferredRequest::ReceiverCallSelection
    );
    assert_eq!(
        fork.deferred_refusal(owner).unwrap().request,
        DeferredRequest::UndeclaredCallName
    );
    assert!(!predecessor.authored_service_reach_rows[0].installation_bound);
    assert!(fork.authored_service_reach_rows[0].installation_bound);
    assert!(
        predecessor
            .applied_domain_applications
            .get(0)
            .unwrap()
            .theory
            .is_none()
    );
    assert!(
        fork.applied_domain_applications
            .get(0)
            .unwrap()
            .theory
            .as_ref()
            .unwrap()
            .is_err()
    );
    assert_eq!(replacement.arena_index(), invocation.arena_index());
    assert_ne!(replacement.generation(), invocation.generation());
    assert!(predecessor.access_policy_invocations.is_valid(invocation));
    assert!(!fork.access_policy_invocations.is_valid(invocation));
    assert!(!predecessor.access_policy_invocations.is_valid(replacement));
}

#[test]
fn selected_continuation_shares_callable_children_and_isolates_writes() {
    use crate::source_state::name::Identifier;
    use crate::source_state::{callable_applications, domain, signature, state};

    let mut original = TypedTrees::default();
    let parameters = original
        .state_parameters
        .insert_many((0..260).map(|_| signature::StateParameter::default()));
    let facts = original
        .proof_facts
        .insert_many([domain::ProofFact::default(), domain::ProofFact::default()]);
    let contracts = original.signature_contracts.insert_many([
        signature::SignatureContract {
            facts,
            ..Default::default()
        },
        signature::SignatureContract::default(),
    ]);
    let states = original.machine_states.insert_many([
        state::State {
            parameters,
            contracts,
            ..Default::default()
        },
        state::State::default(),
    ]);
    let signatures = original.trait_machine_signatures.insert_many([
        signature::StateSignature {
            parameters,
            contracts,
            ..Default::default()
        },
        signature::StateSignature::default(),
    ]);
    let applied = original
        .applied_callable_signatures
        .insert(callable_applications::AppliedCallableSignature::default());
    let other_applied = original
        .applied_callable_signatures
        .insert(callable_applications::AppliedCallableSignature::default());
    let original = Arc::new(original);
    let predecessor = Arc::clone(&original);
    let mut successor = original.with_replaced_symbols(symbols("successor"));
    let fork = Arc::get_mut(&mut successor).unwrap();

    macro_rules! shares {
        ($table:ident, $handle:expr) => {
            assert!(std::ptr::eq(
                predecessor.$table.get($handle),
                fork.$table.get($handle)
            ));
        };
    }
    shares!(state_parameters, parameters.start());
    shares!(proof_facts, facts.start());
    shares!(signature_contracts, contracts.start());
    shares!(machine_states, states.start());
    shares!(trait_machine_signatures, signatures.start());
    shares!(applied_callable_signatures, applied);

    fork.state_parameters.get_mut(parameters.start()).name = Identifier::generated("changed");
    fork.proof_facts.set(
        facts.start(),
        domain::ProofFact::Expression(arena::Handle::from_parts(1, 1)),
    );
    fork.signature_contracts
        .get_mut(contracts.start())
        .binding = Some(Identifier::generated("changed"));
    fork.machine_states.get_mut(states.start()).name = Identifier::generated("changed");
    fork.trait_machine_signatures
        .get_mut(signatures.start())
        .name = Identifier::generated("changed");
    fork.applied_callable_signatures
        .replace(
            applied,
            callable_applications::AppliedCallableSignature::default(),
        )
        .unwrap();
    assert!(!std::ptr::eq(
        predecessor.applied_callable_signatures.get(applied),
        fork.applied_callable_signatures.get(applied)
    ));

    assert_eq!(
        predecessor.state_parameters.get(parameters.start()).name,
        Identifier::default()
    );
    assert_eq!(
        predecessor.proof_facts.get(facts.start()),
        &domain::ProofFact::default()
    );
    assert_eq!(
        predecessor
            .signature_contracts
            .get(contracts.start())
            .binding,
        None
    );
    assert_eq!(
        predecessor.machine_states.get(states.start()).name,
        Identifier::default()
    );
    assert_eq!(
        predecessor
            .trait_machine_signatures
            .get(signatures.start())
            .name,
        Identifier::default()
    );
    assert_eq!(
        predecessor.applied_callable_signatures.get(applied),
        &callable_applications::AppliedCallableSignature::default()
    );
    shares!(state_parameters, parameters.handle_at(259));
    shares!(proof_facts, facts.handle_at(1));
    shares!(signature_contracts, contracts.handle_at(1));
    shares!(machine_states, states.handle_at(1));
    shares!(trait_machine_signatures, signatures.handle_at(1));
    shares!(applied_callable_signatures, other_applied);
    assert!(fork.applied_callable_signatures.free(applied));
    let recycled = fork
        .applied_callable_signatures
        .insert(callable_applications::AppliedCallableSignature::default());
    assert_eq!(recycled.arena_index(), applied.arena_index());
    assert_ne!(recycled.generation(), applied.generation());
    assert!(!fork.applied_callable_signatures.is_valid(applied));
    assert!(predecessor.applied_callable_signatures.is_valid(applied));
    assert!(!predecessor.applied_callable_signatures.is_valid(recycled));
}

#[test]
fn selected_continuation_shares_data_children_and_replaces_only_its_application() {
    use crate::source_state::data::{
        DataDefinition, DataField, DataMember, DataVariant, TypeParameter,
    };
    use crate::source_state::name::Identifier;
    use crate::source_state::types::TypeReferenceNode;

    let mut original = TypedTrees::default();
    let parameters = original.data_type_parameters.insert_many([
        TypeParameter {
            name: Identifier::generated("first"),
            ..Default::default()
        },
        TypeParameter {
            name: Identifier::generated("other"),
            ..Default::default()
        },
    ]);
    let payload = original.data_payload_fields.insert_many([DataField {
        name: Identifier::generated("payload"),
        ..Default::default()
    }]);
    let members = original.data_members.insert_many([
        DataMember::Field(DataField {
            name: Identifier::generated("field"),
            ..Default::default()
        }),
        DataMember::Variant(DataVariant {
            payload,
            ..Default::default()
        }),
    ]);
    let mut applications = Vec::new();
    for name in ["first", "other"] {
        let reference = original
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated(name),
            });
        original.retain_data_application_definition(
            reference,
            DataDefinition {
                name: Identifier::generated(name),
                type_parameters: parameters,
                members,
                ..Default::default()
            },
            false,
        );
        applications.push(reference);
    }
    let original = Arc::new(original);
    let predecessor = Arc::clone(&original);
    let mut successor = original.with_replaced_symbols(symbols("successor"));
    let fork = Arc::get_mut(&mut successor).unwrap();
    assert!(std::ptr::eq(
        predecessor.data_type_parameters.get(parameters.start()),
        fork.data_type_parameters.get(parameters.start())
    ));
    assert!(std::ptr::eq(
        predecessor.data_members.get(members.start()),
        fork.data_members.get(members.start())
    ));
    assert!(std::ptr::eq(
        predecessor.data_payload_fields.get(payload.start()),
        fork.data_payload_fields.get(payload.start())
    ));
    let row =
        arena::Handle::from_parts(applications[0].arena_index(), applications[0].generation());
    assert!(std::ptr::eq(
        predecessor.data_application_definitions.get(row),
        fork.data_application_definitions.get(row)
    ));

    fork.data_type_parameters.get_mut(parameters.start()).name =
        Identifier::generated("changed");
    fork.data_payload_fields.get_mut(payload.start()).name =
        Identifier::generated("changed payload");
    let DataMember::Field(field) = fork.data_members.get_mut(members.start()) else {
        panic!("authored field")
    };
    field.name = Identifier::generated("changed field");
    fork.discard_open_data_application(applications[0]);
    assert!(fork.data_application_definition(applications[0]).is_none());
    assert!(
        predecessor
            .data_application_definition(applications[0])
            .is_some()
    );
    assert!(std::ptr::eq(
        predecessor
            .data_application_definition(applications[1])
            .unwrap(),
        fork.data_application_definition(applications[1]).unwrap()
    ));
    assert!(std::ptr::eq(
        predecessor
            .data_type_parameters
            .get(parameters.handle_at(1)),
        fork.data_type_parameters.get(parameters.handle_at(1))
    ));
    assert_eq!(
        predecessor
            .data_type_parameters
            .get(parameters.start())
            .name
            .as_str(),
        "first"
    );
    assert_eq!(
        predecessor
            .data_payload_fields
            .get(payload.start())
            .name
            .as_str(),
        "payload"
    );
    let DataMember::Field(field) = predecessor.data_members.get(members.start()) else {
        panic!("authored field")
    };
    assert_eq!(field.name.as_str(), "field");
    let old = predecessor
        .data_application_definition(applications[1])
        .unwrap();
    assert_eq!(fork.data_members(old).len(), 2);
    assert!(matches!(fork.data_members(old)[1], DataMember::Variant(_)));
}

#[test]
fn shared_continuation_moves_replacement_symbols_and_preserves_the_predecessor() {
    let mut original = TypedTrees {
        symbols: symbols("old"),
        ..Default::default()
    };
    let expression = original
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let original = Arc::new(original);
    let predecessor = Arc::clone(&original);
    let replacement = symbols("new");
    let replacement_rows = std::ptr::from_ref(replacement.get(replacement.root()));
    let successor = original.with_replaced_symbols(replacement);
    assert_eq!(
        std::ptr::from_ref(successor.symbols.get(successor.symbols.root())),
        replacement_rows
    );
    assert_eq!(predecessor.symbols.name(predecessor.symbols.root()), "old");
    assert_eq!(successor.symbols.name(successor.symbols.root()), "new");
    assert_eq!(
        successor.expression_table.expression(expression),
        &ExpressionNode::Boolean(true)
    );
    assert_eq!(
        predecessor.expression_table.expression(expression),
        &ExpressionNode::Boolean(true)
    );
    assert!(!Arc::ptr_eq(&predecessor, &successor));
}

#[test]
fn owned_continuation_keeps_its_allocation_and_disassociates_weak_predecessors() {
    let original = Arc::new(TypedTrees::default());
    let storage = Arc::as_ptr(&original);
    let successor = original.with_replaced_symbols(symbols("owned"));
    assert_eq!(Arc::as_ptr(&successor), storage);
    let weak = Arc::downgrade(&successor);
    let successor = successor.with_replaced_symbols(symbols("weak"));
    assert!(weak.upgrade().is_none());
    assert_eq!(successor.symbols.name(successor.symbols.root()), "weak");
    let ordinary_clone = successor.as_ref().clone();
    assert_eq!(ordinary_clone.symbols, successor.symbols);
    assert_eq!(ordinary_clone.tables, successor.tables);
}
