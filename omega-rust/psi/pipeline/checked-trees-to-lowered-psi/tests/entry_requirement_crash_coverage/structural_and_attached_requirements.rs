use super::{
    assert_structural_entry_requirement_artifact, assert_unconditional_call_trap_at_entry,
    assert_unconditional_call_trap_with_structural_arguments, typed,
};
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn shared_record_entry_requirement_covers_unconditional_scalar_call() {
    let source = r#"
        data Flag { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &Flag)
        reaches Sink requires record.enabled
        crashes Trap record.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
    "#;
    assert_structural_entry_requirement_artifact(source);
}

#[test]
fn shared_self_entry_requirement_covers_unconditional_scalar_call() {
    assert_structural_entry_requirement_artifact(
        r#"
        data Helper { enabled: bool; }
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(&self)
        reaches Sink requires self.enabled
        crashes Trap self.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Helper)
        requires record.enabled
        crashes Trap record.enabled
        { record.forward(); }
        "#,
    );
}

#[test]
fn explicit_field_identities_preserve_structural_entry_hypotheses() {
    for identity in [0, 7] {
        let source = format!(
            "data Flag {{ #{identity} enabled: bool; #11 other: bool; }}\n\
             data Helper {{}}\ndata Main {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(record: &Flag) reaches Sink\nrequires record.enabled\ncrashes Trap record.enabled\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value(record: &Flag)\nrequires record.enabled\ncrashes Trap record.enabled\n\
             {{ Helper::forward(record); }}",
        );
        assert_structural_entry_requirement_artifact(&source);
    }
}

#[test]
fn declared_structural_entry_requirement_survives_a_body_field_write() {
    assert_structural_entry_requirement_artifact(
        r#"
        data Flag { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &mut Flag)
        reaches Sink requires record.enabled
        crashes Trap record.enabled
        { record.enabled = false; Sink::record(trigger()); }
        machine Main::value(record: &mut Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
        "#,
    );
}

#[test]
fn structural_requires_cannot_recover_corrupted_field_identity_from_spelling() {
    let source = r#"
        data Flag { enabled: bool; other: bool; }
        data Shadow { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &Flag)
        reaches Sink requires record.enabled
        crashes Trap record.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
    "#;
    for corruption in ["foreign symbol", "missing symbol", "wrong spelling"] {
        let mut program = typed(source);
        let owner = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Shadow")
            .unwrap();
        let foreign = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "enabled" => {
                    Some(field.symbol)
                }
                _ => None,
            })
            .unwrap();
        let owner = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Flag")
            .unwrap();
        let wrong_name = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "other" => {
                    Some(field.name.clone())
                }
                _ => None,
            })
            .unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Helper::forward")
            .unwrap();
        let requirement = program
            .machine_contracts(machine)
            .iter()
            .find(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .unwrap();
        let expression = program
            .proof_facts
            .span_or_empty(requirement.facts)
            .iter()
            .find_map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .unwrap();
        let mut pending = vec![expression];
        let mut selected = None;
        while let Some(expression) = pending.pop() {
            match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Member(_) => {
                    selected = Some(expression);
                    break;
                }
                typed_trees::expression::ExpressionNode::Binary(binary) => {
                    pending.extend([binary.right, binary.left])
                }
                typed_trees::expression::ExpressionNode::Unary(unary) => {
                    pending.push(unary.operand)
                }
                _ => {}
            }
        }
        let typed_trees::expression::ExpressionNode::Member(member) = program
            .expression_table
            .expression_mut(selected.expect("retained Requires member"))
        else {
            unreachable!();
        };
        assert!(member.member_symbol.is_valid());
        assert!(foreign.is_valid());
        assert_ne!(member.member_symbol, foreign);
        match corruption {
            "foreign symbol" => member.member_symbol = foreign,
            "missing symbol" => member.member_symbol = Default::default(),
            "wrong spelling" => member.member = wrong_name,
            _ => unreachable!(),
        }
        let diagnostics = match lower_typed_trees(program, &CheckingRequest::settled()) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("corrupted Requires identity was accepted: {corruption}"),
        };
        assert!(!diagnostics.is_empty(), "{corruption}");
    }
}

#[test]
fn structural_entry_formulas_preserve_owned_shared_and_mutable_borrow_snapshots() {
    for ownership in ["", "&", "&mut "] {
        for (record_type, predicate) in [
            ("Flag", "record.enabled"),
            ("Flag", "!record.enabled"),
            ("Flag", "record.enabled && record.other"),
            ("Flag", "record.enabled == record.other"),
            ("Envelope", "record.inner.enabled"),
            ("Envelope", "record.inner.enabled == record.inner.other"),
        ] {
            let source = format!(
                "data Flag {{ enabled: bool; other: bool; }}\n\
                 data Envelope {{ inner: Flag; }}\ndata Helper {{}}\ndata Main {{}}\n\
                 boundary trait Sink {{ machine record(value: bool); }}\n\
                 machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine Helper::forward(record: {ownership}{record_type})\n\
                 reaches Sink requires {predicate}\ncrashes Trap {predicate}\n{{ Sink::record(trigger()); }}\n\
                 machine Main::value(record: {ownership}{record_type})\n\
                 requires {predicate}\ncrashes Trap {predicate}\n{{ Helper::forward(record); }}",
            );
            assert_structural_entry_requirement_artifact(&source);
        }
    }
}

#[test]
fn structural_entry_requirement_does_not_authorize_a_different_field_or_root() {
    for route in ["record.other", "other.enabled", "other.other"] {
        let source = format!(
            "data Flag {{ enabled: bool; other: bool; }}\ndata Helper {{}}\ndata Main {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(record: &Flag, other: &Flag)\n\
             reaches Sink requires record.enabled\ncrashes Trap {route}\n{{ Sink::record(trigger()); }}\n\
             machine Main::value(record: &Flag, other: &Flag)\n\
             requires record.enabled\ncrashes Trap\n{{ Helper::forward(record, other); }}",
        );
        let diagnostics = match lower_typed_trees(typed(&source), &CheckingRequest::settled()) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("a different structural entry identity must reject: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("Helper::forward")
                    && diagnostic.message.contains("uncovered Trap crash route")
            }),
            "the exact field/root crash coverage check must reject: {source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn attached_boolean_entry_requirement_covers_unconditional_scalar_call() {
    assert_unconditional_call_trap_at_entry(
        r#"
        data Main {}
        data Helper {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(flag: bool)
        reaches Sink requires flag
        crashes Trap flag
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { Helper::forward(true); }
        "#,
        "Main::value",
    );
}

#[test]
fn attached_boolean_entry_formulas_keep_original_scalar_parameters() {
    for (predicate, arguments) in [
        ("left && right", "true, true"),
        ("!right", "true, false"),
        ("left == right", "false, false"),
        ("left == right", "true, true"),
        ("!(left && right)", "false, true"),
    ] {
        let source = format!(
            "data Main {{}}\ndata Helper {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(left: bool, right: bool) reaches Sink\nrequires {predicate}\ncrashes Trap {predicate}\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value()\ncrashes Trap\n{{ Helper::forward({arguments}); }}",
        );
        assert_unconditional_call_trap_at_entry(&source, "Main::value");
    }
}

#[test]
fn attached_entry_requirements_cannot_substitute_another_boolean_formal() {
    for (requirement, route, arguments) in [
        ("left", "right", "true, false"),
        ("!left", "!right", "false, true"),
    ] {
        let source = format!(
            "data Main {{}}\ndata Helper {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(left: bool, right: bool) reaches Sink\nrequires {requirement}\ncrashes Trap {route}\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value()\ncrashes Trap\n{{ Helper::forward({arguments}); }}",
        );
        let diagnostics = match lower_typed_trees(typed(&source), &CheckingRequest::settled()) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("distinct attached formals cannot authorize each other: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("Helper::forward")
                    && diagnostic.message.contains("uncovered Trap crash route")
            }),
            "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn mixed_attached_entry_requirement_uses_the_original_nonfirst_boolean() {
    let source = r#"
        data Main {}
        data Helper {}
        data Box { value: u32; }
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(left: bool, record: Box, right: bool)
        reaches Sink requires right
        crashes Trap right
        { Sink::record(trigger()); }
        machine Main::value(record: Box)
        crashes Trap
        { Helper::forward(false, record, true); }
    "#;
    let checked = lower_typed_trees(typed(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let missing = ["Helper::forward", "Main::value"]
        .into_iter()
        .filter(|name| {
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == *name)
                .expect("exact authored mixed fixture machine");
            !checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .any(|plan| plan.machine == machine.symbol)
        })
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "mixed fixture lacks checked Unit plans for {missing:?}"
    );
    assert_unconditional_call_trap_with_structural_arguments(source, "Main::value", true);
}
