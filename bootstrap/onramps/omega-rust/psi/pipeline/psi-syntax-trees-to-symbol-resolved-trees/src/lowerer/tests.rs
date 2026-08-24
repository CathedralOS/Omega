use super::{lower_syntax_trees, lower_syntax_trees_with_sources};
use psi_source::SourceMap;
use psi_source_files_to_tokens::Lexer;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees_with_id;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn retains_public_machine_visibility_in_symbol_resolved_trees() {
    let tokens = Lexer::new("pub machine Package::entry() { }")
        .tokenize()
        .expect("tokenize public machine");
    let syntax = parse_syntax_trees(&tokens).expect("parse public machine");
    let program = lower_syntax_trees(&syntax).expect("resolve public machine");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("resolved public machine");

    assert!(machine.is_public);
    assert_eq!(
        machine.attached_data.as_ref().map(|name| name.as_str()),
        Some("Package")
    );
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );
}

#[test]
fn resolves_name_owned_conformance_telescope_in_its_own_scope() {
    let source = r#"
        trait Converter<Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let conformance = program.conformances.iter().next().expect("one conformance");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    assert_eq!(
        program.symbols.get(conformance.symbol).parent,
        program.symbols.root(),
        "a conformance name is package-scoped even when its subject is a carrier"
    );
    let parameters = program.data_type_parameters(conformance.type_parameters);
    assert_eq!(parameters.len(), 3);
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );
    assert!(
        parameters
            .iter()
            .all(|parameter| program.symbols.get(parameter.symbol).parent == conformance.symbol)
    );

    let source_argument = program
        .tables
        .declarations
        .child_type_references
        .span_or_empty(conformance.arguments)
        .first()
        .expect("Source trait argument");
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } = source_argument
    else {
        panic!("Source should remain a named reference");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);

    let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
        &parameters[2].kind
    else {
        panic!("Convert should be a machine parameter");
    };
    let contract = program
        .machine_parameter_contract_view(contract)
        .expect("structural machine contract")
        .signature();
    let contract_parameter = program
        .state_parameters(contract.parameters)
        .first()
        .expect("Convert value parameter");
    let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
        &contract_parameter.type_reference
    else {
        panic!("contract parameter should remain named");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);
}

#[test]
fn resolves_forward_declared_nominal_machine_parameter_to_exact_requirement() {
    let source = r#"
        machine register<machine Selected>(value: u32) -> u64
        where machine Selected satisfies WindowProcedure::call;
        {
            Selected(value)
        }

        boundary trait WindowProcedure {
            machine call(value: u32) -> u64;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve nominal requirement");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = program
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
    else {
        panic!("Selected should be a machine parameter");
    };
    let psi_symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
    } = contract
    else {
        panic!("Selected should retain an exact nominal requirement");
    };
    let trait_definition_row = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("WindowProcedure trait");
    let requirement_row = program
        .trait_machine_signatures(trait_definition_row.machines)
        .first()
        .expect("call requirement");

    assert_eq!(*trait_definition, trait_definition_row.symbol);
    assert_eq!(*requirement, requirement_row.symbol);
    assert_ne!(parameter.symbol, requirement_row.symbol);
    let psi_symbol_resolved_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement,
    } = program
        .machine_parameter_contract_view(contract)
        .expect("valid nominal contract view")
    else {
        panic!("nominal view")
    };
    assert_eq!(trait_definition.name.as_str(), "WindowProcedure");
    assert_eq!(requirement.name.as_str(), "call");
    assert_eq!(program.state_parameters(requirement.parameters).len(), 1);
}

#[test]
fn rejects_overloaded_nominal_machine_parameter_requirement() {
    let source = r#"
        trait WindowProcedure {
            machine call(value: u32) -> u64;
            machine call(value: u64) -> u64;
        }

        machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let diagnostic = lower_syntax_trees(&syntax).expect_err("overload must reject");

    assert_eq!(diagnostic.len(), 2);
    assert!(
        diagnostic[0]
            .message
            .contains("declaring trait `WindowProcedure`")
    );
    assert!(diagnostic[0].message.contains("source-compatibility break"));
    assert!(
        diagnostic[1]
            .message
            .contains("does not resolve to one exact trait requirement")
    );
    assert!(
        diagnostic
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
}

#[test]
fn rejects_unknown_nominal_machine_parameter_paths() {
    for (path, expected) in [
        ("MissingTrait::call", "does not resolve to one exact trait"),
        (
            "WindowProcedure::missing",
            "does not resolve to one exact trait requirement",
        ),
    ] {
        let source = format!(
            r#"
                trait WindowProcedure {{
                    machine call(value: u32) -> u64;
                }}

                machine register<machine Selected>()
                where machine Selected satisfies {path};
                {{}}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let diagnostic = lower_syntax_trees(&syntax).expect_err("unknown path must reject");
        assert!(
            psi_diagnostics::format_diagnostics(&diagnostic).contains(expected),
            "unexpected diagnostic for {path}: {diagnostic:?}"
        );
    }
}

#[test]
fn nominal_machine_parameter_view_rejects_mismatched_trait_requirement_pair() {
    let source = r#"
        trait First { machine call(value: u32) -> u64; }
        trait Second { machine call(value: u32) -> u64; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve traits");
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
    let mismatched = psi_symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition: first.symbol,
        requirement: second_requirement.symbol,
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
    let program = lower_syntax_trees(&syntax).expect("resolve");
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
        psi_symbols::SymbolKind::ConformanceParameter
    );
    let parameters = program.machine_type_parameters(machine);
    assert_eq!(parameters.len(), 1);
    assert_eq!(bound.subject, parameters[0].symbol);
    assert!(bound.carrier.is_valid());
    assert_eq!(program.symbols.name(bound.carrier), "Ranked");
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
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformances = program.conformances.iter().collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        panic!("one conformance");
    };
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let program = lower_syntax_trees(&syntax_trees).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    assert!(matches!(
        conformance.subject,
        psi_symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
    ));
    assert!(conformance.symbol.is_valid());
    assert_eq!(program.symbols.name(conformance.symbol), "ConcreteEvidence");
    assert_eq!(
        program.symbols.get(conformance.symbol).parent,
        program.symbols.root()
    );
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let program = lower_syntax_trees(&syntax_trees).expect("subjectless rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
            psi_symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
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
    let program = lower_syntax_trees(&syntax_trees)
        .expect("the selected trait-default template should cover the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].source,
        psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
    );
    assert!(rows[0].realization_machine.is_valid());
    assert!(rows[0].realization_state.is_valid());
    assert_eq!(
        rows[0].realization_name.as_str(),
        "Card::PowerOrder::Ranked::fallback"
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
    let program = lower_syntax_trees(&syntax_trees)
        .expect("same-named default overloads retain exact declaration identities");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source
            == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
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
    let program = lower_syntax_trees(&syntax_trees)
        .expect("the inline member's complete signature should select one overload");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source
                    == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline
            })
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| {
                row.source
                    == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
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
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
        psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
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
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
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
    crate::synthesize_trait_defaults(&mut syntax_trees)
        .expect("orchestration may synthesize before resolution");
    let program = lower_syntax_trees(&syntax_trees)
        .expect("resolution's mandatory synthesis pass must not duplicate the row");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let program = lower_syntax_trees(&syntax_trees).expect("exact defaults should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.source
            == psi_symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault
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
    let diagnostic = lower_syntax_trees(&syntax_trees)
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
    let program = lower_syntax_trees(&syntax_trees).expect("qualified rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
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
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let Some(psi_symbol_resolved_trees::statement::StatementNode::LocalData(local)) = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .first()
    else {
        panic!("value-call normalization should retain its hoisted initializer");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
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
    let program = lower_syntax_trees(&syntax_trees).expect("closed rows should normalize");
    let conformance = program.conformances.iter().next().expect("one conformance");
    let psi_symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
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
    let [psi_symbol_resolved_trees::statement::StatementNode::Call(call)] = program
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
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");

    let trait_definition = &program.traits[0];
    let [carrier, relation] = program.trait_type_parameters(trait_definition) else {
        panic!("trait should retain its carrier and proposition parameters");
    };
    assert_eq!(
        program.symbols.get(relation.symbol).kind,
        psi_symbols::SymbolKind::PropositionParameter
    );
    let psi_symbol_resolved_trees::data::TypeParameterKind::Proposition { contract } =
        &relation.kind
    else {
        panic!("Relation should retain a proposition signature");
    };
    let [left, right] = program.state_parameters(contract.parameters) else {
        panic!("Relation should retain two value parameters");
    };
    assert!(left.symbol.is_valid() && right.symbol.is_valid());
    for parameter in [left, right] {
        let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
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
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(contract.facts)
    else {
        panic!("resolved proof fact should remain an expression");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("ensures should remain a proposition-family call");
    };
    assert_eq!(call.target_symbol, relation.symbol);
}

#[test]
fn proposition_declarations_resolve_as_a_distinct_proof_category() {
    let source = r#"
        proposition related(left: i32, right: i32);
        proposition witnessed<machine Generator>(value: i32) evidence i32;
        proposition reflexive(value: i32) = related(value, value);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");

    assert_eq!(program.propositions.len(), 3);
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
            .all(|item| program.symbols.get(item.symbol).kind
                == psi_symbols::SymbolKind::Proposition)
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
        psi_symbol_resolved_trees::proposition::PropositionBinderKind::Machine
    ));
    assert_eq!(
        program.symbols.get(generator.symbol).kind,
        psi_symbols::SymbolKind::PropositionMachineParameter
    );
    let psi_symbol_resolved_trees::proposition::PropositionBody::Witness { evidence } =
        &witnessed.body
    else {
        panic!("witness evidence should remain distinct from a body");
    };
    assert!(matches!(
        evidence,
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if symbol.is_valid() && name.as_str() == "i32"
    ));

    let psi_symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        program.propositions[2].body
    else {
        panic!("transparent proposition should retain its source expansion");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
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
        let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            program.tables.bodies.expressions.expression(*argument)
        else {
            panic!("alias arguments should remain parameter names");
        };
        assert!(path.symbol.is_valid());
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            psi_symbols::SymbolKind::Parameter
        );
    }
}

#[test]
fn lowers_dungeon_style_machine_program() {
    let source = r#"
    data Inventory {
        gold: u32[exact];
    }

    machine Inventory::clear {
        pub entry(&mut self, inventory: &mut Inventory) {
            inventory.gold = 0;
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.data_definitions.len(), 1);
    assert_eq!(program.machines.len(), 1);
    assert!(program.machines[0].symbol.is_valid());
    assert_eq!(
        program
            .machine_state_handles(program.machines[0].states)
            .len(),
        1
    );
    let state = program.machine_state_handles(program.machines[0].states)[0];
    assert!(program.machine_state(state).symbol.is_valid());
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "u32")
            .is_some()
    );
}

#[test]
fn normalizes_service_rows_from_resolved_boundary_trait_symbols() {
    let source = r#"
    boundary trait Readable {
    }

    boundary trait Filesystem: Readable {
        machine inspect() reaches Readable;
    }

    trait Policy {
    }

    machine backup() reaches Filesystem {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    let readable = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Readable")
        .expect("Readable boundary trait");
    let filesystem = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Filesystem")
        .expect("Filesystem boundary trait");
    let readable_id = program
        .service_reaches
        .id_for_symbol(readable.symbol)
        .expect("Readable service id");
    let filesystem_id = program
        .service_reaches
        .id_for_symbol(filesystem.symbol)
        .expect("Filesystem service id");
    assert!(
        program
            .service_reaches
            .id_for_symbol(
                program
                    .traits
                    .iter()
                    .find(|definition| definition.name.as_str() == "Policy")
                    .expect("ordinary policy trait")
                    .symbol,
            )
            .is_none(),
        "ordinary traits must not mint service identities",
    );

    let backup = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "backup")
        .expect("backup machine");
    let mut backup_services = vec![readable_id, filesystem_id];
    backup_services.sort_by_key(|service| service.0);
    assert_eq!(
        program
            .service_reach_rows
            .services(backup.service_reach_row),
        backup_services,
        "authored service rows include normalized boundary-parent closure",
    );

    let inspect = program
        .trait_machine_signatures(filesystem.machines)
        .first()
        .expect("Filesystem::inspect signature");
    assert_eq!(
        program
            .service_reach_rows
            .services(inspect.service_reach_row),
        &[readable_id],
    );
}

#[test]
fn rejects_unknown_machine_service_reach_before_resolved_trees() {
    let source = r#"
        machine work()
        reaches MissingService
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("unknown machine service reach must not enter resolved trees");

    assert!(
        diagnostic[0]
            .message
            .contains("machine `work` declares unknown boundary service `MissingService`")
    );
}

#[test]
fn rejects_ordinary_trait_in_machine_service_reach_before_resolved_trees() {
    let source = r#"
        trait Policy {
        }

        machine work()
        reaches Policy
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("ordinary traits must not enter a service row");

    assert!(
        diagnostic[0]
            .message
            .contains("machine `work` declares unknown boundary service `Policy`")
    );
}

#[test]
fn rejects_unknown_machine_parameter_service_reach_before_resolved_trees() {
    let source = r#"
        machine invoke<machine F>()
        where machine F() reaches MissingService;
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("unknown machine-parameter reach must not enter resolved trees");

    assert!(diagnostic[0].message.contains(
        "machine-parameter requirement `F` state `F` declares unknown boundary service `MissingService`"
    ));
}

#[test]
fn rejects_authored_service_reach_on_external_realization_before_resolved_trees() {
    let source = r#"
        boundary trait Process {
            machine exit(code: i32)
            reaches Process;
        }

        machine exit_leaf(code: i32)
        satisfies Process::exit
        via Binding::Syscall(60)
        reaches Process;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("external realization must derive rather than repeat service reach");

    assert!(
        diagnostic[0]
            .message
            .contains("repeats an authored `reaches` row")
    );
}

#[test]
fn retains_external_realization_mechanism_without_rendering_classification() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("resolve external realization");
    let leaf = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");

    let psi_language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism } =
        leaf.supply_mode
    else {
        panic!("bodyless via leaf must retain external supply");
    };
    assert!(binding.is_valid());
    assert_eq!(
        mechanism,
        psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic
    );
    let [conformance] = program.machine_trait_conformances(leaf.satisfies) else {
        panic!("external leaf must retain one exact satisfaction row");
    };
    assert_eq!(conformance.external_binding, Some(binding));
}

#[test]
fn keeps_attached_machines_as_distinct_callables() {
    let source = r#"
    machine Game::new {
        pub entry() {}
    }

    machine Game::running {
        pub entry() {}
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.machines.len(), 2);
    assert_eq!(program.machines[0].name.as_str(), "Game::new");
    assert_eq!(
        program.machines[0]
            .attached_data
            .as_ref()
            .map(|name| name.as_str()),
        Some("Game")
    );
    assert_eq!(program.machines[1].name.as_str(), "Game::running");
    assert_eq!(
        program
            .machine_state_handles(program.machines[0].states)
            .len(),
        1
    );
}

#[test]
fn lowers_domain_definitions() {
    let source = r#"
    domain Player::Valid
    requires
        self.health >= 0

    domain Player::Alive
    requires
        self in Player::Valid;
        self.health > 0

    domain Player::Tagged;

    domain Player::Usable =
        Player::Valid & Player::Alive;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.domain_definitions.len(), 4);
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = program.proof_facts(domain.facts);
    assert_eq!(facts.len(), 2);
    let psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.semantic_clause_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Tagged")
        .expect("tagged domain should lower");
    assert_eq!(
        tagged.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(tagged.semantic_clause_token_count, 0);
    assert!(tagged.semantic_roles.is_empty());
    assert!(
        program
            .symbols
            .find_child_by_name(program.symbols.root(), "Player::Alive")
            .is_some()
    );
    let usable = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Usable")
        .expect("usable alias should lower");
    let alias = usable.alias.as_ref().expect("alias theory");
    assert_eq!(alias.constituents.len(), 2);
    assert!(
        alias
            .constituents
            .iter()
            .all(|constituent| constituent.domain_symbol.is_valid())
    );
    assert!(usable.facts.is_empty(), "aliases are not predicate facts");
}

#[test]
fn resolves_free_machine_calls_in_domain_predicates() {
    let source = r#"
    boundary machine no_wrap(base: addr, length: u64) -> bool;

    data Region {
        base: addr;
        length: u64;
    }

    domain Region::Valid
    requires
        no_wrap(self.base, self.length);
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Valid")
        .expect("valid domain");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(predicate)] =
        program.proof_facts(domain.facts)
    else {
        panic!("one predicate call");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*predicate)
    else {
        panic!("predicate should remain a call");
    };
    assert!(call.target_symbol.is_valid());
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "no_wrap")
        .expect("predicate machine");
    assert!(
        program
            .machine_state_handles(machine.states)
            .iter()
            .any(|state| program.machine_state(*state).symbol == call.target_symbol)
    );
}

#[test]
fn resolves_repeated_capacity_specializations_as_one_domain_identity() {
    let source = r#"
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    domain [u8; 16]::Utf8
    requires
        valid_utf8(self);

    data Holder {
        label: [u8; 8] in Utf8;
    }

    machine fill(out: &mut Holder)
    ensures
        out.label in Utf8
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    assert_eq!(
        program.domain_definitions[0].semantic_id, program.domain_definitions[1].semantic_id,
        "capacity-specialized declarations with the same normalized predicate should share semantic identity",
    );

    let machine = program.machines.first().expect("fill machine");
    let contract = program
        .machine_contracts(machine)
        .iter()
        .find(|contract| {
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("fill should retain its ensures contract");
    let [psi_symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
        program.proof_facts(contract.facts)
    else {
        panic!("ensures should contain one domain membership")
    };
    assert!(membership.domain_symbol.is_valid());
}

#[test]
fn preserves_operator_declarations() {
    let source = r#"
    operator Slice::index<T>(items: &[T], index: usize) -> T
    requires
        index < items.len;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.operators.len(), 1);
    let operator = &program.operators[0];
    assert_eq!(
        program
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Slice", "index"]
    );
    assert_eq!(
        program.data_type_parameters(operator.type_parameters).len(),
        1
    );
    assert_eq!(program.state_parameters(operator.parameters).len(), 2);
    assert!(operator.symbol.is_valid());
    assert!(operator.return_type.is_some());
    assert_eq!(program.signature_contracts(operator.contracts).len(), 1);
    assert!(operator.token_count > 0);
}

#[test]
fn preserves_domain_operator_declarations() {
    let source = r#"
    data Quantity {
        value: i32;
    }

    domain Quantity::Additive
    requires
        self.value >= 0;

    operator Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Quantity::Additive")
        .expect("domain should lower");
    let operators = program.operator_definitions(domain.operators);

    assert_eq!(operators.len(), 1);
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert!(domain.semantic_roles.arithmetic_policy.is_none());
    assert!(operators[0].symbol.is_valid());
    assert_eq!(
        program
            .operator_path_members(operators[0].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
    assert_eq!(program.proof_facts(domain.facts).len(), 1);
}

#[test]
fn infers_top_level_operator_home_from_qualified_operands() {
    let source = r#"
    domain i32::Degrees;

    operator + add(left: i32 in Degrees, right: i32 in Degrees) -> i32 in Degrees;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "i32::Degrees")
        .expect("domain should lower");
    let operator = program
        .operator_definitions(domain.operators)
        .first()
        .expect("qualified operands should supply one semantic home");

    assert!(program.operators.is_empty());
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert_eq!(
        program
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
}

#[test]
fn rejects_ambiguous_inferred_domain_operator_home() {
    let source = r#"
    domain i32::Degrees;
    domain i32::Radians;

    operator + add(left: i32 in Degrees, right: i32 in Radians) -> i32;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("competing operand domains must not infer an operator home");
    assert!(
        diagnostic[0]
            .message
            .contains("has more than one possible domain home")
    );
}

#[test]
fn does_not_infer_domain_establishment_from_contract_placement() {
    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued;

    machine Token::issue(value: u64) -> Token
    ensures
        result in Token::Issued
    {
        Token { value: value }
    }

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Issued;
    }

    domain Token::Stamped;

    operator Token::Stamped::stamp(value: Token) -> Token
    ensures
        result in Token::Stamped;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    let issued = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("issued domain");
    assert!(issued.establishment_routes.is_empty());

    let stamped = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Stamped")
        .expect("stamped domain");
    assert!(program.operator_definitions(stamped.operators).len() == 1);
    assert!(stamped.establishment_routes.is_empty());
}

#[test]
fn normalizes_authored_checked_and_boundary_requirement_routes() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }

    domain Token::Checked
    established by CheckedIssuer::issue;
    domain Token::Admitted
    established by BoundaryIssuer::issue;

    trait CheckedIssuer {
        machine issue(value: u64) -> Token in Checked;
    }
    boundary trait BoundaryIssuer {
        machine issue(value: u64) -> Token
        ensures result in Token::Admitted;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    for (domain_name, trait_name, is_boundary) in [
        ("Token::Checked", "CheckedIssuer", false),
        ("Token::Admitted", "BoundaryIssuer", true),
    ] {
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == domain_name)
            .expect("domain");
        let definition = program
            .traits
            .iter()
            .find(|definition| definition.name.as_str() == trait_name)
            .expect("trait");
        let requirement = program
            .trait_machine_signatures(definition.machines)
            .first()
            .expect("requirement");
        let expected = if is_boundary {
            DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: definition.symbol,
                requirement: requirement.symbol,
            }
        } else {
            DomainEstablishmentRoute::CheckedRequirement {
                trait_definition: definition.symbol,
                requirement: requirement.symbol,
            }
        };
        assert!(domain.establishment_routes.contains(&expected));
    }
}

#[test]
fn preserves_explicit_progress_profile_classification_during_resolution() {
    let source = r#"
    data SchedulerHandle {}
    domain SchedulerHandle::WeakFair
    satisfies ProgressProfile
    established by SchedulerAdmission::grant;
    boundary trait SchedulerAdmission {
        machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "SchedulerHandle::WeakFair")
        .expect("profile domain");

    assert_eq!(
        domain.classification,
        Some(psi_language_semantics::DomainClassification::ProgressProfile)
    );
    assert!(matches!(
        domain.establishment_routes.as_slice(),
        [psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }]
    ));
}

#[test]
fn boundary_requirement_route_accepts_exact_non_self_parameter_domain() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token { value: u64; }
    domain Token::Pending
    established by BoundaryIngress::enter;
    boundary trait BoundaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Pending")
        .expect("pending domain");
    let ingress = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "BoundaryIngress")
        .expect("boundary ingress trait");
    let enter = program
        .trait_machine_signatures(ingress.machines)
        .first()
        .expect("entry requirement");
    assert!(
        domain
            .establishment_routes
            .contains(&DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: ingress.symbol,
                requirement: enter.symbol,
            })
    );
}

#[test]
fn ordinary_requirement_route_rejects_parameter_domain_as_introduction() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Pending
    established by OrdinaryIngress::enter;
    trait OrdinaryIngress {
        machine enter(token: Token in Pending);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("an ordinary call must treat its parameter domain as a precondition");
    assert!(diagnostic[0].message.contains(
        "does not name the domain on its exact result or an exact non-self external-root parameter"
    ));
}

#[test]
fn rejects_unresolved_authored_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by MissingIssuer::issue;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees).expect_err("route must resolve exactly");
    assert!(
        diagnostic[0]
            .message
            .contains("does not resolve to one exact trait")
    );
}

#[test]
fn rejects_overloaded_signature_free_domain_requirement_route() {
    let source = r#"
    data Token { value: u64; }
    domain Token::Issued
    established by Issuer::issue;
    trait Issuer {
        machine issue(value: u64) -> Token in Issued;
        machine issue(value: i64) -> Token in Issued;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let diagnostic = lower_syntax_trees(&syntax_trees)
        .expect_err("a signature-free requirement path must not choose among overloads");
    assert_eq!(diagnostic.len(), 2);
    assert!(diagnostic[0].message.contains("declaring trait `Issuer`"));
    assert!(
        diagnostic[1]
            .message
            .contains("does not resolve to one exact trait requirement")
    );
    assert!(
        diagnostic
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
}

#[test]
fn signature_free_overload_reports_one_declaration_and_every_affected_use() {
    let source = r#"
        data Token { value: u64; }
        domain Token::Issued
        established by Issuer::issue;

        trait Issuer {
            machine issue(value: u64) -> Token in Issued;
            machine issue(value: i64) -> Token in Issued;
        }

        machine register<machine Selected>()
        where machine Selected satisfies Issuer::issue;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let diagnostics = lower_syntax_trees(&syntax).expect_err("overload must reject every use");

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("declaring trait `Issuer`"));
    assert!(diagnostics[1].message.starts_with("domain `Token::Issued`"));
    assert!(
        diagnostics[2]
            .message
            .starts_with("nominal machine parameter `Selected`")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_span.is_some())
    );
    assert!(
        diagnostics[1].source_span.unwrap().span.start
            < diagnostics[2].source_span.unwrap().span.start
    );
}

#[test]
fn expands_alias_establishment_routes_to_atomic_domains() {
    use psi_language_semantics::DomainEstablishmentRoute;

    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued
    established by TokenIssuer::issue;
    domain Token::Stamped
    established by TokenIssuer::issue;
    domain Token::Ready = Token::Issued & Token::Stamped;

    boundary trait TokenIssuer {
        machine issue(value: u64) -> Token
        ensures
            result in Token::Ready;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let issuer = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "TokenIssuer")
        .expect("boundary trait");
    let requirement = program
        .trait_machine_signatures(issuer.machines)
        .first()
        .expect("issue requirement");
    let route = DomainEstablishmentRoute::BoundaryRequirement {
        boundary_trait: issuer.symbol,
        requirement: requirement.symbol,
    };

    for name in ["Token::Issued", "Token::Stamped"] {
        let atom = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .expect("atomic domain");
        assert_eq!(atom.establishment_routes, [route]);
    }
    let alias = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Ready")
        .expect("alias domain");
    assert!(
        alias.establishment_routes.is_empty(),
        "routes belong to normalized atomic facts, not alias spellings"
    );
}

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
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("machine");
    let contracts = program.machine_contracts(machine);

    assert_eq!(contracts.len(), 2);
    assert!(contracts[0].token_count >= 3);
    assert!(contracts[1].token_count >= 3);
    assert_eq!(program.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(program.proof_facts(contracts[1].facts).len(), 1);
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
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward machine");
    let contracts = program.machine_contracts(machine);
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
fn classifies_evidence_forwarding_out_of_runtime_statements() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    machine forward(value: i32)
    requires input_proof: carries(value)
    ensures output_proof: carries(value)
    {
        output_proof = input_proof;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let [forwarding] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence forwarding expected");
    };
    assert!(forwarding.machine_symbol.is_valid());
    assert!(forwarding.state_symbol.is_valid());
    assert_eq!(forwarding.target.as_str(), "output_proof");
    assert_eq!(forwarding.source.as_str(), "input_proof");
    assert_eq!(forwarding.source_conformance, None);
    assert_eq!(program.snapshot().evidence_forwardings.len(), 1);
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.symbol == forwarding.machine_symbol)
        .expect("owner machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    assert!(
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .is_empty(),
        "erased forwarding must not enter runtime statement spans"
    );
}

#[test]
fn resolves_explicit_evidence_producer_to_exact_subjectless_conformance() {
    let source = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    machine produce(value: i32)
    ensures output_proof: carries(value)
    {
        output_proof = ConcreteEvidence;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");
    let [assignment] = program.evidence_forwardings.as_slice() else {
        panic!("one resolved evidence assignment expected");
    };
    let producer = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "ConcreteEvidence")
        })
        .expect("subjectless producer conformance");
    assert_eq!(assignment.source_conformance, Some(producer.symbol));
    assert_eq!(
        program.snapshot().evidence_forwardings[0].source_conformance,
        Some(producer.symbol.arena_index())
    );
}

#[test]
fn binds_evidence_forwarding_to_attached_machine_with_duplicate_short_name() {
    let source = r#"
    data Left {}
    data Right {}
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;

    machine Left::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }

    machine Right::forward(value: i32)
    requires incoming: carries(value)
    ensures outgoing: carries(value)
    {
        outgoing = incoming;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax_trees).expect("lower");

    assert_eq!(program.evidence_forwardings.len(), 2);
    for (root_index, forwarding) in program.evidence_forwardings.iter().enumerate() {
        let machine = program
            .machines
            .iter()
            .nth(root_index)
            .expect("parallel root machine");
        assert_eq!(forwarding.machine_root_index, root_index);
        assert_eq!(forwarding.machine_symbol, machine.symbol);
    }
    assert_ne!(
        program.evidence_forwardings[0].machine_symbol,
        program.evidence_forwardings[1].machine_symbol
    );
}

#[test]
fn resolves_generic_calls_inside_machine_contracts() {
    let source = r#"
    data Index {
        case Zero;
        case Next(previous: Index);
    }

    machine generic<machine S>(value: Index) -> Index
    where machine S(index: Index) -> Index;
    {
        value
    }

    machine witness<machine Selected>(value: Index)
    where machine Selected(index: Index) -> Index;
    ensures generic<Selected>(value) == value
    {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let witness = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "witness")
        .expect("witness machine");
    let ensures = program
        .machine_contracts(witness)
        .iter()
        .find(|contract| {
            contract.kind == psi_symbol_resolved_trees::signature::SignatureContractKind::Ensures
        })
        .expect("witness ensures");
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(ensures.facts)
    else {
        panic!("one expression fact")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("equality expression")
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(binary.left)
    else {
        panic!("generic call on equality left")
    };
    assert!(call.target_symbol.is_valid());
    assert_eq!(call.machine_arguments.len(), 1);
    assert!(call.machine_arguments[0].symbol.is_valid());
}

#[test]
fn lowers_attached_main_state_name_as_main() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");

    assert_eq!(program.machines.len(), 1);
    assert_eq!(program.machines[0].name.as_str(), "Main::main");
    assert_eq!(
        program.machines[0]
            .attached_data
            .as_ref()
            .map(|name| name.as_str()),
        Some("Main")
    );
    let state = program
        .machine_state_handles(program.machines[0].states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("entry state");
    assert_eq!(state.name.as_str(), "main");
}

#[test]
fn transition_target_prefers_state_over_same_named_attached_field() {
    let source = r#"
    data Main { next: bool; }

    machine Main::main(&mut self) {
        transition { _ -> next() }

        state next(&mut self) {}
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("main machine");
    let states = program.machine_state_handles(machine.states);
    let entry = program.machine_state(states[0]);
    let next = program.machine_state(states[1]);
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) =
        &program.state_statements(entry.statements)[0]
    else {
        panic!("main should transition to next");
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("next should remain a named transition target");
    };

    assert_eq!(target.symbol, next.symbol);
    assert_eq!(
        program.symbols.get(target.symbol).kind,
        psi_symbols::SymbolKind::State
    );
}

#[test]
fn resolves_qualified_attached_machine_tail_transition() {
    let source = r#"
    data Main {}

    machine Main::pack(left: i32, right: i32) -> i32 {
        left + right
    }

    machine Main::issue() -> i32 {
        transition { _ -> Main::pack(1, 2) }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let pack = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pack")
        .expect("pack machine");
    let pack_state = program
        .machine_state_handles(pack.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("pack state");
    let issue = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Main::issue")
        .expect("issue machine");
    let issue_state = program
        .machine_state_handles(issue.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("issue state");
    let psi_symbol_resolved_trees::statement::Statement::Transition(transition) = program
        .state_statements(issue_state.statements)
        .last()
        .expect("terminal transition")
    else {
        panic!("issue should end in a transition");
    };
    let psi_symbol_resolved_trees::statement::TransitionTarget::Named(target) = &transition.target
    else {
        panic!("qualified tail call should remain a named transition");
    };

    assert_eq!(target.symbol, pack_state.symbol);
    assert!(target.head_symbol.is_valid());
}

#[test]
fn resolves_self_parameter_type_to_machine_symbol() {
    let source = r#"
    data Main {
    }

    machine Main::main(&mut self) {
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let program = lower_syntax_trees(&syntax_trees).expect("lowering should succeed");
    let machine = program.machines.first().expect("machine");
    let entry = program
        .machine_state_handles(machine.states)
        .first()
        .map(|state| program.machine_state(*state))
        .expect("entry state");
    let parameter = program
        .state_parameters(entry.parameters)
        .first()
        .expect("self parameter");

    let psi_symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &parameter.type_reference
    else {
        panic!("self parameter should retain its authored reference shell");
    };
    let psi_symbol_resolved_trees::types::TypeReference::SelfType { symbol } =
        program.child_type_reference(reference.referee)
    else {
        panic!("self parameter referee should stay explicit");
    };

    assert_eq!(*symbol, machine.symbol);
}

#[test]
fn source_backed_names_are_used_when_sources_are_available() {
    let source = r#"
    data Inventory {
        gold: u32;
    }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees =
        parse_syntax_trees_with_id(source_id, &tokens).expect("parse should succeed");
    let program = lower_syntax_trees_with_sources(&syntax_trees, Arc::new(sources))
        .expect("lowering should succeed");
    let counts = program.symbols.name_storage_counts();

    assert!(
        counts.source_names > 0,
        "source identifiers should be stored by source span"
    );
    assert!(
        counts.owned_names == 0,
        "loaded source-backed identifiers should not allocate owned symbol names"
    );
    assert!(
        counts.static_names > 0,
        "builtins and synthetic roots should stay static"
    );
}

#[test]
fn trait_operator_requirement_retains_fixed_token_after_resolution() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let trait_definition = program.traits.first().expect("Ranked trait");
    let [requirement] = program.trait_machine_signatures(trait_definition.machines) else {
        panic!("one trait operator requirement expected");
    };

    assert_eq!(
        requirement.spelling.map(|spelling| spelling.symbol()),
        Some("<")
    );
}
