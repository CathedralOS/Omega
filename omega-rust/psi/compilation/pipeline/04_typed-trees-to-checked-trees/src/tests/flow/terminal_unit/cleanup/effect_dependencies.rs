use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::{
    checked_program, checked_program_result, typed_program_from_source_map,
};

fn inferred_services(checked: &checked_trees::CheckedTrees, name: &str) -> Vec<String> {
    inferred_services_for(checked, machine_named(checked, name))
}

fn inferred_services_for(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Vec<String> {
    let row = checked.facts.service_reaches.for_machine(machine).unwrap();
    checked
        .facts
        .service_reaches
        .rows
        .services(row.inferred_transitive)
        .iter()
        .map(|service| {
            checked
                .facts
                .service_reaches
                .services
                .definition(*service)
                .unwrap()
                .name
                .to_string()
        })
        .collect()
}

#[test]
fn selected_cleanup_dependency_specializes_generic_consumers_independently() {
    let checked = checked_program(
        r#"
        pub boundary trait Console { machine write_byte(byte: u8) reaches Console terminates; }
        data Token { value: u8; }
        data Plain { value: u8; }
        machine Token::drop(token: &mut Token) reaches Console { Console::write_byte(token.value); }
        machine consume<T>(value: T) {}
        machine hooked(token: Token) { consume(token); }
        machine plain(value: Plain) { consume(value); }
        "#,
    );
    assert_eq!(inferred_services(&checked, "hooked"), ["Console"]);
    assert!(inferred_services(&checked, "plain").is_empty());
    let template = machine_named(&checked, "consume");
    let instances = checked
        .machine_specializations
        .iter()
        .filter(|row| row.template == template)
        .map(|row| row.instance)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert_eq!(
        instances
            .iter()
            .filter(|instance| !inferred_services_for(&checked, **instance).is_empty())
            .count(),
        1,
    );
    let dependencies = &checked.facts.operational.cleanup_dependencies;
    assert!(
        !dependencies
            .iter()
            .any(|(_, edge)| edge.caller_machine == template)
    );
    assert_eq!(
        dependencies
            .iter()
            .filter(|(_, edge)| instances.contains(&edge.caller_machine))
            .count(),
        1,
    );
    assert!(
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .any(|(_, event)| {
                event.machine_symbol == template
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            }),
        "the retained template keeps its symbolic source disposition",
    );

    let mut incomplete = checked.facts.operational.clone();
    let call = incomplete
        .calls
        .iter()
        .find(|(_, call)| instances.contains(&call.target_machine_symbol))
        .expect("a concrete caller selects an instance")
        .0;
    incomplete.calls.get_mut(call).target_machine_symbol = template;
    incomplete.calls.get_mut(call).target_state_symbol =
        checked.machine_states(checked.machine_by_symbol(template).unwrap())[0].symbol;
    let errors = crate::flow::derive_cleanup_effect_dependencies(
        &checked.typed,
        &incomplete,
        &checked.facts,
        &crate::checks::CleanupDispositions::default(),
    )
    .expect_err("a retained template cannot stand in for executable instance effects");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("complete specialization"))
    );
}

#[test]
fn selected_cleanup_dependency_specialized_forwarding_preserves_transfer() {
    let checked = checked_program(
        r#"
        pub boundary trait Console { machine write_byte(byte: u8) reaches Console terminates; }
        data Token { value: u8; }
        machine Token::drop(token: &mut Token) reaches Console { Console::write_byte(token.value); }
        machine forward<T>(value: T) -> T { value }
        machine caller(token: Token) -> Token { forward(token) }
        "#,
    );
    assert!(inferred_services(&checked, "caller").is_empty());
    let template = machine_named(&checked, "forward");
    let instance = checked
        .machine_specializations
        .iter()
        .find(|row| row.template == template)
        .expect("a closed forwarding application")
        .instance;
    assert!(inferred_services_for(&checked, instance).is_empty());
    assert!(
        !checked
            .facts
            .operational
            .cleanup_dependencies
            .iter()
            .any(|(_, edge)| {
                edge.caller_machine == instance || edge.caller_machine == template
            })
    );
}

#[test]
fn selected_cleanup_dependency_reaches_private_consumers_and_their_callers() {
    let checked = checked_program(
        r#"
        pub boundary trait Console { machine write_byte(byte: u8) reaches Console terminates; }
        data Token { value: u8; }
        machine Token::drop(token: &mut Token) reaches Console { Console::write_byte(token.value); }
        machine consume(token: Token) {}
        machine forward(token: Token) { consume(token); }
    "#,
    );
    assert_eq!(inferred_services(&checked, "consume"), ["Console"]);
    assert_eq!(inferred_services(&checked, "forward"), ["Console"]);
    let consume = machine_named(&checked, "consume");
    let forward = machine_named(&checked, "forward");
    let dependencies = &checked.facts.operational.cleanup_dependencies;
    assert!(
        dependencies
            .iter()
            .any(|(_, edge)| edge.caller_machine == consume)
    );
    assert!(
        !dependencies
            .iter()
            .any(|(_, edge)| edge.caller_machine == forward),
        "forward's call transfers its owner; only consume performs cleanup"
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(consume)
        .expect("the inferred private consumer retains an executable cleanup plan");
    assert_eq!(plan.cleanups.len(), 1);
}

#[test]
fn selected_cleanup_dependency_excludes_a_returned_owner() {
    let checked = checked_program(
        r#"
        pub boundary trait Console { machine write_byte(byte: u8) reaches Console terminates; }
        data Token { value: u8; }
        machine Token::drop(token: &mut Token) reaches Console { Console::write_byte(token.value); }
        machine forward(token: Token) -> Token { token }
    "#,
    );
    assert!(inferred_services(&checked, "forward").is_empty());
    let forward = machine_named(&checked, "forward");
    assert!(
        !checked
            .facts
            .operational
            .cleanup_dependencies
            .iter()
            .any(|(_, edge)| edge.caller_machine == forward)
    );
}

#[test]
fn selected_cleanup_dependency_does_not_supply_a_missing_hook_boundary_declaration() {
    let errors = checked_program_result(
        r#"
        pub boundary trait Console { machine write_byte(byte: u8) reaches Console terminates; }
        data Token { value: u8; }
        machine Token::drop(token: &mut Token) { Console::write_byte(token.value); }
        machine consume(token: Token) {}
    "#,
    )
    .expect_err("compiler selection does not grant the hook's direct boundary reach");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("undeclared service")
                && error.message.contains("Token::drop")),
        "{errors:?}"
    );
}

#[test]
fn selected_cleanup_dependency_rejects_transitive_external_completion() {
    use source::{SourceMap, SourceOrigin};
    use std::path::PathBuf;
    let core = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/process_exit.omg"
    ));
    let user = r#"
        data Token { value: u8; }
        machine leave() reaches ProcessExit { ProcessExit::exit_process(0); }
        machine Token::drop(token: &mut Token) { leave(); }
        machine consume(token: Token) {}
    "#;
    let root = PathBuf::from("fixture");
    let mut sources = SourceMap::default();
    let core_id = sources
        .add_with_metadata(
            root.join("core/process_exit.omg"),
            core.to_owned(),
            root.join("core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_id = sources
        .add_with_metadata(
            root.join("main.omg"),
            user.to_owned(),
            root,
            None,
            SourceOrigin::User,
        )
        .source_id;
    let typed = typed_program_from_source_map(sources, &[(core_id, core), (user_id, user)]);
    let errors = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .map(|_| ())
        .expect_err("cleanup must return normally even through a private helper");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cleanup machine")
                && error.message.contains("external process exit")),
        "{errors:?}"
    );
}

fn cleanup_targets(checked: &checked_trees::CheckedTrees, caller: &str) -> Vec<String> {
    let caller = machine_named(checked, caller);
    let mut targets = checked
        .facts
        .operational
        .cleanup_dependencies
        .iter()
        .filter(|(_, edge)| edge.caller_machine == caller)
        .map(|(_, edge)| {
            checked
                .machine_by_symbol(edge.target_machine)
                .unwrap()
                .name
                .as_str()
                .to_owned()
        })
        .collect::<Vec<_>>();
    targets.sort();
    targets.dedup();
    targets
}

#[test]
fn selected_cleanup_dependency_excludes_moved_nested_field_but_retains_sibling() {
    let checked = checked_program(
        r#"
        data First { value: u8; }
        data Second { value: u8; }
        machine First::drop(value: &mut First) {}
        machine Second::drop(value: &mut Second) {}
        data Pair { first: First; second: Second; }
        machine take(value: First) {}
        machine consume(pair: Pair) { take(pair.first); }
        machine forward(pair: Pair) -> Pair { pair }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "consume"), ["Second::drop"]);
    assert_eq!(cleanup_targets(&checked, "take"), ["First::drop"]);
    assert!(cleanup_targets(&checked, "forward").is_empty());
}

#[test]
fn selected_cleanup_dependency_tracks_remaining_array_elements() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        machine take(value: Token) {}
        machine partial(values: [Token; 2]) { take(values[0]); }
        machine complete(values: [Token; 2]) { take(values[0]); take(values[1]); }
        machine empty(values: [Token; 0]) {}
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "partial"), ["Token::drop"]);
    assert!(cleanup_targets(&checked, "complete").is_empty());
    assert!(cleanup_targets(&checked, "empty").is_empty());
}

#[test]
fn selected_cleanup_dependency_follows_named_state_transfer() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        data Pair { first: Token; second: Token; }
        machine route(pair: Pair) {
            transition { _ -> next(pair) }
            state next(incoming: Pair) {}
        }
    "#,
    );
    let route = checked
        .machine_by_symbol(machine_named(&checked, "route"))
        .unwrap();
    let states = checked.machine_states(route);
    let entry = states[0].symbol;
    let successor = states
        .iter()
        .find(|state| state.name.as_str() == "next")
        .unwrap()
        .symbol;
    let edges = &checked.facts.operational.cleanup_dependencies;
    assert!(
        !edges
            .iter()
            .any(|(_, edge)| edge.caller_machine == route.symbol && edge.caller_state == entry)
    );
    assert!(
        edges
            .iter()
            .any(|(_, edge)| edge.caller_machine == route.symbol && edge.caller_state == successor)
    );
}

#[test]
fn selected_cleanup_dependency_retains_replacement_before_final_transfer() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        machine replace() -> Token {
            let mut value: Token = Token { value: 1 };
            value = Token { value: 2 };
            value
        }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "replace"), ["Token::drop"]);
}

#[test]
fn selected_cleanup_dependency_excludes_borrowed_and_erased_children() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        data Borrowed { token: &Token; }
        data Erased { proof [erased]: Token; value: u8; }
        machine borrowed(value: Borrowed) {}
        machine erased(value: Erased) {}
    "#,
    );
    assert!(cleanup_targets(&checked, "borrowed").is_empty());
    assert!(cleanup_targets(&checked, "erased").is_empty());
}

#[test]
fn selected_cleanup_dependency_restores_a_replaced_projected_owner() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        data Holder { token: Token; }
        machine take(value: Token) {}
        machine replace() {
            let mut holder: Holder = Holder { token: Token { value: 1 } };
            take(holder.token);
            holder.token = Token { value: 2 };
        }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "replace"), ["Token::drop"]);
    let machine = machine_named(&checked, "replace");
    let events = &checked.facts.flow.ownership.permissions;
    assert!(
        events
            .iter()
            .any(|(_, event)| event.machine_symbol == machine
                && matches!(
                    event.source,
                    language_semantics::PermissionEventSource::Statement { statement_index: 2 }
                )
                && event.kind == language_semantics::PermissionEventKind::Establish)
    );
    assert!(
        !events
            .iter()
            .any(|(_, event)| event.machine_symbol == machine
                && matches!(
                    event.source,
                    language_semantics::PermissionEventSource::Statement { statement_index: 2 }
                )
                && event.kind == language_semantics::PermissionEventKind::AffineDrop),
        "replacing the moved-out field must not dispose its transferred former value"
    );
}

#[test]
fn selected_cleanup_dependency_excludes_proof_only_recursive_values() {
    let checked = checked_program(
        r#"
        data Nat { case Zero; case Succ(prev: Nat); }
        data Wrapper { value: Nat; }
        machine wrap(value: Nat) -> Wrapper { Wrapper { value: value } }
    "#,
    );
    assert!(cleanup_targets(&checked, "wrap").is_empty());
}

#[test]
fn selected_cleanup_dependency_retains_borrowed_temporary_owner() {
    let checked = checked_program(
        r#"
        data Token { value: u8; }
        machine Token::drop(value: &mut Token) {}
        machine make() -> Token { Token { value: 1 } }
        machine inspect(value: &Token) {}
        machine use_temporary() { inspect(&make()); }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "use_temporary"), ["Token::drop"]);
}

#[test]
fn selected_cleanup_dependency_follows_projected_local_binding_transfer() {
    let checked = checked_program(
        r#"
        data First { value: u8; }
        data Second { value: u8; }
        machine First::drop(value: &mut First) {}
        machine Second::drop(value: &mut Second) {}
        data Pair { first: First; second: Second; }
        machine extract(pair: Pair) -> First {
            let moved: First = pair.first;
            moved
        }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "extract"), ["Second::drop"]);
}

#[test]
fn selected_cleanup_dependency_retains_projected_temporary_residual() {
    let checked = checked_program(
        r#"
        data First { value: u8; }
        data Second { value: u8; }
        machine First::drop(value: &mut First) {}
        machine Second::drop(value: &mut Second) {}
        data Pair { first: First; second: Second; }
        machine make() -> Pair { Pair { first: First { value: 1 }, second: Second { value: 2 } } }
        machine take(value: First) {}
        machine use_temporary() { take(make().first); }
    "#,
    );
    assert_eq!(cleanup_targets(&checked, "use_temporary"), ["Second::drop"]);
}
