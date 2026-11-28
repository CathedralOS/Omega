use crate::source_state::signature::{AuthoredInvocationTarget, ReachSubject};

#[test]
fn applied_service_reach_uses_the_declared_parent_argument_and_invokes_only_its_entry() {
    let source = crate::resolve_fixture(
        r#"
        boundary trait Parent<const Id: u64> {}
        boundary trait Fixed<const Id: u64>: Parent<0u64> {}
        boundary trait Forward<const Id: u64>: Parent<Id> {}
        machine parent0() reaches Parent<0u64> {}
        machine parent17() reaches Parent<17u64> {}
        machine parent29() reaches Parent<29u64> {}
        machine fixed() reaches Fixed<17u64> invokes Fixed<17u64> {}
        machine forward() reaches Forward<29u64> {}
        machine same() reaches Parent<17u64> + Parent<17u64> {}
        machine symbolic<const K: u64>() reaches Forward<K> {}
    "#,
    );
    let typed = crate::lower_symbol_resolved_trees(&source).unwrap();
    let machine = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|m| m.name.as_str() == name)
            .unwrap()
    };
    let members = |name: &str| {
        typed
            .authored_service_reach_rows_for(machine(name).symbol)
            .next()
            .unwrap()
            .targets
            .iter()
            .map(|target| match target.service {
                ReachSubject::Service(id) => typed.service_reaches.canonical_id(id),
                _ => {
                    panic!("an explicit boundary application must have a checked effect coordinate")
                }
            })
            .collect::<Vec<_>>()
    };
    let closure = |name| {
        let mut result = Vec::new();
        for id in members(name) {
            typed.service_reaches.extend_closure(id, &mut result);
        }
        result
    };
    let parent0 = members("parent0")[0];
    let parent17 = members("parent17")[0];
    let parent29 = members("parent29")[0];
    assert_ne!(parent17, parent29);
    assert_eq!(closure("same"), [parent17]);
    assert_eq!(closure("fixed"), [members("fixed")[0], parent0]);
    assert_eq!(closure("forward"), [members("forward")[0], parent29]);
    let open = closure("symbolic");
    assert_eq!(open.len(), 2);
    assert_ne!(open[1], parent0);
    assert_ne!(open[1], parent29);
    let invokes = typed
        .signature_invokes
        .span_or_empty(machine("fixed").invokes);
    assert_eq!(invokes.len(), 1);
    let entry = invokes.iter().next().unwrap();
    let AuthoredInvocationTarget::AppliedService(id) = entry.target else {
        panic!("entry lost its application")
    };
    assert_eq!(typed.service_reaches.canonical_id(id), members("fixed")[0]);
}

#[test]
fn applied_service_rejects_a_const_from_the_wrong_integer_carrier() {
    let source = crate::resolve_fixture(
        "boundary trait Sensor<const Id: u64> {} machine invalid() reaches Sensor<17u32> {}",
    );
    let error = crate::lower_symbol_resolved_trees(&source).expect_err("wrong integer carrier");
    assert!(
        error
            .message
            .contains("does not inhabit its declared carrier"),
        "{error:?}"
    );
}

#[test]
fn applied_service_invocation_preserves_a_lexically_shadowing_parameter() {
    // An explicit application cannot turn a callback parameter into a service grant.
    let errors = crate::try_resolve_fixture(
        r#"
        boundary trait Sensor<const Id: u64> {}
        data Callback = machine() reaches none crashes none;
        machine shadow(Sensor: Callback) invokes Sensor<17u64> {}
        "#,
    )
    .expect_err("arguments on the shadowing callback");
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("only when it names a boundary service")),
        "{errors:?}"
    );
}

#[test]
fn a_requirement_family_description_names_its_owner_and_refuses_unprepared_arguments() {
    use crate::source_state::expression::{ExpressionNode, StaticMachineArgument};
    use crate::source_state::reflection::construction::callable_description_identity;

    let source = r#"
        boundary trait Reader {
            machine read<T>(word: u32) -> u32 reaches none crashes none;
        }
        boundary trait Peer {
            machine read<T>(word: u32) -> u32 reaches none crashes none;
        }
        machine main() reaches Reader reaches Peer invokes Reader invokes Peer {
            let first: u32 = Reader::read::<u32>(1);
            let second: u32 = Peer::read::<u32>(1);
        }
        "#;
    let typed = crate::qualified_operator_home_tests::checked(&[("main.omg", source)]).unwrap();
    let requester = typed.realized_machine_named("main").unwrap().symbol;
    let mut descriptions = Vec::new();
    for (_, node) in typed.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let bare = StaticMachineArgument {
            symbol: call.target_symbol,
            ..Default::default()
        };
        let (requirement, identity) =
            callable_description_identity(&typed, &bare, requester).unwrap();
        assert!(requirement);
        descriptions.push(identity);

        assert!(call.authored_machine_application.is_valid());
        let unprepared = StaticMachineArgument {
            authored_machine_application: call.authored_machine_application,
            ..bare
        };
        let refusal = callable_description_identity(&typed, &unprepared, requester).unwrap_err();
        assert!(
            refusal.contains("requires its complete requirement application"),
            "{refusal}"
        );
    }
    let [reader, peer] = descriptions.as_slice() else {
        panic!("expected both requirement descriptions");
    };
    assert_ne!(reader[0], peer[0], "the declaring traits differ");
    assert_ne!(reader[1], peer[1], "the callable identities differ");
    assert_eq!(
        reader[2], peer[2],
        "both declarations share a source revision"
    );
}
