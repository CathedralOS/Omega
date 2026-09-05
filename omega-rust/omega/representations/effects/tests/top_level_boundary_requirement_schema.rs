use effects::provider_plan::ServiceSchema;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    lower_symbol_resolved_trees(&resolved).expect("type source")
}

fn requirement<'program>(
    typed: &'program typed_trees::TypedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("typed top-level requirement")
}

#[test]
fn exact_top_level_requirement_schema_retains_its_typed_operational_shape() {
    let typed = typed_source(
        r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}
        boundary trait Callback {
            machine call();
        }

        pub boundary requirement InterruptAcknowledgement::complete(
            callback: &mut Callback,
            acknowledgement: u32
        ) -> bool
        reaches <= MachineControl + PortIo
        invokes callback;
        invokes MachineControl;
        suspends;
        blocks;
        terminates;
        ensures acknowledgement == acknowledgement;
        "#,
    );
    let requirement = requirement(&typed, "InterruptAcknowledgement::complete");
    let schema = ServiceSchema::from_typed_boundary_requirement(&typed, requirement)
        .expect("public non-generic requirement schema");

    assert_eq!(schema.trait_name, "InterruptAcknowledgement::complete");
    assert_eq!(schema.trait_package_identity, None);
    let [method] = schema.methods.as_slice() else {
        panic!("one exact top-level requirement method")
    };
    assert_eq!(method.name, "complete");
    assert_eq!(method.requirement_owner, "InterruptAcknowledgement");
    assert_eq!(method.requirement_owner_package_identity, None);
    assert_eq!(
        method.requirement_identity,
        typed
            .normalized_machine_overload_identity(requirement)
            .expect("normalized requirement identity")
            .identity()
    );
    assert_eq!(method.parameter_count, 2);
    assert_eq!(
        method.parameter_type_identities,
        ["ref-mut(named(name(Callback)))", "named(name(u32))"]
    );
    assert!(method.entry_claims.is_empty());
    assert!(method.has_result);
    assert_eq!(
        method.result_type_identity.as_deref(),
        Some("named(name(bool))")
    );
    assert!(method.result_claims.is_empty());
    assert_eq!(
        method.service_reach,
        ["Callback", "MachineControl", "PortIo"]
    );
    assert_eq!(
        method.synchronous_invocations,
        ["Callback", "MachineControl"]
    );
    assert!(method.may_suspend);
    assert!(method.may_block);
    assert!(method.terminates_guarantee);
    assert!(method.termination_premises.is_empty());
    assert_eq!(method.calling_plan_report_fingerprint, None);
    assert_eq!(method.calling_plan_commitment, None);
}

#[test]
fn top_level_requirement_schema_fences_non_public_and_generic_declarations() {
    let typed = typed_source(
        r#"
        boundary requirement Private::complete(value: u32);
        pub boundary requirement StaticGeneric::complete<T>(value: T);
        pub boundary requirement LifetimeGeneric::complete<'a>(value: &'a u32);
        pub boundary requirement Plain::complete(value: u32);
        "#,
    );

    for name in [
        "Private::complete",
        "StaticGeneric::complete",
        "LifetimeGeneric::complete",
    ] {
        assert!(
            ServiceSchema::from_typed_boundary_requirement(&typed, requirement(&typed, name))
                .is_none(),
            "{name} must remain outside the first planning rung"
        );
    }
    assert!(
        ServiceSchema::from_typed_boundary_requirement(
            &typed,
            requirement(&typed, "Plain::complete")
        )
        .is_some()
    );
}

#[test]
fn top_level_requirement_schema_rejects_wrong_supply_and_unpublished_termination() {
    let mut typed = typed_source("pub boundary requirement Carrier::operation(value: u32);");
    let requirement_symbol = requirement(&typed, "Carrier::operation").symbol;

    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == requirement_symbol)
        .expect("mutable requirement")
        .supply_mode = language_semantics::MachineSupplyMode::Boundary;
    assert!(
        ServiceSchema::from_typed_boundary_requirement(
            &typed,
            requirement(&typed, "Carrier::operation")
        )
        .is_none()
    );

    let mut typed = typed_source("pub boundary requirement Carrier::operation(value: u32);");
    let requirement_symbol = requirement(&typed, "Carrier::operation").symbol;
    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == requirement_symbol)
        .expect("mutable requirement")
        .body_is_present = true;
    assert!(
        ServiceSchema::from_typed_boundary_requirement(
            &typed,
            requirement(&typed, "Carrier::operation")
        )
        .is_none()
    );

    let mut typed = typed_source("pub boundary requirement Carrier::operation(value: u32);");
    let requirement_symbol = requirement(&typed, "Carrier::operation").symbol;
    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == requirement_symbol)
        .expect("mutable requirement")
        .termination_plan
        .interface = language_semantics::TerminationInterface::InternalDerived;
    assert!(
        ServiceSchema::from_typed_boundary_requirement(
            &typed,
            requirement(&typed, "Carrier::operation")
        )
        .is_none()
    );
}
