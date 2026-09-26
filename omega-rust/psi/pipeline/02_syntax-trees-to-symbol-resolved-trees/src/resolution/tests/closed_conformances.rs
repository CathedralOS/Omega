use crate::resolution::{ResolutionRequest, resolve};
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn nominal_machine_parameter_view_rejects_mismatched_trait_requirement_pair() {
    let source = r#"
        trait First { machine call(value: u32) -> u64; }
        trait Second { machine call(value: u32) -> u64; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve traits");
    let first = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "First")
        .expect("First trait");
    let second = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Second")
        .expect("Second trait");
    let second_requirement = program
        .trait_machine_signatures(second.machines)
        .first()
        .expect("Second::call");
    let mismatched = symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition: first.symbol,
        requirement: second_requirement.symbol,
        authored_path: Vec::new(),
    };

    assert!(
        program
            .machine_parameter_contract_view(&mismatched)
            .is_none()
    );
}

#[test]
fn resolves_explicit_conformance_binder_as_proof_static_machine_child() {
    let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let machine = program.machines.iter().next().expect("machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };
    let binder = bound.binder.expect("binder symbol");
    assert!(binder.is_valid());
    assert_eq!(program.symbols.name(binder), "Order");
    assert_eq!(program.symbols.get(binder).parent, machine.symbol);
    assert_eq!(
        program.symbols.get(binder).kind,
        symbols::SymbolKind::ConformanceParameter
    );
    let parameters = program.machine_type_parameters(machine);
    assert_eq!(parameters.len(), 1);
    assert_eq!(bound.subject, parameters[0].symbol);
    assert!(bound.carrier.is_valid());
    assert_eq!(program.symbols.name(bound.carrier), "Ranked");
}

#[test]
fn resolves_explicit_conformance_binder_as_proof_static_trait_child() {
    let source = r#"
        trait Ranked<Metric> {}

        trait Ordering<Element, Order: Element satisfies Ranked<u32>> {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let ordering = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Ordering")
        .expect("Ordering trait");
    let [bound] = ordering.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };
    let binder = bound.binder.expect("binder symbol");
    assert!(binder.is_valid());
    assert_eq!(program.symbols.name(binder), "Order");
    assert_eq!(program.symbols.get(binder).parent, ordering.symbol);
    assert_eq!(
        program.symbols.get(binder).kind,
        symbols::SymbolKind::ConformanceParameter
    );
    let parameters = program.trait_type_parameters(ordering);
    assert_eq!(parameters.len(), 1);
    assert_eq!(bound.subject, parameters[0].symbol);
    assert!(bound.carrier.is_valid());
    assert_eq!(program.symbols.name(bound.carrier), "Ranked");
    assert_eq!(program.child_type_references(bound.arguments).len(), 1);
}

#[test]
fn retains_callable_conformance_bound_declarations_with_owner_exposure() {
    let source = r#"
        trait Ranked {}
        data Card {}
        PowerOrder: Card satisfies Ranked;

        pub machine rank<Element, Evidence: Element satisfies Ranked>(value: &Element) {}

        machine rank_power<Element>(value: &Element)
        where Element satisfies Card::PowerOrder
        {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize conformance-bound custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance-bound custody");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve conformance-bound custody");
    let ranked = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Ranked")
        .expect("Ranked trait")
        .symbol;
    let card = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Card")
        .expect("Card data")
        .symbol;
    let power_order = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "PowerOrder")
        })
        .expect("PowerOrder conformance")
        .symbol;
    let selections = program.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::TypeReference
            && selection.exposure()
                == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == ranked
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::TypeReference
            && selection.exposure()
                == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == card
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == power_order
            )
    }));
}

#[test]
fn resolves_every_selected_conformance_bound_application_lane() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<'scope, Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine inspect<'view, Element>(value: &'view Element)
        where Element satisfies Card::FullEncoding<'view, Card, Message, 7, rank>
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("inspect machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one selected conformance bound");
    };
    let selected = bound
        .selected_conformance
        .as_ref()
        .expect("selected conformance");
    assert!(matches!(
        program.symbols.get(selected.symbol).kind,
        symbols::SymbolKind::Conformance
    ));
    let application = selected.application.as_ref().expect("complete application");
    assert_eq!(application.lifetime_arguments[0].as_str(), "view");
    assert!(matches!(
        program.symbols.get(application.arguments[0].symbol).kind,
        symbols::SymbolKind::Data
    ));
    assert!(matches!(
        program.symbols.get(application.arguments[1].symbol).kind,
        symbols::SymbolKind::Data
    ));
    assert!(application.arguments[2].const_literal.is_some());
    assert!(matches!(
        program.symbols.get(application.arguments[3].symbol).kind,
        symbols::SymbolKind::State
    ));
    for expected in [
        application.arguments[0].symbol,
        application.arguments[1].symbol,
        application.arguments[3].symbol,
    ] {
        assert!(program.authored_declaration_selections().iter().any(|selection| {
            selection.exposure()
                == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
                && matches!(
                    selection.target(),
                    symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == expected
                )
        }));
    }
}

#[test]
fn lowers_closed_conformance_rows_to_exact_machine_states() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
            machine Self::rank_value(&self) -> u32;
        }
        data Card { }
        machine Card::stable_rank_value(&self) -> u32 { }

        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool { }
            Ranked::rank_value = Card::stable_rank_value;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("closed rows should normalize");
    let conformances = program.conformances.iter().collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        panic!("one conformance");
    };
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.declaring_trait.is_valid()
            && row.requirement.is_valid()
            && row.realization_machine.is_valid()
            && row.realization_state.is_valid()
    }));
    let before = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "before")
        .expect("inline row");
    assert_eq!(before.realization_name.as_str(), "Card::PowerOrder::before");
    let rank = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "rank_value")
        .expect("reference row");
    assert_eq!(rank.realization_name.as_str(), "Card::stable_rank_value");
}

#[test]
fn retains_named_conformance_visibility_and_snapshot_identity() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub PublicCircle: Circle satisfies Shape;
        PrivateCircle: Circle satisfies Shape;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");
    let conformances = program.conformances.iter().collect::<Vec<_>>();

    assert_eq!(conformances.len(), 2);
    assert!(conformances[0].is_public);
    assert!(!conformances[1].is_public);
    let snapshot = program
        .snapshot_json()
        .expect("resolved conformance snapshot");
    assert!(snapshot.contains("\"name\":\"PublicCircle\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"trait_name\":\"Shape\""));
}

#[test]
fn lowers_subjectless_conformance_to_package_symbol_and_closed_rows() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("subjectless block should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    assert!(matches!(
        conformance.subject,
        symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
    ));
    assert!(conformance.symbol.is_valid());
    assert_eq!(program.symbols.name(conformance.symbol), "ConcreteEvidence");
    assert_eq!(
        program.symbols.get(conformance.symbol).parent,
        program.symbols.root()
    );
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let [row] = rows.as_slice() else {
        panic!("one normalized evidence row");
    };
    assert!(row.requirement.is_valid());
    assert!(row.realization_machine.is_valid());
    assert!(row.realization_state.is_valid());
    assert_eq!(row.realization_name.as_str(), "ConcreteEvidence::witness");
    let realization = program
        .machines
        .iter()
        .find(|machine| machine.symbol == row.realization_machine)
        .expect("inline realization machine");
    assert!(realization.attached_data.is_none());
}

#[test]
fn subjectless_inline_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Evidence {
            machine first(value: i32);
            machine second(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine first(value: i32) { second(value); }
            machine second(value: i32) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("subjectless block should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let first_machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("first realization");
    let first_state = program.machine_state(
        *program
            .machine_state_handles(first_machine.states)
            .first()
            .expect("first realization state"),
    );
    let call = program
        .tables
        .bodies
        .statements
        .statements(first_state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("first calls second");
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn closed_conformance_blocks_never_fall_back_to_ambient_attached_machines() {
    let source = r#"
        trait Ranked { machine Self::before(&self, other: &Self) -> bool; }
        data Card { }
        machine Card::before(&self, other: &Card) -> bool { }
        PowerOrder: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("closed map must ignore the ambient attached look-alike");
    assert!(
        diagnostic[0]
            .message
            .contains("is incomplete: missing `Ranked::before`")
    );
}

#[test]
fn closed_conformance_retains_trait_default_selection_rows() {
    let source = r#"
        trait Ranked { machine Self::fallback(&self) { } }
        data Card { }
        PowerOrder: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("the selected trait-default template should cover the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].source,
        symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    assert!(rows[0].realization_machine.is_valid());
    assert!(rows[0].realization_state.is_valid());
    assert_eq!(
        rows[0].realization_name.as_str(),
        "Card::PowerOrder::Ranked::fallback"
    );
    let realization = program
        .machines
        .iter()
        .find(|machine| machine.symbol == rows[0].realization_machine)
        .expect("selected default realization");
    let [requirement] = program.machine_trait_conformances(realization.satisfies) else {
        panic!("default realization retains one exact requirement edge");
    };
    assert_eq!(requirement.symbol, rows[0].declaring_trait);
    assert_eq!(
        requirement.requirement.as_ref().map(|name| name.as_str()),
        Some("fallback")
    );
}

#[test]
fn inherited_trait_default_applications_partition_shared_authored_calls() {
    let source = r#"
        trait Resettable {
            machine set(&mut self, value: i32);
            machine reset(&mut self) { self.set(30); }
        }
        trait Counter { requires Resettable; }

        data Left { value: i32; }
        LeftCounter: Left satisfies Counter;
        machine Left::set(&mut self, value: i32) { self.value = value; }

        data Right { value: i32; }
        RightCounter: Right satisfies Counter;
        machine Right::set(&mut self, value: i32) { self.value = value; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("source should parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("each inherited default application should own its routed call");

    let applications = ["Left::reset", "Right::reset"].map(|name| {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("synthesized attached default");
        let partition = machine
            .compiler_selection_partition
            .expect("default application partition");
        let [requirement] = program.machine_trait_conformances(machine.satisfies) else {
            panic!("synthesized default retains one requirement edge");
        };
        assert_eq!(
            requirement.requirement.as_ref().map(|name| name.as_str()),
            Some("reset")
        );
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let call = program
            .state_statements(state.statements)
            .iter()
            .find_map(|statement| match statement {
                symbol_resolved_trees::statement::Statement::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        let occurrence = call
            .authored_call_selection
            .expect("routed call selection occurrence");
        let selection = *program
            .authored_declaration_selections()
            .get(occurrence)
            .expect("selection ledger row");
        assert_eq!(selection.compiler_partition(), Some(partition));
        (partition, selection)
    });

    assert_ne!(applications[0].0, applications[1].0);
    assert_eq!(
        applications[0].1.source_span(),
        applications[1].1.source_span()
    );
    assert_ne!(applications[0].1.target(), applications[1].1.target());
}

#[test]
fn parameterized_trait_default_applications_partition_shared_authored_calls() {
    let source = r#"
        trait Sink<T> {
            machine store(&mut self, value: T);
            machine put(&mut self, value: T) { self.store(value); }
        }
        trait IntSink: Sink<i32> { }
        trait ForwardedSink<U>: Sink<U> { }

        data ParentCounter { value: i32; }
        ParentCounterIntSink: ParentCounter satisfies IntSink;
        machine ParentCounter::store(&mut self, value: i32) { self.value = value; }

        data ForwardedCounter { value: i32; }
        ForwardedCounterForwardedSink: ForwardedCounter satisfies ForwardedSink<i32>;
        machine ForwardedCounter::store(&mut self, value: i32) { self.value = value; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("source should parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("each parameterized default application should own its routed call");

    let applications = ["ParentCounter::put", "ForwardedCounter::put"].map(|name| {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("synthesized attached default");
        // A parameterized declaring trait rejoins through the instantiated
        // generic instead of a requirement edge, so the partition must come
        // from the compiler-derived attached machine itself.
        assert!(
            program
                .machine_trait_conformances(machine.satisfies)
                .is_empty()
        );
        let partition = machine
            .compiler_selection_partition
            .expect("parameterized default application partition");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let call = program
            .state_statements(state.statements)
            .iter()
            .find_map(|statement| match statement {
                symbol_resolved_trees::statement::Statement::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        let occurrence = call
            .authored_call_selection
            .expect("routed call selection occurrence");
        let selection = *program
            .authored_declaration_selections()
            .get(occurrence)
            .expect("selection ledger row");
        assert_eq!(selection.compiler_partition(), Some(partition));
        (partition, selection)
    });

    assert_ne!(applications[0].0, applications[1].0);
    assert_eq!(
        applications[0].1.source_span(),
        applications[1].1.source_span()
    );
    assert_ne!(applications[0].1.target(), applications[1].1.target());
}

#[test]
fn synthesized_default_machine_symbol_keeps_authored_carrier_span() {
    let source = r#"
        trait Sink<T> {
            machine store(&mut self, value: T);
            machine put(&mut self, value: T) { self.store(value); }
        }
        data Counter { value: i32; }
        CounterSink: Counter satisfies Sink<i32>;
        machine Counter::store(&mut self, value: i32) { self.value = value; }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let syntax_trees = parse_syntax_trees_with_id(
        source_id,
        &Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed"),
    )
    .expect("source should parse");
    let program = resolve(ResolutionRequest {
        syntax: &syntax_trees,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve should succeed");

    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::put")
        .expect("synthesized attached default");
    let attached = machine
        .attached_data
        .as_ref()
        .expect("synthesized default keeps its carrier occurrence");
    // The generated machine name owns no spelling; the authored carrier
    // occurrence supplies the provenance a package owner is derived from.
    assert_eq!(
        program.symbols.symbol_source_span(machine.symbol),
        Some(attached.source_span())
    );
}

#[test]
fn closed_conformance_retains_every_same_named_default_overload() {
    let source = r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 { value }
            machine Self::convert(&self, value: i32) -> i32 in Saturating { value }
        }
        data Item { }
        Primary: Item satisfies Converter { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("same-named default overloads retain exact declaration identities");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source == symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            && row.requirement.is_valid()
            && row.realization_state.is_valid()
    }));
    assert_ne!(rows[0].requirement, rows[1].requirement);
    assert_ne!(rows[0].realization_state, rows[1].realization_state);
}

#[test]
fn closed_conformance_matches_inline_members_to_result_overloads() {
    let source = r#"
        trait Converter {
            machine Self::convert(&self, value: i32) -> i32 { value }
            machine Self::convert(&self, value: i32) -> i32 in Saturating { value }
        }
        data Item { }
        Primary: Item satisfies Converter {
            machine convert(&self, value: i32) -> i32 in Saturating { value }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("the inline member's complete signature should select one overload");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source == symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
            })
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source
                    == symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            })
            .count(),
        1
    );
    assert_eq!(
        program
            .machines
            .iter()
            .filter(|machine| {
                machine
                    .name
                    .as_str()
                    .contains("Primary::Converter::convert")
            })
            .count(),
        1,
        "the overridden Saturating default candidate must not remain executable"
    );
}

#[test]
fn trait_default_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self) { self.second(); }
            machine Self::second(&self);
        }
        data Card { }
        machine Card::second(&self) { }
        Selected: Card satisfies Pair {
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("default first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("inline second row");
    assert_eq!(
        first.source,
        symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("instantiated default machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("instantiated default state");
    let [symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one default-body call");
    };
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn trait_default_synthesis_is_idempotent_across_orchestration_and_lowering() {
    let source = r#"
        trait Ranked { machine Self::fallback(&self) { } }
        data Card { }
        Selected: Card satisfies Ranked { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let mut syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    crate::preparation::trait_defaults::synthesize_trait_defaults(
        &mut syntax_trees,
        None,
        Vec::new(),
    )
    .expect("orchestration may synthesize before resolution");
    let program = resolve(ResolutionRequest::new(&syntax_trees))
        .expect("resolution's mandatory synthesis pass must not duplicate the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        program
            .machines
            .iter()
            .filter(|machine| machine.name.as_str() == "Card::Selected::Ranked::fallback")
            .count(),
        1
    );
}

#[test]
fn inherited_same_name_defaults_keep_distinct_exact_rows() {
    let source = r#"
        trait Left { machine Self::fallback(&self) { } }
        trait Right { machine Self::fallback(&self) { } }
        trait Both: Left + Right { }
        data Card { }
        Selected: Card satisfies Both { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("exact defaults should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source == symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
            && row.realization_machine.is_valid()
            && row.realization_state.is_valid()
    }));
    assert_ne!(rows[0].declaring_trait, rows[1].declaring_trait);
    assert_ne!(rows[0].realization_name, rows[1].realization_name);
}

#[test]
fn inherited_requirement_collisions_require_trait_qualified_rows() {
    let ambiguous = r#"
        trait LeftOrder { machine Self::before(&self, other: &Self); }
        trait RightOrder { machine Self::before(&self, other: &Self); }
        trait BothOrders: LeftOrder + RightOrder { }
        data Card { }
        Selected: Card satisfies BothOrders {
            machine before(&self, other: &Card) { }
        }
    "#;
    let tokens = Lexer::new(ambiguous)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let diagnostic = resolve(ResolutionRequest::new(&syntax_trees))
        .expect_err("a short row name must not choose one inherited declaration");
    assert!(
        diagnostic[0]
            .message
            .contains("is ambiguous across inherited traits")
    );

    let qualified = r#"
        trait LeftOrder { machine Self::before(&self, other: &Self); }
        trait RightOrder { machine Self::before(&self, other: &Self); }
        trait BothOrders: LeftOrder + RightOrder { }
        data Card { }
        machine Card::left_before(&self, other: &Card) { }
        machine Card::right_before(&self, other: &Card) { }
        Selected: Card satisfies BothOrders {
            LeftOrder::before = Card::left_before;
            RightOrder::before = Card::right_before;
        }
    "#;
    let tokens = Lexer::new(qualified)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("qualified rows should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("qualified rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].declaring_trait, rows[1].declaring_trait);
}

#[test]
fn inline_conformance_member_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self, other: &Self);
            machine Self::second(&self);
        }
        data Card { }
        machine Card::second(&self) { }
        Selected: Card satisfies Pair {
            machine first(&self, other: &Card) { other.second(); }
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state_handles = program.machine_state_handles(machine.states);
    let state = state_handles
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let [symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one call statement");
    };
    assert_eq!(
        call.target_symbol,
        second.realization_state,
        "the ambient Card::second look-alike must not supply the closed row; receiver={:?}, starts_at_self={}, target={}",
        program
            .tables
            .bodies
            .statements
            .name_path_members(call.receiver),
        call.receiver_starts_at_self,
        call.target
    );
}

#[test]
fn inline_conformance_value_calls_route_through_the_same_closed_map() {
    let source = r#"
        trait Pair {
            machine Self::first(&self) -> i32;
            machine Self::second(&self) -> i32;
        }
        data Card { }
        machine Card::second(&self) -> i32 { transition { _ -> (1) } }
        Selected: Card satisfies Pair {
            machine first(&self) -> i32 {
                transition { _ -> (self.second()) }
            }
            machine second(&self) -> i32 { transition { _ -> (2) } }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let Some(symbol_resolved_trees::statement::StatementNode::LocalData(local)) = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .first()
    else {
        panic!("value-call normalization should retain its hoisted initializer");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(local.initial_value)
    else {
        panic!("hoisted value call");
    };
    assert_eq!(call.target_symbol, second.realization_state);
}

#[test]
fn inline_conformance_calls_preserve_a_foreign_receiver_method() {
    let source = r#"
        data Other { }
        machine Other::second(&self) { }
        trait Pair {
            machine Self::first(&self, other: &Other);
            machine Self::second(&self);
        }
        data Card { }
        Selected: Card satisfies Pair {
            machine first(&self, other: &Other) { other.second(); }
            machine second(&self) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("block syntax should parse");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let first = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "first")
        .expect("first row");
    let second = rows
        .iter()
        .find(|row| row.requirement_name.as_str() == "second")
        .expect("second row");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == first.realization_machine)
        .expect("inline first machine");
    let state = program
        .machine_state_handles(machine.states)
        .first()
        .map(|handle| program.machine_state(*handle))
        .expect("inline first state");
    let [symbol_resolved_trees::statement::StatementNode::Call(call)] = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    else {
        panic!("one call statement");
    };
    assert!(call.target_symbol.is_valid());
    assert_ne!(call.target_symbol, second.realization_state);
    assert_eq!(program.symbols.name(call.target_symbol), "second");
}

#[test]
fn proposition_parameter_signatures_receive_distinct_symbols() {
    let source = r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine prove(value: C) ensures Relation(value, value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");

    let trait_definition = &program.traits[0];
    let [carrier, relation] = program.trait_type_parameters(trait_definition) else {
        panic!("trait should retain its carrier and proposition parameters");
    };
    assert_eq!(
        program.symbols.get(relation.symbol).kind,
        symbols::SymbolKind::PropositionParameter
    );
    let symbol_resolved_trees::data::TypeParameterKind::Proposition { contract } = &relation.kind
    else {
        panic!("Relation should retain a proposition signature");
    };
    let [left, right] = program.state_parameters(contract.parameters) else {
        panic!("Relation should retain two value parameters");
    };
    assert!(left.symbol.is_valid() && right.symbol.is_valid());
    for parameter in [left, right] {
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &parameter.type_reference
        else {
            panic!("relation parameter should retain C");
        };
        assert_eq!(*symbol, carrier.symbol);
    }

    let [signature] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("trait should retain one proof signature");
    };
    let [contract] = program.signature_contracts(signature.contracts) else {
        panic!("proof signature should retain one ensures contract");
    };
    let [symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(contract.facts)
    else {
        panic!("resolved proof fact should remain an expression");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("ensures should remain a proposition-family call");
    };
    assert_eq!(call.target_symbol, relation.symbol);
}

#[test]
fn proposition_declarations_resolve_as_a_distinct_proof_category() {
    let source = r#"
        pub proposition related(left: i32, right: i32);
        proposition witnessed<machine Generator>(value: i32) evidence i32;
        proposition reflexive(value: i32) = related(value, value);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");

    assert_eq!(program.propositions.len(), 3);
    assert!(program.propositions[0].is_public);
    assert!(!program.propositions[1].is_public);
    assert!(
        program
            .snapshot_json()
            .expect("resolved proposition snapshot")
            .contains("\"is_public\":true")
    );
    assert_eq!(program.machines.len(), 0);
    assert!(
        program
            .propositions
            .iter()
            .all(|item| item.symbol.is_valid())
    );
    assert!(
        program
            .propositions
            .iter()
            .all(|item| program.symbols.get(item.symbol).kind == symbols::SymbolKind::Proposition)
    );

    let witnessed = &program.propositions[1];
    let [generator] = program
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(witnessed.binders)
    else {
        panic!("witnessed proposition should retain one binder");
    };
    assert!(matches!(
        generator.kind,
        symbol_resolved_trees::proposition::PropositionBinderKind::Machine
    ));
    assert_eq!(
        program.symbols.get(generator.symbol).kind,
        symbols::SymbolKind::PropositionMachineParameter
    );
    let symbol_resolved_trees::proposition::PropositionBody::Witness { evidence } = &witnessed.body
    else {
        panic!("witness evidence should remain distinct from a body");
    };
    assert!(matches!(
        evidence,
        symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if symbol.is_valid() && name.as_str() == "i32"
    ));

    let symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        program.propositions[2].body
    else {
        panic!("transparent proposition should retain its source expansion");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("transparent expansion should remain a proposition call");
    };
    assert_eq!(call.target_symbol, program.propositions[0].symbol);
    for argument in program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    {
        let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            program.tables.bodies.expressions.expression(*argument)
        else {
            panic!("alias arguments should remain parameter names");
        };
        assert!(path.symbol.is_valid());
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            symbols::SymbolKind::Parameter
        );
    }
}
