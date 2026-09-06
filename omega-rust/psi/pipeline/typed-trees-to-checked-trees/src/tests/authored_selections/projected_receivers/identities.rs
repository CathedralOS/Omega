use super::*;

fn input_symbol(program: &TypedTrees, machine_name: &str) -> SymbolHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("fixture machine");
    program.state_parameters(&program.machine_states(machine)[0])[0].symbol
}

fn foreign_context_field(program: &TypedTrees) -> SymbolHandle {
    let owner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "ForeignCarrier")
        .expect("foreign carrier");
    program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "context" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("foreign same-named field")
}

fn replace_expression_input_roots(program: &mut TypedTrees, replacement: SymbolHandle) {
    let expected = input_symbol(program, "inspect");
    let roots = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let ExpressionNode::Name(path) = node else {
                return None;
            };
            let members = program.expression_table.name_path_members(path.members);
            (members
                .first()
                .is_some_and(|member| member.as_str() == "carrier"))
            .then_some((expression, *path, members.len()))
        })
        .collect::<Vec<_>>();
    assert!(!roots.is_empty(), "retained expression receiver roots");
    for (expression, path, length) in roots {
        assert_eq!(
            path.head_symbol, expected,
            "exact current-state input before tampering"
        );
        // Keep every identity slot coherent, including the one-member Name
        // root of a Member expression and flattened NamePath representations.
        if !path.member_symbols.is_empty() {
            program
                .expression_table
                .set_name_path_member_symbol_at_offset(path.member_symbols, 0, replacement);
        }
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        path.head_symbol = replacement;
        if length == 1 {
            path.symbol = replacement;
        }
    }
}

fn replace_expression_endpoints(program: &mut TypedTrees, replacement: SymbolHandle) {
    let endpoints = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            ExpressionNode::Member(member) if member.member.as_str() == "context" => {
                Some(expression)
            }
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                (members.len() == 2
                    && members[0].as_str() == "carrier"
                    && members[1].as_str() == "context")
                    .then_some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !endpoints.is_empty(),
        "retained projected expression endpoints"
    );
    for expression in endpoints {
        let member_symbols = match program.expression_table.expression_mut(expression) {
            ExpressionNode::Member(member) => {
                member.member_symbol = replacement;
                continue;
            }
            ExpressionNode::Name(path) => {
                path.symbol = replacement;
                path.member_symbols
            }
            _ => unreachable!(),
        };
        if program
            .expression_table
            .name_path_member_symbols(member_symbols)
            .len()
            == 2
        {
            program
                .expression_table
                .set_name_path_member_symbol_at_offset(member_symbols, 1, replacement);
        }
    }
}

fn pending_statement_call_fixture() -> CheckedTrees {
    let typed = typed_fixture(CallForm::Statement);
    let mut authored = typed.authored_declaration_selections().clone();
    let mut checked = lower_typed_trees(typed).expect("untampered projected call checks");
    assert_exact_selection(&checked);
    let pending = authored
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].target(),
        AuthoredDeclarationSelectionTarget::LateBound(
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        ),
        "fixture must exercise late-bound call finalization"
    );
    // Retain checked non-call rows so an orphaned receiver expression cannot
    // make a negative case fail on member custody before testing the call.
    let other_pending = authored
        .iter()
        .filter_map(|selection| match selection.target() {
            AuthoredDeclarationSelectionTarget::LateBound(binding)
                if selection.kind() != AuthoredDeclarationSelectionKind::Call =>
            {
                Some((selection.occurrence_id(), binding))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (occurrence, binding) in other_pending {
        match checked
            .authored_declaration_selections()
            .get(occurrence)
            .unwrap()
            .target()
        {
            AuthoredDeclarationSelectionTarget::Resolved(target) => authored
                .finalize_late_bound(occurrence, binding, target.selected_symbol())
                .expect("retain checked non-call declaration"),
            AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => authored
                .finalize_intrinsic(occurrence, binding, intrinsic)
                .expect("retain checked non-call intrinsic"),
            AuthoredDeclarationSelectionTarget::LateBound(_) => {
                panic!("checked ledger is complete")
            }
        }
    }
    checked
        .typed
        .retain_authored_declaration_selections(authored);
    crate::authored_selections::finalize_checked_authored_selections(
        &mut checked.typed.clone(),
        &checked.facts,
    )
    .expect("untampered facts rejoin the pending authored call");
    checked
}

#[test]
fn foreign_projected_callees_cannot_finalize_the_authored_call() {
    for form in CALL_FORMS {
        let mut typed = typed_fixture(form);
        let foreign = method_symbol(&typed, "Decoy");
        assert_ne!(foreign, method_symbol(&typed, "Context"));
        replace_typed_callees(&mut typed, foreign, false);
        let diagnostics = lower_typed_trees(typed)
            .expect_err("a foreign nominal method cannot supply authored call custody");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("receiver") && diagnostic.message.contains("target")
            }),
            "{form:?}: {diagnostics:#?}"
        );
    }
}

#[test]
fn a_foreign_projected_endpoint_cannot_override_the_exact_input_root() {
    let mut typed = typed_fixture(CallForm::Statement);
    let endpoint = foreign_context_field(&typed);
    let call = statement_call_mut(&mut typed);
    call.receiver_symbol = endpoint;
    call.target_symbol = SymbolHandle::invalid();
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the endpoint must belong to the exact receiver projection");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("receiver") && diagnostic.message.contains("field")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_missing_projected_input_root_cannot_bind_by_spelling() {
    let mut typed = typed_fixture(CallForm::Statement);
    let call = statement_call_mut(&mut typed);
    call.receiver_root_symbol = SymbolHandle::invalid();
    call.receiver_symbol = SymbolHandle::invalid();
    call.target_symbol = SymbolHandle::invalid();
    let diagnostics = lower_typed_trees(typed)
        .expect_err("carrier.context spelling cannot replace an exact input root");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("not a declared receiver in this state")
                || (diagnostic
                    .message
                    .contains("authored Call declaration selection")
                    && diagnostic.message.contains("remained unresolved"))
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_foreign_expression_endpoint_cannot_override_the_exact_input_root() {
    let mut typed = typed_fixture(CallForm::ValueOperand);
    let endpoint = foreign_context_field(&typed);
    replace_expression_endpoints(&mut typed, endpoint);
    replace_typed_callees(&mut typed, SymbolHandle::invalid(), false);
    let diagnostics = lower_typed_trees(typed)
        .expect_err("an expression endpoint cannot be trusted or silently replaced");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("receiver") && diagnostic.message.contains("field")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn missing_or_foreign_expression_roots_cannot_bind_by_spelling() {
    for foreign in [false, true] {
        let mut typed = typed_fixture(CallForm::ValueOperand);
        let replacement = if foreign {
            input_symbol(&typed, "unrelated")
        } else {
            SymbolHandle::invalid()
        };
        assert_ne!(replacement, input_symbol(&typed, "inspect"));
        replace_expression_input_roots(&mut typed, replacement);
        replace_typed_callees(&mut typed, SymbolHandle::invalid(), false);
        let diagnostics = lower_typed_trees(typed)
            .expect_err("same-spelled input storage from another state cannot authorize a call");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("receiver")
                    || diagnostic
                        .message
                        .contains("not a declared local, parameter, field, or type in this state")
                    || (diagnostic
                        .message
                        .contains("authored Call declaration selection")
                        && diagnostic.message.contains("remained unresolved"))
            }),
            "foreign={foreign}: {diagnostics:#?}"
        );
    }
}

#[test]
fn an_erased_projected_statement_callee_without_receiver_or_checked_evidence_rejects() {
    let mut checked = pending_statement_call_fixture();
    replace_typed_callees(&mut checked.typed, SymbolHandle::invalid(), true);
    let diagnostic = crate::authored_selections::finalize_checked_authored_selections(
        &mut checked.typed,
        &checked_trees::CheckFacts::default(),
    )
    .expect_err("method spelling alone cannot finalize a projected call");
    assert!(
        diagnostic
            .message
            .contains("authored Call declaration selection")
            && diagnostic.message.contains("remained unresolved"),
        "{diagnostic:?}"
    );
    assert!(!checked.authored_declaration_selections().all_finalized());
}

#[test]
fn attached_lookup_preserves_exact_owners_when_nominal_spellings_collide() {
    let mut program = typed_fixture(CallForm::Statement);
    let expected = method_symbol(&program, "Context");
    let foreign = method_symbol(&program, "Decoy");
    let endpoint = field_symbol(&program, "Carrier", "context");
    assert_ne!(expected, foreign);
    let resolve = |program: &TypedTrees, target| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .expect("caller machine");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("projected statement call")
        };
        crate::lookup::resolve_state_call_target(
            program,
            machine,
            state,
            endpoint,
            target,
            crate::lookup::statement_call_receiver_members(program, call),
            &call.target,
        )
    };
    assert_eq!(
        resolve(&program, expected),
        expected,
        "untampered lookup control"
    );
    assert_eq!(
        resolve(&program, SymbolHandle::invalid()),
        expected,
        "an exact receiver legitimately supplies a missing target"
    );

    let context = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Context")
        .expect("correct nominal owner");
    let context_symbol = context.symbol;
    let spelling = context.name.clone();
    let foreign_owner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Decoy")
        .expect("foreign nominal owner")
        .symbol;
    assert_ne!(context_symbol, foreign_owner);
    // Authored names are globally unique in this small source fixture. Model
    // colliding retained spellings at the lookup boundary while preserving
    // both nominal symbols and their independently owned method states.
    for definition in program
        .tables
        .data_definitions
        .span_mut_or_empty(program.roots.data_definitions)
    {
        if definition.symbol == foreign_owner {
            definition.name = spelling.clone();
        }
    }
    for machine in program.machines_mut() {
        if machine.attached_data_symbol == foreign_owner {
            machine.attached_data = Some(spelling.clone());
        }
    }
    let owners = program
        .machines()
        .iter()
        .filter(|machine| machine.attached_data.as_ref() == Some(&spelling))
        .map(|machine| machine.attached_data_symbol)
        .collect::<Vec<_>>();
    assert_eq!(
        owners,
        [foreign_owner, context_symbol],
        "foreign candidate precedes the correct owner"
    );
    for (target, selected) in [
        (SymbolHandle::invalid(), expected),
        (expected, expected),
        (foreign, SymbolHandle::invalid()),
    ] {
        assert_eq!(
            resolve(&program, target),
            selected,
            "same-spelled foreign owners cannot settle or replace target {target:?}"
        );
    }
}
