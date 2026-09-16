use crate::lowerer::lower_symbol_resolved_trees;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn lowers_machine_contract_clauses() {
    let source = r#"
    machine distinct_indices(i: usize, j: usize)
    requires
        i < j
    ensures
        i != j
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let machine = typed_trees.machines().first().expect("machine");
    let contracts = typed_trees.machine_contracts(machine);

    assert_eq!(contracts.len(), 2);
    assert!(contracts[0].token_count >= 3);
    assert!(contracts[1].token_count >= 3);
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("typed contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
    assert_eq!(
        typed_trees
            .proof_facts
            .span_or_empty(contracts[0].facts)
            .len(),
        1
    );
    assert_eq!(
        typed_trees
            .proof_facts
            .span_or_empty(contracts[1].facts)
            .len(),
        1
    );
}

#[test]
fn lowers_named_contract_evidence_bindings() {
    let source = r#"
    proposition carries(value: i32) evidence i32;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let contracts = typed.machine_contracts(machine);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0].binding.as_ref().map(|name| name.as_str()),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1].binding.as_ref().map(|name| name.as_str()),
        Some("output_proof")
    );
}

#[test]
fn lowers_statement_argument_spans_from_statement_table() {
    let source = r#"
    data Parser {}

    machine Parser::start(&mut self, level: i32, cell: i32, line: i32) -> i32 {
        transition {
            _ -> self.resolve_exit(level, cell, line)
        }

        state resolve_exit(&mut self, level: i32, cell: i32, line: i32) -> i32 {
            0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let machine = &typed_trees.machines()[0];
    let entry = &typed_trees.machine_states(machine)[0];
    let statements = typed_trees
        .statement_table
        .statements(entry.statement_nodes);

    let typed_trees::statement::StatementNode::Transition(transition) = &statements[0] else {
        panic!("entry should lower to transition statement");
    };
    let typed_trees::statement::TransitionTargetNode::Named {
        arguments,
        source_span,
        authored_call_selection,
        ..
    } = typed_trees
        .statement_table
        .transition_target(transition.target)
    else {
        panic!("transition target should be named");
    };
    assert_eq!(
        &source[source_span.span.start..source_span.span.end],
        "resolve_exit"
    );
    assert!(authored_call_selection.is_some());
    let arguments = typed_trees.statement_table.expression_handles(*arguments);
    let argument_names = arguments
        .iter()
        .map(|argument| typed_trees.expression_table.display_name(*argument))
        .collect::<Vec<_>>();

    assert_eq!(argument_names, ["level", "cell", "line"]);
}

#[test]
fn preserves_linear_multiplicity_through_typed_lowering() {
    let source = r#"
        data Token [linear] {}
        data Holder<T [linear]> [linear] { token: T; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    for definition in typed_trees.data_definitions() {
        assert_eq!(
            definition.properties.multiplicity,
            language_semantics::Multiplicity::Linear
        );
    }
    let holder = &typed_trees.data_definitions()[1];
    assert_eq!(
        typed_trees.data_type_parameters(holder)[0]
            .bounds
            .multiplicity,
        language_semantics::Multiplicity::Linear
    );
}

#[test]
fn indexed_qualification_binder_keeps_machine_const_identity() {
    let source = r#"
        data Unit {}
        domain<T, const U: Unit> T::Quantity<U>;

        trait Conversion {
            machine retag_requirement<const To: Unit>(value: i64) -> i64 in Quantity<To>;
        }

        machine retag<const To: Unit>(value: i64) -> i64 in Quantity<To> {
            transition { _ -> (value as i64 in Quantity<To>) }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    let machine = typed_trees.machines().first().expect("retag machine");
    let [parameter] = typed_trees.machine_type_parameters(machine) else {
        panic!("retag should retain one const parameter");
    };
    assert_eq!(parameter.name.as_str(), "To");
    assert!(matches!(
        parameter.kind,
        typed_trees::data::TypeParameterKind::Const { .. }
    ));
    let state = &typed_trees.machine_states(machine)[0];
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed_trees
        .type_reference_table
        .type_reference(state.return_type)
    else {
        panic!("return should retain Quantity<To>");
    };
    let [typed_trees::types::TypeConstraintNode::Domain(return_domain)] =
        typed_trees.type_reference_table.constraints(*constraints)
    else {
        panic!("return should carry one declared domain");
    };
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: return_symbol,
        name: return_name,
    } = typed_trees
        .type_reference_table
        .type_reference(return_domain.arguments[0])
    else {
        panic!("return index should be a direct binder leaf");
    };
    assert_eq!(return_name.as_str(), "To");
    assert_eq!(*return_symbol, parameter.symbol);

    let (cast_expression, cast) = typed_trees
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            typed_trees::expression::ExpressionNode::Cast(cast) => Some((handle, cast)),
            _ => None,
        })
        .expect("retag body should retain its qualification cast");
    let [cast_argument] = typed_trees
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments)
    else {
        panic!("cast should retain one index argument");
    };
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: cast_symbol,
        name: cast_name,
    } = typed_trees
        .type_reference_table
        .type_reference(*cast_argument)
    else {
        panic!("cast index should be a direct binder leaf");
    };
    assert_eq!(cast_name.as_str(), "To");
    assert_eq!(*cast_symbol, parameter.symbol);
    assert_eq!(cast.semantic_domain_id, return_domain.semantic_id);
    let occurrences = typed_trees
        .expression_table
        .authored_selection_occurrences(cast_expression)
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        panic!("qualification cast should retain one exact authored selection")
    };
    let selection = typed_trees
        .authored_declaration_selections()
        .get(*occurrence)
        .expect("qualification-cast occurrence must rejoin its selection");
    assert_eq!(
        selection.kind(),
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership
    );
    assert!(matches!(
        selection.target(),
        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == cast.semantic_domain_symbol
    ));

    let conversion = typed_trees.traits().first().expect("Conversion trait");
    let [requirement] = typed_trees.trait_machine_signatures(conversion) else {
        panic!("Conversion should retain one requirement");
    };
    let [requirement_parameter] = typed_trees.state_signature_type_parameters(requirement) else {
        panic!("generic requirement should retain its const binder");
    };
    let typed_trees::types::TypeReferenceNode::Constrained {
        constraints: requirement_constraints,
        ..
    } = typed_trees
        .type_reference_table
        .type_reference(requirement.return_type)
    else {
        panic!("generic requirement result should retain Quantity<To>");
    };
    let [typed_trees::types::TypeConstraintNode::Domain(requirement_domain)] = typed_trees
        .type_reference_table
        .constraints(*requirement_constraints)
    else {
        panic!("generic requirement result should carry one domain");
    };
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: requirement_symbol,
        ..
    } = typed_trees
        .type_reference_table
        .type_reference(requirement_domain.arguments[0])
    else {
        panic!("generic requirement index should be a direct binder");
    };
    assert_eq!(*requirement_symbol, requirement_parameter.symbol);
}

#[test]
fn typed_snapshots_publish_only_normalized_service_reach() {
    let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        machine emit(text: &[u8])
        reaches Console
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let snapshot = typed.snapshot();
    let [machine] = snapshot.roots.machines.as_slice() else {
        panic!("one typed machine snapshot");
    };

    assert_eq!(machine.service_reach, ["Console"]);
    let [trait_definition] = snapshot.roots.traits.as_slice() else {
        panic!("one typed trait snapshot");
    };
    let [signature] = trait_definition.machines.as_slice() else {
        panic!("one typed trait-machine snapshot");
    };
    assert_eq!(signature.service_reach, ["Console"]);
    assert!(!signature.service_reach_is_installation_bound);
}

#[test]
fn retains_installation_bound_reach_through_typed_snapshot() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete(acknowledgement: u64)
            reaches <= MachineControl + PortIo;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let snapshot = typed.snapshot();
    let trait_definition = snapshot
        .roots
        .traits
        .iter()
        .find(|definition| definition.name == "InterruptCompletion")
        .expect("interrupt completion trait snapshot");
    let [requirement] = trait_definition.machines.as_slice() else {
        panic!("one typed requirement snapshot");
    };

    assert!(requirement.service_reach_is_installation_bound);
    assert_eq!(requirement.service_reach, ["MachineControl", "PortIo"]);
}

#[test]
fn authored_premise_replays_a_generic_application_leaf() {
    // `b.item` selects Box's own member; its declared `T` resumes at the
    // `Context` argument the application bound, so `scheduler` names the
    // substituted declaration's field rather than stopping at an opaque leaf.
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        pub data Box<T> { item: T; }
        pub domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        pub boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        pub machine wait_boxed(b: &Box<Context>)
        requires b.item.scheduler in WeakFair
        terminates;
        -> u64 { 0 }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wait_boxed")
        .expect("wait_boxed machine");
    let language_semantics::TerminationInterface::Published(
        language_semantics::TerminationGuarantee::Terminates { premises },
    ) = &machine.termination_plan.interface
    else {
        panic!("wait_boxed should publish authored premises")
    };
    let [premise] = premises.as_slice() else {
        panic!("wait_boxed should publish exactly one premise")
    };
    let b = typed
        .state_parameters(&typed.machine_states(machine)[0])
        .iter()
        .find(|parameter| parameter.name.as_str() == "b")
        .expect("parameter b");
    assert_eq!(premise.subject.root, b.symbol);
    let projections = premise
        .subject
        .projections
        .iter()
        .map(|symbol| typed.symbols.display_path(*symbol, "::"))
        .collect::<Vec<_>>();
    assert_eq!(projections, ["Box::item", "Context::scheduler"]);
}

#[test]
fn authored_premise_through_a_generic_leaf_still_names_a_declared_member() {
    // The application replays its own declaration only: `scheduler` is a
    // `Context` member, not a `Box` member, so this premise still has no
    // exact field path and must not mint one.
    for requirement in [
        "requires b.scheduler in WeakFair",
        "requires b.item.missing in WeakFair",
    ] {
        let source = format!(
            r#"
            data Main {{}}
            machine Main::run(&mut self) {{}}
            pub data SchedulerHandle [copy] {{}}
            pub data Context {{ scheduler: SchedulerHandle; }}
            pub data Box<T> {{ item: T; }}
            pub domain SchedulerHandle::WeakFair
            satisfies ProgressProfile
            established by SchedulerAdmission::grant;
            pub boundary trait SchedulerAdmission {{
                machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
            }}
            pub machine wait_boxed(b: &Box<Context>)
            {requirement}
            terminates;
            -> u64 {{ 0 }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
        let diagnostic = lower_symbol_resolved_trees(&resolved)
            .expect_err("a non-member premise must still fail normalization");
        assert!(
            diagnostic
                .message
                .contains("must name one identity-preserving parameter or field path"),
            "{requirement}: {}",
            diagnostic.message
        );
    }
}

#[test]
fn typed_snapshot_publishes_normalized_termination_witness() {
    let source = r#"
        machine countdown(remaining: u64)
        terminates by remaining -> Nat::Descending;
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let snapshot = typed.snapshot();
    let [machine] = snapshot.roots.machines.as_slice() else {
        panic!("one typed machine snapshot");
    };
    let witness = machine
        .termination_witness
        .as_ref()
        .expect("normalized ranking witness");

    assert_eq!(witness.subjects, ["remaining"]);
    assert_eq!(witness.view_path, "Nat::Descending");
    assert!(witness.view_arguments.is_empty());
    assert!(witness.rank_range.is_none());
}

#[test]
fn typed_snapshot_retains_trait_owned_operator_token() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let snapshot = typed.snapshot();
    let [trait_definition] = snapshot.roots.traits.as_slice() else {
        panic!("one trait snapshot expected");
    };
    let [requirement] = trait_definition.machines.as_slice() else {
        panic!("one trait requirement expected");
    };

    assert_eq!(requirement.spelling, Some("<"));
}
