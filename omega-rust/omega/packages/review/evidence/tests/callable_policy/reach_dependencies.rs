use super::*;

const SOURCE: &str = include_str!(
    "../../../../../../../tests/omega/pass/effects/nominal_callback_dependency/main.omg"
);

fn assert_private_closed_uses_preserve_public_policy(case: &str, users: &str) {
    let original = project(&Fixture::local(SOURCE));
    let original_bytes = original.canonical_bytes().unwrap();
    let callbacks = r#"
machine quiet(value: u64) -> u64 satisfies StepContract::step { value }
machine loud(value: u64) -> u64 satisfies StepContract::step reaches Console { value }
"#;
    let fixture = Fixture::local(&format!("{SOURCE}\n{callbacks}\n{users}"));
    let policy = project(&fixture);
    assert_eq!(
        callable(&original, "traverse"),
        callable(&policy, "traverse"),
        "{case}: private closed selections cannot consume the public template contract"
    );
    assert_eq!(
        original, policy,
        "{case}: only the original public surface is published"
    );
    assert_eq!(
        original_bytes,
        policy.canonical_bytes().unwrap(),
        "{case}: recovered canonical policy cannot depend on private selection order"
    );
}

#[test]
fn private_closed_quiet_use_preserves_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "quiet",
        "machine use_quiet(value: u64) -> u64 { traverse<quiet>(value) }",
    );
}

#[test]
fn private_closed_console_use_preserves_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "Console",
        "machine use_loud(value: u64) -> u64 { traverse<loud>(value) }",
    );
}

#[test]
fn private_closed_both_uses_preserve_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "both",
        "machine use_both(value: u64) -> u64 { let next: u64 = traverse<quiet>(value); traverse<loud>(next) }",
    );
}

#[test]
fn private_closed_reversed_uses_preserve_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "reversed",
        "machine use_both(value: u64) -> u64 { let next: u64 = traverse<loud>(value); traverse<quiet>(next) }",
    );
}

#[test]
fn private_closed_relay_uses_preserve_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "private relay",
        "machine relay<machine Forward>(value: u64) -> u64\n\
         where machine Forward satisfies StepContract::step;\n\
         { traverse<Forward>(value) }\n\
         machine use_both(value: u64) -> u64 { let next: u64 = relay<quiet>(value); relay<loud>(next) }",
    );
}

#[test]
fn unused_private_generic_closed_use_preserves_public_generic_policy() {
    assert_private_closed_uses_preserve_public_policy(
        "unused generic caller",
        "machine unused<Element>(witness: &Element, value: u64) -> u64 { traverse<quiet>(value) }",
    );
}

#[test]
fn exported_nominal_reach_dependency_distinguishes_an_additive_private_service() {
    let dependent = project(&Fixture::local(SOURCE));
    let additive = project(&Fixture::local(
        &SOURCE.replace("{ Step(value) }", "{ note(); Step(value) }"),
    ));
    let dependent_traversal = callable(&dependent, "traverse");
    let additive_traversal = callable(&additive, "traverse");
    assert_eq!(
        dependent_traversal.declared_service_reach(),
        additive_traversal.declared_service_reach(),
        "both open applications retain the same conservative Console bound"
    );
    let bound = dependent_traversal
        .declared_service_reach()
        .expect("public bound");
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].path(), "Console");
    assert!(
        dependent_traversal
            .service_reach_dependency()
            .concrete()
            .is_empty()
    );
    assert_eq!(
        dependent_traversal.service_reach_dependency().parameters(),
        &[0]
    );
    assert_eq!(
        additive_traversal.service_reach_dependency().parameters(),
        &[0]
    );
    assert_eq!(
        additive_traversal.service_reach_dependency().concrete(),
        bound
    );
    assert_ne!(
        dependent_traversal, additive_traversal,
        "reach(Step) and reach(Step) + Console are different public dependencies"
    );
    assert_ne!(
        dependent.canonical_bytes().unwrap(),
        additive.canonical_bytes().unwrap(),
        "the distinction must survive canonical policy recovery"
    );
}

#[test]
fn exported_nominal_reach_dependency_ignores_binder_names_and_private_extraction() {
    let direct = project(&Fixture::local(SOURCE));
    let renamed = project(&Fixture::local(
        &SOURCE
            .replace("machine Step>", "machine Selected>")
            .replace(
                "where machine Step satisfies",
                "where machine Selected satisfies",
            )
            .replace("{ Step(value) }", "{ Selected(value) }"),
    ));
    assert_eq!(direct, renamed, "nominal binder identity is positional");
    assert_eq!(
        direct.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );

    let extracted = format!(
        "{}\n\
         machine relay<machine Forward>(value: u64) -> u64\n\
         where machine Forward satisfies StepContract::step;\n\
         {{ Forward(value) }}",
        SOURCE.replace("{ Step(value) }", "{ relay<Step>(value) }")
    );
    let extracted = project(&Fixture::local(&extracted));
    assert_eq!(
        direct, extracted,
        "a private generic helper preserves the same dependency"
    );
    assert_eq!(
        direct.canonical_bytes().unwrap(),
        extracted.canonical_bytes().unwrap()
    );
}

#[test]
fn exported_nominal_reach_dependency_normalizes_repeated_concrete_contributions() {
    let once = project(&Fixture::local(
        &SOURCE.replace("{ Step(value) }", "{ note(); Step(value) }"),
    ));
    let repeated = project(&Fixture::local(
        &SOURCE.replace("{ Step(value) }", "{ note(); note(); Step(value) }"),
    ));
    assert_eq!(once, repeated, "concrete service union remains idempotent");
    assert_eq!(
        once.canonical_bytes().unwrap(),
        repeated.canonical_bytes().unwrap()
    );
}

#[test]
fn dependency_parameters_use_complete_static_telescope_ordinals() {
    let source = SOURCE.replace("<machine Step>", "<Element, machine Step>");
    let policy = project(&Fixture::local(&source));
    assert_eq!(
        callable(&policy, "traverse")
            .service_reach_dependency()
            .parameters(),
        &[1]
    );
}

#[test]
fn stale_checked_dependency_rejects_even_with_unchanged_conservative_rows() {
    for additive in [false, true] {
        let source = if additive {
            SOURCE.replace("{ Step(value) }", "{ note(); Step(value) }")
        } else {
            SOURCE.to_owned()
        };
        let fixture = Fixture::local(&source);
        project(&fixture);
        let mut changed = fixture.checked.clone();
        let machine = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "traverse")
            .unwrap()
            .symbol;
        let root_machines = changed.facts.service_reaches.root_machines;
        let summary = changed
            .facts
            .service_reaches
            .machines
            .span_mut_or_empty(root_machines)
            .iter_mut()
            .find(|summary| summary.machine == machine)
            .unwrap();
        if additive {
            summary.dependency.concrete = language_semantics::ServiceReachRowTable::EMPTY_ROW;
        } else {
            summary.dependency.parameters = Default::default();
        }
        assert!(
            project_checked_callable_policy(&changed, fixture.target, package_identity()).is_err(),
            "flat conservative reach cannot authorize a stale normalized dependency"
        );
    }
}

#[test]
fn changing_a_forwarded_nominal_argument_rejects_stale_checked_dependency() {
    let fixture = Fixture::local(
        r#"
pub boundary trait Console {}
pub trait StepContract { machine step(value: u64) -> u64 reaches Console; }
machine relay<machine Forward>(value: u64) -> u64
where machine Forward satisfies StepContract::step;
{ Forward(value) }
pub machine traverse<machine First, machine Second>(value: u64) -> u64
where machine First satisfies StepContract::step;
where machine Second satisfies StepContract::step;
{ relay<First>(value) }
"#,
    );
    let policy = project(&fixture);
    assert_eq!(
        callable(&policy, "traverse")
            .service_reach_dependency()
            .parameters(),
        &[0]
    );
    let mut changed = fixture.checked.clone();
    let caller = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "traverse")
        .unwrap();
    let caller_symbol = caller.symbol;
    let parameters = changed.machine_type_parameters(caller);
    let first = parameters[0].symbol;
    let second = parameters[1].symbol;
    let relay = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .unwrap();
    let relay_entry = changed.machine_states(relay)[0].symbol;
    let calls: Vec<_> = changed
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            (call.target_symbol == relay_entry
                && call.machine_arguments.len() == 1
                && call.machine_arguments[0].symbol == first)
                .then_some(handle)
        })
        .collect();
    let [call] = calls.as_slice() else {
        panic!("one exact relay application")
    };
    let typed_trees::expression::ExpressionNode::Call(call) =
        changed.typed.expression_table.expression_mut(*call)
    else {
        panic!("selected relay call")
    };
    // Keep spelling and checked facts unchanged: semantic selection is the
    // exact symbol, not the retained diagnostic path or its requirement shape.
    call.machine_arguments[0].symbol = second;
    assert_eq!(
        changed.facts.service_reaches,
        fixture.checked.facts.service_reaches
    );
    let original = validation::infer_service_reaches(
        &fixture.checked,
        &validation::infer_operational_may(&fixture.checked),
    );
    let replay =
        validation::infer_service_reaches(&changed, &validation::infer_operational_may(&changed));
    assert_eq!(
        original
            .rows
            .services(original.for_machine(caller_symbol).unwrap().published),
        replay
            .rows
            .services(replay.for_machine(caller_symbol).unwrap().published),
        "both nominal arguments have the same conservative Console bound"
    );
    let diagnostics = project_checked_callable_policy(&changed, fixture.target, package_identity())
        .expect_err("changed argument cannot reuse the first binder's checked dependency");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("checked service-reach dependency differs from independent inference")),
        "{diagnostics:?}"
    );
}
