use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolved");
    lower_symbol_resolved_trees(&resolved).expect("typed")
}

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_projection/main.omg"
));

#[test]
fn measure_projection_requires_its_exact_parameter_receiver() {
    let program = typed(COUNTDOWN);
    lower_typed_trees(program).expect("valid direct field measure");
    for receiver in ["nonexistent", "self", "Other::countdown"] {
        let source = COUNTDOWN.replace(
            "{ countdown.remaining }",
            &format!("{{ {receiver}.remaining }}"),
        );
        let program = typed(&source);
        assert_eq!(
            crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
            Some(language_semantics::TerminationGuarantee::NoGuarantee),
            "{receiver} cannot project the measured parameter"
        );
        crate::checks::termination::check_machine_termination(&program)
            .expect_err("invalid measure cannot supply a termination proof");
        assert!(lower_typed_trees(program).is_err());
    }
}

#[test]
fn measure_projection_preserves_constraints_and_rejects_wrong_rank_carriers() {
    let constrained = typed(&COUNTDOWN.replace("remaining: u64;", "remaining: u64 [0..=5];"));
    // This checks the rank carrier, not construction-range proof support.
    crate::checks::termination::check_machine_termination(&constrained)
        .expect("field constraints preserve the u64 rank carrier");
    for carrier in ["u32", "i64", "f64"] {
        let source = COUNTDOWN.replace("remaining: u64;", &format!("remaining: {carrier};"));
        let program = typed(&source);
        assert_eq!(
            crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
            Some(language_semantics::TerminationGuarantee::NoGuarantee),
            "the return annotation cannot convert a {carrier} field to u64"
        );
    }
}

#[test]
fn measure_projection_does_not_drop_nested_receivers_or_stalled_edges() {
    let source = COUNTDOWN
        .replace(
            "data Countdown { remaining: u64; }",
            "data Inner { remaining: u64; } data Countdown { remaining: u64; inner: Inner; }",
        )
        .replace("{ countdown.remaining }", "{ countdown.inner.remaining }")
        .replace(
            "Countdown { remaining: countdown.remaining - 1 }",
            "Countdown { remaining: countdown.remaining - 1, inner: countdown.inner }",
        );
    let program = typed(&source);
    assert_eq!(
        crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
        Some(language_semantics::TerminationGuarantee::NoGuarantee),
        "decreasing the outer field does not decrease the nested projection"
    );
    for next in ["walk(countdown)", "self"] {
        let source = COUNTDOWN.replace("false -> countdown.remaining", &format!("false -> {next}"));
        crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err("one descending arm cannot excuse another cyclic arm");
    }
}

#[test]
fn measure_projection_rejects_substituted_field_and_binder_identities() {
    let source = format!("data Other {{ remaining: u64; }}\n{COUNTDOWN}");
    let program = typed(&source);
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap();
    let typed_trees::data::DataMember::Field(foreign_field) = &program.data_members(other)[0]
    else {
        panic!("field");
    };
    let machine = &program.machines()[0];
    let foreign_parameter = program.state_parameters(&program.machine_states(machine)[0])[0].symbol;
    let measure = &program.measures()[0];
    let body = program.expression_table.expression_handles(measure.body)[0];
    for replacement in [
        symbols::SymbolHandle::invalid(),
        foreign_field.symbol,
        measure.symbol,
    ] {
        let mut changed = program.clone();
        let typed_trees::expression::ExpressionNode::Member(member) =
            changed.expression_table.expression_mut(body)
        else {
            panic!("projection");
        };
        member.member_symbol = replacement;
        assert_eq!(
            crate::infer_machine_termination_summary(&changed, machine.symbol),
            Some(language_semantics::TerminationGuarantee::NoGuarantee)
        );
    }
    let mut changed = program.clone();
    let typed_trees::expression::ExpressionNode::Member(member) =
        changed.expression_table.expression(body)
    else {
        panic!("projection");
    };
    let receiver = member.receiver;
    let typed_trees::expression::ExpressionNode::Name(path) =
        changed.expression_table.expression_mut(receiver)
    else {
        panic!("binder");
    };
    path.symbol = foreign_parameter;
    path.head_symbol = foreign_parameter;
    assert_eq!(
        crate::infer_machine_termination_summary(&changed, machine.symbol),
        Some(language_semantics::TerminationGuarantee::NoGuarantee)
    );
}

#[test]
fn measure_projection_matches_nominal_subject_identity_not_spelling() {
    use typed_trees::types::TypeReferenceNode;

    let program = typed(&format!("data Other {{ remaining: u64; }}\n{COUNTDOWN}"));
    let machine = &program.machines()[0];
    let subject = &program.state_parameters(&program.machine_states(machine)[0])[0];
    let subject_type = program
        .type_reference_table
        .type_reference(subject.type_reference)
        .clone();
    let TypeReferenceNode::Named { name, .. } = &subject_type else {
        panic!("nominal subject");
    };
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap();
    let mut changed = program.clone();
    changed.type_reference_table.substitute_node(
        subject.type_reference,
        TypeReferenceNode::Named {
            symbol: other.symbol,
            name: name.clone(),
        },
    );
    assert_eq!(
        crate::infer_machine_termination_summary(&changed, machine.symbol),
        Some(language_semantics::TerminationGuarantee::NoGuarantee)
    );

    let mut constrained = program.clone();
    let base_type = constrained.type_reference_table.insert(subject_type);
    constrained.type_reference_table.substitute_node(
        subject.type_reference,
        TypeReferenceNode::Constrained {
            base_type,
            constraints: Default::default(),
        },
    );
    assert!(matches!(
        crate::infer_machine_termination_summary(&constrained, machine.symbol),
        Some(language_semantics::TerminationGuarantee::Terminates { .. })
    ));
}
