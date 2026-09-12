use super::*;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedStructuralValueKind,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
};

#[test]
fn fresh_match_value_keeps_ordered_case_roots_and_ordinary_owned_state_transfer() {
    let checked = checked_source(
        r#"
data Tag { case Empty; case First; case Second; }
machine observe(selector: u64) {
    let selected: Tag = match selector {
        0 -> Tag::Empty,
        _ -> match selector { 1 -> Tag::First, _ -> Tag::Second }
    };
    let continued: u64 = selector & 255;
    transition { _ -> inspect(selected, continued) }
    state inspect(selected: Tag, continued: u64) {
        transition selected {
            Tag::Empty -> done(continued)
            Tag::First -> done(continued)
            Tag::Second -> done(continued)
        }
    }
    state done(continued: u64) {}
}
"#,
        false,
    );
    let values = &checked.facts.values.structural_values;
    let root = values
        .roots
        .iter()
        .map(|(_, root)| root)
        .next()
        .expect("structural value root");
    let CheckedStructuralValueKind::Dispatch { arms, .. } = values.nodes.get(root.root).kind else {
        panic!("ordered dispatch root");
    };
    let arms = values.dispatch_arms.span(arms).expect("live ordered arms");
    assert_eq!(arms.len(), 2);
    assert!(matches!(
        values.nodes.get(arms[0].value).kind,
        CheckedStructuralValueKind::Case(_)
    ));
    assert!(matches!(
        values.nodes.get(arms[1].value).kind,
        CheckedStructuralValueKind::Dispatch { .. }
    ));
    let machine = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|machine| machine.machine == root.machine)
        .expect("general state graph");
    let state = machine
        .states
        .iter()
        .find(|state| state.state == root.state)
        .unwrap();
    let permissions = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == root.machine && event.state_symbol == root.state
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    assert_eq!(
        permissions
            .iter()
            .filter(
                |event| event.kind == language_semantics::PermissionEventKind::Establish
                    && matches!(
                        event.provenance,
                        language_semantics::PermissionProvenance::Established { .. }
                    )
            )
            .count(),
        1
    );
    assert_eq!(
        permissions
            .iter()
            .filter(|event| event.kind == language_semantics::PermissionEventKind::Transfer)
            .count(),
        1
    );
    let [
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            value,
            calls,
            discard_result_on_return: false,
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
    ] = state.operations.as_slice()
    else {
        panic!(
            "structural establishment followed by scalar establishment: {:?}",
            state.operations
        );
    };
    assert!(calls.is_empty());
    assert_eq!(*value, root.root);
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } = &state.terminator else {
        panic!("ordinary state successor");
    };
    assert!(successor.transfers.iter().any(|transfer| matches!(transfer.source,
        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
            if binding_ordinal == result.binding_ordinal)));
}

#[test]
fn fresh_match_tail_returns_the_same_materialized_result_namespace() {
    let checked = checked_source(
        "data Tag { case First; case Second; } machine choose(selector: bool) -> Tag { match selector { true -> Tag::First, false -> Tag::Second } }",
        false,
    );
    let machine = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .first()
        .expect("structural result graph");
    let state = &machine.states[0];
    let [
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return: false,
            ..
        },
    ] = state.operations.as_slice()
    else {
        panic!("fresh result materialization");
    };
    let CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } =
        &state.terminator
    else {
        panic!("generic structural completion");
    };
    assert_eq!(
        returned.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal
        }
    );
}

#[test]
fn fresh_match_local_returns_its_owner_after_an_ordinary_scalar_computation() {
    let checked = checked_source(
        "data Tag { case First; case Second; } machine choose(selector: u64) -> Tag { let selected: Tag = match selector { 0 -> Tag::First, _ -> Tag::Second }; let continued: u64 = selector & 255; selected }",
        false,
    );
    let machine = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .first()
        .expect("local structural return graph");
    let state = &machine.states[0];
    let [
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return: false,
            ..
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. },
    ] = state.operations.as_slice()
    else {
        panic!(
            "local owner and subsequent scalar computation: {:?}",
            state.operations
        );
    };
    let CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } =
        &state.terminator
    else {
        panic!("generic local structural return");
    };
    assert_eq!(
        returned.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        }
    );
    let permissions = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine.machine && event.state_symbol == state.state
        })
        .collect::<Vec<_>>();
    assert!(permissions.iter().any(|event| event.kind
        == language_semantics::PermissionEventKind::Establish
        && matches!(
            event.provenance,
            language_semantics::PermissionProvenance::Established { .. }
        )));
    assert!(
        permissions
            .iter()
            .any(|event| event.kind == language_semantics::PermissionEventKind::Transfer)
    );
}

#[test]
fn scalar_prefix_and_call_preserve_later_structural_and_scalar_binding_identity() {
    let checked = checked_source(
        "data Tag { case First; case Second; } machine observe(value: u64) {} machine choose(selector: u64) -> Tag { let before: u64 = selector & 127; observe(before); let selected: Tag = match before { 0 -> Tag::First, _ -> Tag::Second }; let continued: u64 = before & 63; selected }",
        false,
    );
    let root = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .map(|(_, root)| root)
        .next()
        .expect("structural local root");
    let machine = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|machine| machine.machine == root.machine)
        .expect("prefix and call graph");
    let state = &machine.states[0];
    let [
        CheckedUnitEffectOperationPlan::CallUnit { .. },
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return: false,
            ..
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result: scalar, .. },
    ] = state.operations.as_slice()
    else {
        panic!(
            "ordered call, structural local, scalar local: {:?}",
            state.operations
        );
    };
    assert_eq!(scalar.binding_ordinal, 1);
    assert_eq!(scalar.statement_index, 3);
    let CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } =
        &state.terminator
    else {
        panic!("existing structural local returned");
    };
    assert_eq!(
        returned.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        }
    );
}

#[test]
fn fresh_match_subject_uses_its_exact_member_and_arithmetic_carrier() {
    for subject in ["self.err_code", "self.err_code & 255"] {
        let source = format!(
            "data Tag {{ case First; case Other; }} data Status {{ err_code: i32; }} machine Status::kind(&mut self) -> Tag {{ match {subject} {{ 2 -> Tag::First, _ -> Tag::Other }} }}"
        );
        let checked = checked_source(&source, false);
        let values = &checked.facts.values.structural_values;
        let root = values
            .roots
            .iter()
            .map(|(_, root)| root)
            .next()
            .expect("member Match result root");
        let CheckedStructuralValueKind::Dispatch { subject, .. } = values.nodes.get(root.root).kind
        else {
            panic!("member subject remains ordered dispatch");
        };
        let computation = checked.facts.values.scalar_computations.nodes.get(subject);
        assert_eq!(computation.primitive_type, PrimitiveType::I32);
        assert!(computation.authored_root.is_valid());
    }
}

#[test]
fn match_fresh_value_admission_keeps_owned_input_linear_reference_and_hook_rejections() {
    for source in [
        "data Tag { case First; case Second; } machine choose(selector: bool, left: Tag, right: Tag) -> Tag { match selector { true -> left, false -> right } }",
        "data Tag [linear] { case First; case Second; } machine choose(selector: bool) -> Tag { match selector { true -> Tag::First, false -> Tag::Second } }",
        "data Tag { case First; case Second; } machine Tag::drop(&mut self) {} machine choose(selector: bool) -> Tag { match selector { true -> Tag::First, false -> Tag::Second } }",
        "machine choose(selector: bool, left: &u64, right: &u64) -> &u64 { match selector { true -> left, false -> right } }",
    ] {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let diagnostics =
            crate::lower_typed_trees(typed).expect_err("unsupported branch custody must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.to_string().contains("branch custody join")),
            "{source}: {diagnostics:?}"
        );
    }
}
