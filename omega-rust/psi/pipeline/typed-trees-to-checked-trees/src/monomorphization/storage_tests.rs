use super::{CallSite, ExpressionNode, StatementNode, SymbolHandle, TypedTrees};
use crate::monomorphization::selection::contract_expression_handles;
use crate::monomorphization::{
    CallSelection, apply_call_specializations, candidate, candidate_for_selection,
    canonical_template_contract_bytes, cloned_runtime_call_subjects, collect_call_selections,
    collect_statement_expression_trees, monomorphize_generic_machine_value_calls_with_selections,
    runtime_value_subjects,
};

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

#[test]
fn named_type_arguments_cannot_reorder_the_authored_binder_tuple() {
    for argument in ["u8", "Marker"] {
        for body in [
            "bound<7, TYPE>()",
            "let result: u64 = bound<7, TYPE>(); result",
        ] {
            let mut program = typed(&format!(
                "data Marker {{}}
                 machine bound<T, const N: u64>() -> u64 {{ N }}
                 machine main() -> u64 {{ {} }}",
                body.replace("TYPE", argument),
            ));
            let errors = crate::specialize_static_machine_calls(&mut program)
                .expect_err("grouping proposals by kind cannot reorder their authored slots");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("declared binder kind")),
                "{errors:?}"
            );
        }
        let mut program = typed(&format!(
            "data Marker {{}}
             machine bound<T, const N: u64>() -> u64 {{ N }}
             machine main() -> u64 {{ bound<{argument}, 7>() }}"
        ));
        crate::specialize_static_machine_calls(&mut program)
            .expect("correctly ordered type/const tuple specializes");
    }
}

#[test]
fn unused_named_type_argument_cannot_erase_unsupplied_data_binders() {
    for declaration in [
        "data Marker<T> { value: T; }",
        "data Marker<const N: u64> { value: [u8; N]; }",
        "data Marker<'a> { value: &'a u8; }",
    ] {
        let mut program = typed(&format!(
            "{declaration}
             machine bound<T, const N: u64>() -> u64 {{ N }}
             machine main() -> u64 {{ bound<Marker, 7>() }}"
        ));
        let errors = crate::specialize_static_machine_calls(&mut program)
            .expect_err("every static type argument must be formed, including an unused binder");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("complete type/lifetime arguments")),
            "{errors:?}"
        );
    }
}

#[test]
fn detached_const_recipe_keeps_its_tuple_until_executable_probe_specialization() {
    let mut program = typed(
        "machine identity<T [copy]>(value: T) -> T { value }
         machine probe() -> u64 { identity<u64>(7) }",
    );
    let probe = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .expect("executable probe")
        .clone();
    let entry = program.machine_states(&probe)[0].clone();
    let mut expressions = Vec::new();
    for statement in program.statement_table.statements(entry.statement_nodes) {
        collect_statement_expression_trees(&program, statement, &mut expressions);
    }
    let initializer = expressions
        .into_iter()
        .find(|expression| {
            matches!(program.expression_table.expression(*expression),
                ExpressionNode::Call(call) if call.target.as_str() == "identity")
        })
        .expect("authored generic call");
    let ExpressionNode::Call(authored) = program.expression_table.expression(initializer).clone()
    else {
        panic!("authored call");
    };
    assert_eq!(authored.machine_arguments.len(), 1);
    let materialized = program
        .expression_table
        .expression_handles(authored.arguments)[0];
    let symbol = program.symbols.insert_generated_root_from(
        probe.symbol,
        symbols::SymbolKind::Const,
        "VALUE",
    );
    program.push_const_declaration(typed_trees::constant::ConstDeclaration {
        symbol,
        is_public: false,
        declared_type: entry.return_type,
        initializer_source_span: program.expression_table.source_span(initializer),
        canonical_value_encoding: Some(
            language_semantics::const_value::CanonicalConstIdentity::integer("u64", 7).encoding,
        ),
        authored_initializer: initializer,
        materialized_initializer: materialized,
    });
    // Retain the parsed call as declaration evidence, with no executable owner.
    // Activation below restores the very same parsed state in a private clone.
    let retained_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol != probe.symbol)
        .cloned()
        .collect::<Vec<_>>();
    program.roots.machines = arena::HandleSpan::empty();
    for machine in retained_machines {
        program.push_machine(machine);
    }
    crate::specialize_static_machine_calls(&mut program).expect("detached recipe preparation");
    assert_eq!(
        program.expression_table.expression(initializer),
        &ExpressionNode::Call(authored.clone())
    );
    assert!(program.machine_specializations.is_empty());

    let mut activated = program.clone();
    activated.push_machine(probe);
    crate::specialize_static_machine_calls(&mut activated).expect("executable probe preparation");
    let ExpressionNode::Call(selected) = activated.expression_table.expression(initializer) else {
        panic!("selected probe call");
    };
    assert!(selected.machine_arguments.is_empty());
    assert_ne!(selected.target_symbol, authored.target_symbol);
    let [specialization] = activated.machine_specializations.as_slice() else {
        panic!("one complete specialization");
    };
    let instance = activated
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.instance)
        .expect("receipt's executable instance");
    assert_eq!(
        selected.target_symbol,
        activated.machine_states(instance)[0].symbol
    );
    assert_eq!(
        program.expression_table.expression(initializer),
        &ExpressionNode::Call(authored)
    );
    assert!(program.machine_specializations.is_empty());
}

#[test]
fn explicit_static_argument_overflow_rejects_before_specialization() {
    for source in [
        "machine identity<T [copy]>(value: T) -> T { value }
         machine caller() -> u64 { identity<u64,u8>(7) }",
        "machine amount<const N: u64>() -> u64 { N }
         machine caller() -> u64 { amount<7,8>() }",
        "machine choose<T [copy],const N: u64>(value: T, bytes: [u8; N]) -> T { value }
         machine caller(value: u64, bytes: [u8; 2]) -> u64 {
             choose<u64,u8>(value, bytes)
         }",
        "machine identity<T [copy]>(value: T) -> T { value }
         machine caller() -> u64 { identity<u64,7>(7); 0 }",
    ] {
        let mut program = typed(source);
        let diagnostics = crate::specialize_static_machine_calls(&mut program)
            .expect_err("explicit surplus cannot disappear during speculative preparation");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("excess explicit static arguments")
                    || diagnostic
                        .message
                        .contains("no const parameter for extra argument")
                    || diagnostic.message.contains("declared binder kind")
            }),
            "{source}: {diagnostics:?}"
        );
        assert!(program.machine_specializations.is_empty());
    }
}

#[test]
fn explicit_static_argument_capacity_preserves_partial_inference() {
    for source in [
        "machine first<T [copy],U [copy]>(value: T, other: U) -> T { value }
         machine caller(value: u64, other: u8) -> u64 { first<u64>(value, other) }",
        "machine choose<T [copy],const N: u64>(value: T, bytes: [u8; N]) -> T { value }
         machine caller(value: u64, bytes: [u8; 2]) -> u64 {
             choose<u64>(value, bytes)
         }",
        "machine choose<T [copy],const N: u64>(value: T, bytes: [u8; N]) -> T { value }
         machine caller(value: u64, bytes: [u8; 2]) -> u64 {
             choose<2>(value, bytes)
         }",
    ] {
        let mut program = typed(source);
        crate::specialize_static_machine_calls(&mut program)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
        assert_eq!(program.machine_specializations.len(), 1, "{source}");
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .expect("caller");
        let instance = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == program.machine_specializations[0].instance)
            .expect("complete inferred instance");
        assert_eq!(
            call_targets(&program, caller),
            [program.machine_states(instance)[0].symbol]
        );
    }
}

#[test]
fn explicit_static_const_argument_retains_its_literal_carrier() {
    let mut wrong_carrier = typed(
        "machine amount<const N: u64>() -> u64 { N }
         machine read() -> u64 { amount<7u8>() }",
    );
    assert!(crate::specialize_static_machine_calls(&mut wrong_carrier).is_err());
    for literal in ["7u64", "7"] {
        let mut program = typed(&format!(
            "machine amount<const N: u64>() -> u64 {{ N }}
             machine read() -> u64 {{ amount<{literal}>() }}"
        ));
        crate::specialize_static_machine_calls(&mut program)
            .unwrap_or_else(|diagnostics| panic!("{literal}: {diagnostics:?}"));
        assert_eq!(program.machine_specializations.len(), 1);
    }
}

#[test]
fn recursive_instances_keep_each_tuples_own_state_and_template_commitment() {
    let mut program = typed(
        "machine repeat<T [copy]>(value: T) -> T { repeat(value) }
        machine first(value: u8) -> u8 { repeat(value) }
        machine second(value: u16) -> u16 { repeat(value) }
        machine third(value: u32) -> u32 { repeat(value) }",
    );
    let original = program.clone();
    let template_index = program
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "repeat")
        .expect("template");
    let template = candidate::from_machine(&program, template_index);
    let operational = validation::infer_operational_may(&program);
    let service_reaches = validation::infer_service_reaches(&program, &operational);
    let contract = canonical_template_contract_bytes(&program, template_index, &service_reaches)
        .expect("template contract");
    // Exercise graph publication independently of discovery's recursive-call
    // admission. These are the three exact external selections from the input.
    let selections = program
        .machines()
        .iter()
        .filter(|machine| matches!(machine.name.as_str(), "first" | "second" | "third"))
        .map(|machine| {
            let state = &program.machine_states(machine)[0];
            let expression = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    Some(local.initial_value)
                })
                .expect("hoisted external value call");
            let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
                panic!("external value call");
            };
            CallSelection {
                site: CallSite::Expression(expression),
                callee_symbol: call.target_symbol,
                candidate_index: 0,
                caller_is_generic: false,
                unresolved_machine_parameters: false,
                unresolved_evidence_parameters: false,
                unresolved_const_parameters: false,
                type_bindings: vec![Some(program.state_parameters(state)[0].type_reference)],
                const_bindings: Vec::new(),
                runtime_value_bindings: Vec::new(),
                machine_bindings: Vec::new(),
                evidence_bindings: Vec::new(),
                conflicted: false,
                explicit_argument_overflow: false,
            }
        })
        .collect::<Vec<_>>();
    apply_call_specializations(&mut program, &template, &selections, 0, &service_reaches)
        .expect("three recursive graph instances");
    assert_eq!(program.machine_specializations.len(), 3);
    assert!(
        !program
            .machine_type_parameters(&program.machines()[template_index])
            .is_empty()
    );
    for receipt in &program.machine_specializations {
        assert_ne!(receipt.instance, template.template.template_symbol);
        assert_eq!(receipt.canonical_template_contract_bytes, contract);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == receipt.instance)
            .expect("instance");
        assert!(!machine.is_public);
        assert!(program.machine_type_parameters(machine).is_empty());
        let state = &program.machine_states(machine)[0];
        let calls = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                let ExpressionNode::Call(call) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return None;
                };
                Some(call.target_symbol)
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, [state.symbol]);
    }
    assert_template_unchanged(&original, &program, "repeat");
}

fn assert_template_unchanged(before: &TypedTrees, after: &TypedTrees, name: &str) {
    let original = before
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("original template");
    let retained = after
        .machines()
        .iter()
        .find(|machine| machine.symbol == original.symbol)
        .expect("retained template");
    assert_eq!(retained, original);
    assert_eq!(
        after.machine_type_parameters(retained),
        before.machine_type_parameters(original)
    );
    assert!(!after.machine_type_parameters(retained).is_empty());
    assert_eq!(
        after.machine_states(retained),
        before.machine_states(original)
    );
    for state in before.machine_states(original) {
        let statements = before.statement_table.statements(state.statement_nodes);
        assert_eq!(
            after.statement_table.statements(state.statement_nodes),
            statements
        );
        let mut expressions = Vec::new();
        for statement in statements {
            collect_statement_expression_trees(before, statement, &mut expressions);
        }
        for expression in expressions {
            assert_eq!(
                after.expression_table.expression(expression),
                before.expression_table.expression(expression)
            );
        }
    }
}

fn call_targets(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<SymbolHandle> {
    let mut targets = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Call(call) = statement {
                targets.push(call.target_symbol);
            }
            let mut expressions = Vec::new();
            collect_statement_expression_trees(program, statement, &mut expressions);
            for expression in expressions {
                if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                {
                    targets.push(call.target_symbol);
                }
            }
        }
    }
    targets
}

#[test]
fn mutually_recursive_generic_instances_reuse_exact_tuples_without_changing_templates() {
    let mut program = typed(
        r#"
        pub machine ping<T>(value: &T) { pong(value); }
        machine pong<T>(value: &T) { ping(value); }
        machine first(value: &u8) { ping(value); }
        machine second(value: &u16) { ping(value); }
    "#,
    );
    let original = program.clone();
    let mut selections = validation::ValidatedStaticMachineSelections::default();
    monomorphize_generic_machine_value_calls_with_selections(&mut program, &mut selections, true)
        .expect("mutual recursion closes both concrete tuples");
    assert_eq!(program.machine_specializations.len(), 4);
    for name in ["ping", "pong"] {
        assert_template_unchanged(&original, &program, name);
    }
    for receipt in &program.machine_specializations {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == receipt.instance)
            .expect("private instance");
        assert!(!machine.is_public);
        assert!(program.machine_type_parameters(machine).is_empty());
        assert_ne!(receipt.template, receipt.instance);
        let targets = call_targets(&program, machine);
        let [target] = targets.as_slice() else {
            panic!("one exact recursive target: {targets:?}");
        };
        let destination = program
            .machines()
            .iter()
            .find(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == *target)
            })
            .expect("selected recursive callee");
        let peer = program
            .machine_specializations
            .iter()
            .find(|peer| peer.instance == destination.symbol)
            .expect("peer receipt");
        assert_ne!(peer.template, receipt.template);
        assert_eq!(
            peer.type_argument_identities,
            receipt.type_argument_identities
        );
    }
    let instances = program
        .machine_specializations
        .iter()
        .map(|receipt| receipt.instance)
        .collect::<Vec<_>>();
    monomorphize_generic_machine_value_calls_with_selections(&mut program, &mut selections, true)
        .expect("repeated specialization is a fixed point");
    assert_eq!(
        program
            .machine_specializations
            .iter()
            .map(|receipt| receipt.instance)
            .collect::<Vec<_>>(),
        instances
    );
}

#[test]
fn nested_generic_reference_forwarding_only_specializes_the_closed_caller() {
    let mut program = typed(
        r#"
        data Buffer<T> { value: T; }
        machine inspect<T>(value: &T) {}
        pub machine relay<U>(value: &Buffer<U>) { inspect(value); }
        machine first(value: &Buffer<u8>) { relay(value); }
        machine second(value: &Buffer<u8>) { relay(value); }
    "#,
    );
    let original = program.clone();
    monomorphize_generic_machine_value_calls_with_selections(
        &mut program,
        &mut Default::default(),
        true,
    )
    .expect("nested reference argument closes in the cloned caller");
    assert_template_unchanged(&original, &program, "relay");
    assert_template_unchanged(&original, &program, "inspect");
    assert_eq!(program.machine_specializations.len(), 2);
    let relay = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay");
    let relay_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| receipt.template == relay.symbol)
        .expect("one relay instance");
    let relay_instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == relay_receipt.instance)
        .expect("relay instance");
    let entry = program.machine_states(relay_instance)[0].symbol;
    for name in ["first", "second"] {
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("caller");
        assert_eq!(call_targets(&program, caller), [entry]);
    }
    let inspect_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| receipt.template != relay.symbol)
        .expect("one inspect instance");
    let inspect_instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == inspect_receipt.instance)
        .expect("inspect instance");
    assert_eq!(
        call_targets(&program, relay_instance),
        [program.machine_states(inspect_instance)[0].symbol]
    );
    assert!(!relay_instance.is_public);
    assert!(!inspect_instance.is_public);
}

#[test]
fn selected_applications_borrow_template_metadata_without_sharing_binding_mutation() {
    let program = typed(
        "machine identity<T [copy]>(value: T) -> T { value }
         machine caller(value: u8) -> u8 { identity(value) }",
    );
    let candidates = candidate::collect(&program);
    let callees = candidate::callees(&program, &candidates);
    let contracts = contract_expression_handles(&program);
    let selections = collect_call_selections(&program, &candidates, &callees, &contracts);
    let selection = selections
        .iter()
        .find(|selection| selection.is_complete())
        .expect("closed call");
    let template = &candidates[selection.candidate_index];
    let mut first = candidate_for_selection(template, selection);
    let second = candidate_for_selection(template, selection);
    assert!(std::ptr::eq(
        first.template.as_ref(),
        template.template.as_ref()
    ));
    assert!(std::ptr::eq(
        second.template.as_ref(),
        template.template.as_ref()
    ));
    assert_ne!(first.type_bindings.as_ptr(), second.type_bindings.as_ptr());
    first.type_bindings[0] = None;
    assert!(second.type_bindings[0].is_some());
    assert!(selection.type_bindings[0].is_some());
    assert!(template.type_bindings[0].is_none());
}

#[test]
fn runtime_call_subject_collection_matches_lexical_owner_lookup() {
    let program = typed(
        "machine recurse<Count: u32>(value: u32) -> u32 {
             let next: u32 = value;
             recurse<next>(recurse<value>(value))
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let state_symbols = states
        .iter()
        .map(|state| (state.symbol, state.symbol))
        .collect::<Vec<_>>();
    let calls = cloned_runtime_call_subjects(&program, machine.states, &state_symbols, 0);
    assert!(calls.len() >= 2);
    for (expression, actual) in &calls {
        let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
            panic!("retained call");
        };
        let owner = states
            .iter()
            .find(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        let mut expressions = Vec::new();
                        collect_statement_expression_trees(&program, statement, &mut expressions);
                        expressions.contains(expression)
                    })
            })
            .expect("lexical owner");
        assert_eq!(
            *actual,
            runtime_value_subjects(&program, owner, &call.machine_arguments)
        );
    }
    let after_region = program.expression_table.iter_expressions().count();
    assert!(
        cloned_runtime_call_subjects(&program, machine.states, &state_symbols, after_region)
            .is_empty()
    );
}

#[test]
fn structural_static_argument_commitments_use_normalized_type_identity() {
    let program = typed(
        "machine exclusive(value: u64[0..257]) -> u64 { 0 }
         machine inclusive(value: u64[0..=256]) -> u64 { 0 }
         machine smaller(value: u64[0..256]) -> u64 { 0 }
         machine seven(value: [u8; 7]) -> u64 { 0 }
         machine eight(value: [u8; 8]) -> u64 { 0 }
         machine generic_first<T>(value: [T; 7]) -> u64 { 0 }
         machine generic_renamed<U>(value: [U; 7]) -> u64 { 0 }",
    );
    let commitment = |name: &str| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("fixture machine");
        let state = &program.machine_states(machine)[0];
        let argument = super::StaticMachineArgument {
            type_reference: program.state_parameters(state)[0].type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        };
        let binders = program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        super::identities::encode_bound_static_argument(
            &program,
            &argument,
            &[],
            &binders,
            &mut bytes,
        );
        bytes
    };
    assert_eq!(commitment("exclusive"), commitment("inclusive"));
    assert_ne!(commitment("exclusive"), commitment("smaller"));
    assert_ne!(commitment("seven"), commitment("eight"));
    assert_ne!(commitment("exclusive"), commitment("seven"));
    assert_eq!(commitment("generic_first"), commitment("generic_renamed"));
}
