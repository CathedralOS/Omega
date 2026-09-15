use super::lower_source;
use crate::lowerer::lower_symbol_resolved_trees;
use crate::lowerer::seeded_continuation::retained_typed_base_is_exact_prefix;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn numeric_result_policies_are_retained_without_result_annotations_or_input_ranges() {
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    for source in [
        "machine run(value: u64 [0..10] in Wrapping) { value; }",
        "machine run() { 1u64 as u64 in Wrapping; }",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokens");
        let syntax = parse_syntax_trees(&tokens).expect("syntax");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing");
        let carrier = typed
            .symbols
            .child_handles(typed.symbols.root())
            .unwrap()
            .find(|symbol| {
                typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
            })
            .unwrap();
        let reference = typed
            .type_reference_table
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping)
            .expect("policy-only result retained at producer");
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = typed.type_reference_table.type_reference(reference)
        else {
            panic!("qualified result");
        };
        assert!(
            matches!(typed.type_reference_table.type_reference(*base_type), TypeReferenceNode::Named {symbol, ..} if *symbol == carrier)
        );
        assert!(matches!(
            typed.type_reference_table.constraints(*constraints),
            [TypeConstraintNode::ArithmeticDomain(
                ArithmeticDomain::Wrapping
            )]
        ));
        if let Some(parameter) = typed
            .state_parameters(&typed.machine_states(&typed.machines()[0])[0])
            .first()
        {
            assert_ne!(
                parameter.type_reference, reference,
                "authored range retained separately"
            );
            assert!(
                typed
                    .display_type_reference_with_constraints(parameter.type_reference)
                    .contains("0")
            );
        }
    }
}

#[test]
fn retained_base_rejects_type_identity_changes_hidden_by_display_snapshots() {
    use typed_trees::types::TypeReferenceNode;
    let tokens = Lexer::new("machine main(value: u64) { value; }")
        .tokenize()
        .expect("type identity tokens");
    let syntax = parse_syntax_trees(&tokens).expect("type identity syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("type identity resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type identity typing");
    let parameter = &typed.state_parameters(&typed.machine_states(&typed.machines()[0])[0])[0];
    let mut changed = typed.clone();
    let TypeReferenceNode::Named { name, .. } = changed
        .type_reference_table
        .type_reference(parameter.type_reference)
        .clone()
    else {
        panic!("named scalar parameter");
    };
    changed.type_reference_table.substitute_node(
        parameter.type_reference,
        TypeReferenceNode::Named {
            name,
            symbol: symbols::SymbolHandle::invalid(),
        },
    );
    assert_eq!(
        typed.snapshot(),
        changed.snapshot(),
        "display is not identity"
    );
    assert!(!retained_typed_base_is_exact_prefix(&typed, &changed));
}

#[test]
fn retained_base_rejects_a_changed_local_inference_origin() {
    let tokens = Lexer::new("machine main() -> u64 { let value: u64 = 7; value }")
        .tokenize()
        .expect("local origin tokens");
    let syntax = parse_syntax_trees(&tokens).expect("local origin syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("local origin resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("local origin typing");
    let mut changed = typed.clone();
    let body = changed.machine_states(&changed.machines()[0])[0].statement_nodes;
    let typed_trees::statement::StatementNode::LocalData(local) =
        &mut changed.statement_table.statements_mut(body)[0]
    else {
        panic!("authored local");
    };
    assert!(!local.type_is_inferred);
    local.type_is_inferred = true;
    assert!(!retained_typed_base_is_exact_prefix(&typed, &changed));
}

#[test]
fn inferred_types_do_not_depend_on_generated_binding_names() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Record { value: u64; }
        machine produce() -> u64
        ensures outgoing: ready()
        { outgoing = ConcreteEvidence; 7 }
        machine caller(record: &Record) -> u64 {
            let { value as selected } = record;
            let (runtime; outgoing: witness) = produce();
            runtime
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("binding origin tokens");
    let syntax = parse_syntax_trees(&tokens).expect("binding origin syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve binding origins");
    let typed = lower_symbol_resolved_trees(&resolved).expect("infer unannotated binding types");
    for name in ["selected", "runtime"] {
        let local = typed
            .machines()
            .iter()
            .flat_map(|machine| typed.machine_states(machine))
            .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == name =>
                {
                    Some(local)
                }
                _ => None,
            })
            .expect("authored binding without an annotation");
        assert!(local.type_is_inferred, "{name} has an inferred type");
        assert_eq!(
            typed
                .type_reference_table
                .display_name(local.type_reference),
            "u64"
        );
    }
}

#[test]
fn proof_output_runtime_calls_copy_arguments_into_the_statement_arena() {
    use typed_trees::{expression::ExpressionNode, statement::StatementNode};

    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        machine touch(first: u64, second: u64, third: u64) {}
        machine effect(output: &mut u64, value: u64)
        requires incoming: ready()
        ensures outgoing: ready()
        {
            output = value;
            outgoing = incoming;
        }
        machine caller(destination: &mut u64, selected: u64)
        requires incoming: ready()
        {
            touch(99, 88, 77);
            let (; outgoing: witness) = effect(destination, selected; incoming);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize proof-output call");
    let syntax = parse_syntax_trees(&tokens).expect("parse proof-output call");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve proof-output call");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type proof-output call");
    let caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("caller machine");
    let state = &typed.machine_states(caller)[0];
    let parameters = typed.state_parameters(state);
    let [package] = typed.proof_output_calls.as_slice() else {
        panic!("one proof-output call");
    };
    assert_eq!(package.state_symbol, state.symbol);
    let runtime_index = package
        .runtime_call_statement_index
        .expect("effect executes");
    assert_eq!(
        runtime_index, 1,
        "ordinary call precedes the proof-output call"
    );
    let StatementNode::Call(runtime) =
        &typed.statement_table.statements(state.statement_nodes)[runtime_index]
    else {
        panic!("proof-output runtime statement");
    };
    let ExpressionNode::Call(expression) = typed.expression_table.expression(package.call) else {
        panic!("retained proof-output expression");
    };
    // The preceding three-argument statement deliberately offsets this arena
    // from expression-call argument storage. A raw span copy reads its literals.
    assert_ne!(runtime.arguments, expression.arguments);
    let runtime_arguments = typed.statement_table.expression_handles(runtime.arguments);
    assert_eq!(
        runtime_arguments,
        typed
            .expression_table
            .expression_handles(expression.arguments)
    );
    assert_eq!(runtime_arguments.len(), parameters.len());
    for (argument, parameter) in runtime_arguments.iter().zip(parameters) {
        let ExpressionNode::Name(path) = typed.expression_table.expression(*argument) else {
            panic!("exact caller parameter argument");
        };
        assert_eq!(path.head_symbol, parameter.symbol);
        assert_eq!(path.symbol, parameter.symbol);
        assert_eq!(typed.symbols.get(path.symbol).parent, state.symbol);
    }
}

#[test]
fn inherited_trait_default_realizations_settle_exact_requirement_symbols() {
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let reset_requirement = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Resettable")
        .and_then(|definition| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|requirement| requirement.name.as_str() == "reset")
        })
        .expect("Resettable::reset requirement");

    let applications = ["Left::reset", "Right::reset"].map(|name| {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("synthesized default realization");
        let [conformance] = typed.machine_trait_conformances(machine) else {
            panic!("one synthesized requirement edge");
        };
        assert_eq!(conformance.requirement_symbol, reset_requirement.symbol);
        let state = typed.machine_states(machine).first().expect("entry state");
        let call = typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        call.target_symbol
    });
    assert_ne!(applications[0], applications[1]);
}

#[test]
fn deep_left_associated_boolean_expression_types_on_the_default_test_stack() {
    let expression = std::iter::repeat_n("enabled", 128)
        .collect::<Vec<_>>()
        .join(" && ");
    let source =
        format!("data Root {{}} machine Root::measure(enabled: bool) -> bool {{ {expression} }}");

    lower_source(&source).expect("type deep expression on default test stack");
}

#[test]
fn exact_quoted_bytes_land_as_an_owned_fixed_u8_array() {
    let typed = lower_source(
        r#"
        machine bytes() -> [u8; 2] {
            "\x80A"
        }
        "#,
    )
    .expect("exact-width raw bytes should type");

    let array = typed
        .expression_table
        .expression_entries()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::ArrayLiteral(elements) => Some(*elements),
            _ => None,
        })
        .expect("the contextual string must become an ordinary array literal");
    let values = typed
        .expression_table
        .expression_handles(array)
        .iter()
        .map(
            |element| match typed.expression_table.expression(*element) {
                typed_trees::expression::ExpressionNode::Integer(literal) => {
                    literal.value_i64().expect("byte integer")
                }
                other => panic!("expected byte integer, got {other:?}"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(values, [0x80, i64::from(b'A')]);
}

#[test]
fn quoted_bytes_reject_short_and_long_owned_fixed_array_destinations() {
    for (width, literal_length) in [(3, 2), (1, 2)] {
        let diagnostic = lower_source(&format!("machine bytes() -> [u8; {width}] {{ \"ab\" }}"))
            .expect_err("fixed byte arrays neither pad nor truncate literals");
        assert!(
            diagnostic.message.contains(&format!(
                "quoted byte literal has {literal_length} source byte(s)"
            )) && diagnostic
                .message
                .contains(&format!("requires exactly {width}")),
            "{}",
            diagnostic.message,
        );
    }
}

#[test]
fn quoted_bytes_reject_non_byte_fixed_array_destinations() {
    let diagnostic = lower_source(r#"machine words() -> [u16; 2] { "ab" }"#)
        .expect_err("quoted bytes must not acquire a non-byte element interpretation");
    assert!(
        diagnostic.message.contains("element type must be `u8`"),
        "{}",
        diagnostic.message,
    );
}

#[test]
fn quoted_bytes_reject_nonliteral_or_undetermined_widths() {
    for source in [
        r#"
            machine bytes<const N: u64>() -> [u8; N] {
                "ab"
            }
        "#,
        r#"
            machine width() -> u64 { 2 }
            machine bytes() -> [u8; width()] {
                "ab"
            }
        "#,
    ] {
        let diagnostic = lower_source(source)
            .expect_err("a parameter/call width is not a resolved literal extent");
        assert!(
            diagnostic
                .message
                .contains("width must be a compile-known resolved integer literal"),
            "{}",
            diagnostic.message,
        );
    }
}

#[test]
fn trait_machine_requirement_identity_reaches_typed_trees() {
    let tokens = Lexer::new("trait PrivateCallbackSlot<machine Requirement> {}")
        .tokenize()
        .expect("tokenize trait machine requirement parameter");
    let syntax = parse_syntax_trees(&tokens).expect("parse trait machine requirement parameter");
    let resolved = resolve(ResolutionRequest::new(&syntax))
        .expect("resolve trait machine requirement parameter");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("lower trait machine requirement parameter to typed trees");
    let trait_definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("PrivateCallbackSlot trait");
    let [parameter] = typed.trait_type_parameters(trait_definition) else {
        panic!("one typed trait machine requirement parameter")
    };
    assert!(parameter.symbol.is_valid());
    assert!(matches!(
        parameter.kind,
        typed_trees::data::TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::RequirementIdentity
        }
    ));
}

#[test]
fn exact_trait_requirement_argument_reaches_typed_conformance() {
    let source = r#"
        boundary trait WindowProcedure { machine call(value: u32); }
        trait PrivateCallbackSlot<machine Requirement> {}
        data WndClassLayout {}
        WndClassWindowProcedureSlot:
            WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize private callback slot");
    let syntax = parse_syntax_trees(&tokens).expect("parse private callback slot");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve private callback slot");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type private callback slot");
    let window_procedure = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("WindowProcedure trait");
    let requirement = typed
        .trait_machine_signatures(window_procedure)
        .first()
        .expect("WindowProcedure::call");
    let conformance = typed.conformances().first().expect("slot conformance");
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        panic!("one typed slot requirement argument")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(*argument)
    else {
        panic!("typed requirement argument remains a named identity")
    };
    assert_eq!(name.as_str(), "WindowProcedure::call");
    assert_eq!(*symbol, requirement.symbol);
}

#[test]
fn retains_public_conformance_visibility_snapshot_and_header_selections() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionTarget,
    };

    let source = "pub trait Ranked {} pub data Card {} pub PowerOrder: Card satisfies Ranked {}";
    let tokens = Lexer::new(source).tokenize().expect("tokenize conformance");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve conformance");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type conformance");
    let conformance = typed.conformances().first().expect("typed conformance");

    assert!(conformance.is_public);
    let snapshot = typed.snapshot();
    assert_eq!(snapshot.roots.conformances.len(), 1);
    assert!(snapshot.roots.conformances[0].is_public);
    assert_eq!(snapshot.tables.conformance_count, 1);

    let public_header_targets = typed
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.exposure() == Exposure::PublicInterface)
        .filter_map(|selection| match selection.target() {
            AuthoredDeclarationSelectionTarget::Resolved(target) => {
                Some(typed.symbols.display_path(target.selected_symbol(), "::"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(public_header_targets.iter().any(|target| target == "Card"));
    assert!(
        public_header_targets
            .iter()
            .any(|target| target == "Ranked")
    );
}

#[test]
fn retains_exact_nominal_type_selections_with_declaration_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        pub data Dependency { }
        pub data PublicApi { value: Dependency; }
        data PrivateState { value: Dependency; }
        pub machine expose(value: Dependency) {
            transition { _ -> hidden(value) }
            state hidden(value: Dependency) { }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let dependency = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Dependency")
        .expect("Dependency data")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let mut exposures = typed
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            (selection.kind() == Kind::TypeReference
                && matches!(
                    selection.target(),
                    Target::Resolved(target) if target.selected_symbol() == dependency
                ))
            .then_some(selection.exposure())
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        Exposure::PrivateImplementation => 0,
        Exposure::PublicInterface => 1,
    });

    assert_eq!(
        exposures,
        vec![
            Exposure::PrivateImplementation,
            Exposure::PrivateImplementation,
            Exposure::PublicInterface,
            Exposure::PublicInterface,
        ]
    );
}

#[test]
fn expression_embedded_zero_value_types_keep_contract_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        data Marker {}
        pub proposition public_zero() =
            zero_value<Marker>() == zero_value<Marker>();
        proposition private_zero() =
            zero_value<Marker>() == zero_value<Marker>();
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let marker = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Marker")
        .expect("Marker data")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let mut exposures = typed
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            (selection.kind() == Kind::TypeReference
                && matches!(
                    selection.target(),
                    Target::Resolved(target) if target.selected_symbol() == marker
                ))
            .then_some(selection.exposure())
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        Exposure::PrivateImplementation => 0,
        Exposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [
            Exposure::PrivateImplementation,
            Exposure::PrivateImplementation,
            Exposure::PublicInterface,
            Exposure::PublicInterface,
        ]
    );
}

#[test]
fn expression_embedded_cast_targets_keep_contract_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        data Marker {}
        pub proposition public_cast(value: Marker) = (value as Marker) == value;
        proposition private_cast(value: Marker) = (value as Marker) == value;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let marker = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Marker")
        .expect("Marker data")
        .symbol;
    let cast_targets = resolved
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let symbol_resolved_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            Some(resolved.child_type_reference(cast.target_type).clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(cast_targets.len(), 2, "cast targets: {cast_targets:#?}");
    let cast_target_spans = cast_targets
        .iter()
        .filter_map(|target| {
            let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = target else {
                return None;
            };
            (*symbol == marker).then_some(name.source_span())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cast_target_spans.len(),
        2,
        "cast targets: {cast_targets:#?}"
    );
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let mut exposures = typed
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            (selection.kind() == Kind::TypeReference
                && cast_target_spans.contains(&selection.source_span())
                && matches!(
                    selection.target(),
                    Target::Resolved(target) if target.selected_symbol() == marker
                ))
            .then_some(selection.exposure())
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        Exposure::PrivateImplementation => 0,
        Exposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [Exposure::PrivateImplementation, Exposure::PublicInterface,]
    );
}

#[test]
fn retains_public_operator_visibility_and_signature_exposure() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        pub data Token [copy] { value: u64; }
        pub operator < Token::less(left: Token, right: Token) -> bool;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let token = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let [operator] = typed.operators() else {
        panic!("one root operator")
    };
    assert!(operator.is_public);
    assert!(
        typed
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.kind() == Kind::TypeReference
                    && matches!(
                        selection.target(),
                        Target::Resolved(target) if target.selected_symbol() == token
                    )
            })
            .all(|selection| selection.exposure() == Exposure::PublicInterface)
    );
    assert!(typed.snapshot().roots.operators[0].is_public);
}

#[test]
fn retains_public_data_trait_and_wire_visibility_in_typed_trees() {
    let tokens = Lexer::new(
        "pub data PublicRecord { value: u32; } pub data Packet { #1 value: u32; } pub trait PublicTrait {}",
    )
        .tokenize()
        .expect("tokenize public data");
    let syntax = parse_syntax_trees(&tokens).expect("parse public data");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve public data");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type public data");
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "PublicRecord")
        .expect("typed public data");
    let wire_data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Packet")
        .expect("typed wire-derived data");
    let wire_schema = typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == "Packet")
        .expect("typed wire schema");
    let trait_definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PublicTrait")
        .expect("typed public trait");

    assert!(data.is_public);
    assert!(wire_data.is_public);
    assert!(wire_schema.is_public);
    assert!(trait_definition.is_public);
    let snapshot = typed.snapshot();
    assert!(
        snapshot
            .roots
            .wire_schemas
            .iter()
            .any(|schema| schema.name == "Packet" && schema.is_public)
    );
    assert!(
        snapshot
            .roots
            .traits
            .iter()
            .any(|definition| definition.name == "PublicTrait" && definition.is_public)
    );
}

#[test]
fn retains_public_machine_visibility_in_typed_trees() {
    let tokens = Lexer::new("pub machine Package::entry() { }")
        .tokenize()
        .expect("tokenize public machine");
    let syntax = parse_syntax_trees(&tokens).expect("parse public machine");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve public machine");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type public machine");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("typed public machine");

    assert!(machine.is_public);
    assert_eq!(
        machine.attached_data.as_ref().map(|name| name.as_str()),
        Some("Package")
    );
    assert_eq!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::CheckedBody
    );
}

#[test]
fn retains_structured_external_binding_table_in_typed_trees() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via Binding::DllImport("a,b", "c");
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let leaf = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");
    let [conformance] = typed.machine_trait_conformances(leaf) else {
        panic!("one exact external conformance")
    };
    let binding = conformance.external_binding.expect("external binding id");

    assert_eq!(
        typed.external_bindings.identity(binding),
        Some(&language_semantics::ExternalBindingIdentity::Import {
            library: "a,b".to_owned(),
            symbol: "c".to_owned(),
        })
    );
}

#[test]
fn retains_ordinary_via_call_in_typed_conformance_without_bootstrap_identity() {
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding() -> i32 {
            0
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
    "#;
    let typed = lower_source(source).expect("type ordinary via call");
    let leaf = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write_leaf")
        .expect("external leaf");
    assert_eq!(
        leaf.supply_mode,
        language_semantics::MachineSupplyMode::ExternalRealization {
            binding: None,
            mechanism: None,
        }
    );
    let [conformance] = typed.machine_trait_conformances(leaf) else {
        panic!("one exact external conformance")
    };
    assert!(conformance.external_binding.is_none());
    let typed_trees::expression::ExpressionNode::Call(call) = typed
        .expression_table
        .expression(conformance.via_expression)
    else {
        panic!("ordinary via source must retain its typed call");
    };
    assert_eq!(call.target.as_str(), "binding");
    assert!(call.target_symbol.is_valid());
    assert!(
        typed
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
    );
}

#[test]
fn settles_satisfied_operator_to_its_exact_overload_symbol() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        boundary operator Float::add(left: f32, right: f32) -> f32;
        boundary operator Float::add(left: f64, right: f64) -> f64;

        machine add32(left: f32, right: f32) -> f32
        satisfies Float::add
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "add32")
        .expect("f32 operator satisfier");
    let [conformance] = typed.machine_trait_conformances(machine) else {
        panic!("one exact operator realization")
    };
    let operator =
        typed_trees::operator::declaration_by_symbol(&typed, conformance.requirement_symbol)
            .expect("settled exact operator");
    assert_eq!(typed.display_type_reference(operator.return_type), "f32");
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::StaticPathSegment
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == operator.symbol)
    }));
}

#[test]
fn settles_satisfied_top_level_requirement_to_its_exact_machine_symbol() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        pub boundary requirement InterruptAcknowledgement::complete();

        machine complete_provider()
        satisfies InterruptAcknowledgement::complete
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let requirement_symbol = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement")
        .symbol;
    let satisfier = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "complete_provider")
        .expect("checked satisfier");
    let [conformance] = resolved
        .tables
        .declarations
        .machine_trait_conformances
        .span_or_empty(satisfier.satisfies)
    else {
        panic!("one exact satisfies edge")
    };
    assert_eq!(conformance.symbol, requirement_symbol);

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == requirement_symbol)
        .expect("typed top-level requirement");
    assert_eq!(
        requirement.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    let satisfier = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "complete_provider")
        .expect("typed checked satisfier");
    let [conformance] = typed.machine_trait_conformances(satisfier) else {
        panic!("one typed satisfies edge")
    };
    assert_eq!(conformance.symbol, requirement_symbol);
    assert_eq!(conformance.requirement_symbol, requirement_symbol);
    assert!(matches!(
        typed_trees::machine::resolve_satisfied_declaration(
            &typed,
            satisfier,
            conformance,
        ),
        Some(typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(selected))
            if selected.symbol == requirement_symbol
    ));
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::StaticPathSegment
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == requirement_symbol)
    }));
}

#[test]
fn top_level_requirement_settlement_rejects_an_exact_wrong_supply_machine() {
    let source = r#"
        pub boundary requirement InterruptAcknowledgement::complete();

        machine complete_provider()
        satisfies InterruptAcknowledgement::complete
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let mut resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let requirement = resolved
        .machines
        .find_mut(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement");
    let ordinary_symbol = requirement.symbol;
    requirement.supply_mode = language_semantics::MachineSupplyMode::Boundary;

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let satisfier = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "complete_provider")
        .expect("typed checked satisfier");
    let [conformance] = typed.machine_trait_conformances(satisfier) else {
        panic!("one typed satisfies edge")
    };
    assert_eq!(conformance.symbol, ordinary_symbol);
    assert!(!conformance.requirement_symbol.is_valid());
    assert!(
        typed_trees::machine::resolve_satisfied_declaration(&typed, satisfier, conformance,)
            .is_none()
    );
}

#[test]
fn retains_exact_nominal_machine_parameter_identity_in_typed_trees() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        boundary trait WindowProcedure {
            machine call(value: u32) -> u64;
        }

        machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = typed
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
        panic!("Selected should be a machine parameter")
    };
    let typed_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
    } = contract
    else {
        panic!("Selected should retain nominal identity")
    };
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition: definition,
        requirement: signature,
    } = typed
        .machine_parameter_contract_view(contract)
        .expect("valid exact requirement")
    else {
        panic!("nominal view")
    };

    assert_eq!(*trait_definition, definition.symbol);
    assert_eq!(*requirement, signature.symbol);
    assert_eq!(definition.name.as_str(), "WindowProcedure");
    assert_eq!(signature.name.as_str(), "call");
    assert_ne!(parameter.symbol, signature.symbol);
    assert_eq!(typed.state_signature_parameters(signature).len(), 1);
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::TypeReference
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == definition.symbol)
    }));
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::StaticPathSegment
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == signature.symbol)
    }));
}

#[test]
fn retains_typed_name_owned_conformance_telescope() {
    let source = r#"
        trait Converter<'view, Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<'scope, Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let conformance = typed.conformances().first().expect("one conformance");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    assert_eq!(conformance.trait_lifetime_arguments, vec![0]);
    assert_eq!(
        typed.snapshot().roots.conformances[0].trait_lifetime_arguments,
        vec![0]
    );
    let parameters = typed.conformance_type_parameters(conformance);
    assert_eq!(parameters.len(), 3);
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );
    assert_eq!(conformance.carrier_symbol, parameters[0].symbol);
    let converter = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Converter")
        .expect("Converter trait");
    assert_eq!(conformance.trait_symbol, converter.symbol);
    let arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    assert_eq!(arguments.len(), 2);
    assert!(matches!(
        typed.type_reference_table.type_reference(arguments[0]),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == parameters[0].symbol && name.as_str() == "Source"
    ));
}

#[test]
fn retains_typed_named_conformance_visibility_and_snapshot_identity() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub PublicCircle: Circle satisfies Shape;
        PrivateCircle: Circle satisfies Shape;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let conformances = typed.conformances();

    assert_eq!(conformances.len(), 2);
    assert!(conformances[0].is_public);
    assert!(!conformances[1].is_public);
    let snapshot = typed.snapshot_json().expect("typed conformance snapshot");
    assert!(snapshot.contains("\"name\":\"PublicCircle\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"trait_name\":\"Shape\""));
}

#[test]
fn retains_typed_explicit_conformance_binder_identity() {
    let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };

    assert_eq!(
        bound.binder_name.as_ref().map(|name| name.as_str()),
        Some("Order")
    );
    assert!(bound.binder.is_some_and(|symbol| symbol.is_valid()));
    assert_eq!(
        bound.subject,
        typed.machine_type_parameters(machine)[0].symbol
    );
    let snapshot = typed.snapshot();
    assert_eq!(
        snapshot.roots.machines[0].conformance_bounds[0]
            .binder
            .as_deref(),
        Some("Order")
    );
}

#[test]
fn retains_typed_selected_conformance_bound_application() {
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
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
    assert_eq!(bound.selected_conformance_symbol(), Some(selected.symbol));
    assert_eq!(
        bound.selected_conformance_name().map(|name| name.as_str()),
        Some("FullEncoding")
    );
    let application = selected.application.as_ref().expect("complete application");
    assert_eq!(application.lifetime_arguments[0].as_str(), "view");
    assert_eq!(application.arguments.len(), 4);
    assert!(application.arguments[2].const_literal.is_some());
    assert!(
        application
            .arguments
            .iter()
            .all(|argument| { argument.const_literal.is_some() || argument.symbol.is_valid() })
    );

    let snapshot = typed.snapshot();
    assert!(snapshot.roots.machines.iter().any(|machine| {
        machine.name == "inspect"
            && machine.conformance_bounds[0].selected_conformance.is_some()
            && machine.conformance_bounds[0]
                .selected_conformance_symbol
                .is_some()
    }));
}

#[test]
fn retains_proof_static_evidence_projection_through_resolved_and_typed_trees() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;

        machine consume<machine Witness>()
        where machine Witness() -> i32;
        {}

        machine caller()
        requires proof: holds()
        {
            consume<proof.modulus>();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let resolved_caller = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("resolved caller");
    let resolved_projection = resolved
        .machine_state_handles(resolved_caller.states)
        .iter()
        .flat_map(|state| resolved.state_statements(resolved.machine_state(*state).statements))
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::Statement::Call(call)
                if call.target.as_str() == "consume" =>
            {
                call.machine_arguments[0].evidence_projection.as_ref()
            }
            _ => None,
        })
        .expect("resolved projection");
    assert_eq!(resolved_projection.term.as_str(), "proof");
    assert_eq!(resolved_projection.member.as_str(), "modulus");

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let typed_caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("typed caller");
    let typed_call = typed
        .machine_states(typed_caller)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "consume" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("typed call");
    let projection = typed_call.machine_arguments[0]
        .evidence_projection
        .as_ref()
        .expect("typed projection");
    assert_eq!(projection.term.as_str(), "proof");
    assert_eq!(projection.member.as_str(), "modulus");
    assert!(!typed_call.machine_arguments[0].symbol.is_valid());
    let snapshot = typed.snapshot_json().expect("typed snapshot");
    assert!(snapshot.contains("\"term\":\"proof\",\"member\":\"modulus\""));
}

#[test]
fn retains_typed_evidence_forwarding_owner_identity() {
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
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let [forwarding] = typed.evidence_forwardings.as_slice() else {
        panic!("one typed evidence forwarding expected");
    };
    assert!(forwarding.machine_symbol.is_valid());
    assert!(forwarding.state_symbol.is_valid());
    assert_eq!(forwarding.target.as_str(), "output_proof");
    assert_eq!(forwarding.source.as_str(), "input_proof");
    assert_eq!(forwarding.source_conformance, None);
    assert_eq!(typed.snapshot().evidence_forwardings.len(), 1);
}

#[test]
fn copies_exact_literal_and_case_membership_symbols_into_typed_tables() {
    let source = r#"
        data Token {
            value: u32;
            case Issued(code: u32);
        }
        machine path() -> u32 { Token::Issued::code }
        machine record() -> Token { Token { value: 1 } }
        machine issue() -> Token { Token::Issued { code: 2 } }
        machine is_issued(token: Token) -> bool { token in Token::Issued }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize exact selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse exact selections");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve exact selections");
    let expected_type = resolved
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let expected_case = resolved
        .data_members(resolved.data_definitions[0].members)
        .iter()
        .find_map(|member| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
            _ => None,
        })
        .expect("Issued case");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type exact selections");

    let authored_path = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Name(path)
                if typed.expression_table.name_path_members(path.members).len() == 3 =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("three-segment typed path");
    let authored_path_symbols = typed
        .expression_table
        .name_path_member_symbols(authored_path.member_symbols);
    assert_eq!(authored_path_symbols.len(), 3);
    assert!(authored_path_symbols.iter().all(|symbol| symbol.is_valid()));

    let literals = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => Some(literal),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(literals.len(), 2);
    for literal in literals {
        assert_eq!(literal.type_symbol, expected_type);
        assert_eq!(literal.case_name.is_some(), literal.case_symbol.is_some());
        assert!(
            typed
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .all(|field| field.field_symbol.is_valid())
        );
    }

    let case_path = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Name(path)
                if typed
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(["Token", "Issued"]) =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("lowered exact case path");
    assert_eq!(
        typed
            .expression_table
            .name_path_member_symbols(case_path.member_symbols),
        [expected_type, expected_case]
    );
}

#[test]
fn typed_lowering_does_not_replace_the_authored_struct_selection_ledger() {
    let source = "data Item { value: u32; } machine make() -> Item { Item { value: 1 } }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize literal");
    let syntax = parse_syntax_trees(&tokens).expect("parse literal");
    let mut resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve literal");
    let literal = resolved
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(
                expression,
                symbol_resolved_trees::expression::ExpressionNode::StructLiteral(_)
            )
            .then_some(handle)
        })
        .expect("struct literal");
    let symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) =
        resolved.tables.bodies.expressions.expression_mut(literal)
    else {
        unreachable!();
    };
    literal.type_symbol = symbols::SymbolHandle::invalid();

    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("typed lowering is not the authored package-admission gate");
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StructLiteralType
            && matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
}

#[test]
fn elaborates_omitted_erased_field_with_unique_nullary_constructor() {
    let source = r#"
        data Evidence {
            case Only;
            case WithPayload(value: i32);
        }
        data Certified {
            value: i32;
            proof [erased]: Evidence;
        }
        machine certify() -> Certified {
            Certified { value: 7 }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let evidence = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Evidence")
        .expect("Evidence definition");
    let only = typed
        .data_members(evidence)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.name.as_str() == "Only" => {
                Some(variant)
            }
            _ => None,
        })
        .expect("Only variant");
    let literal = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == "Certified" =>
            {
                Some(literal)
            }
            _ => None,
        })
        .expect("Certified literal");
    let fields = typed.expression_table.struct_fields(literal.fields);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["value", "proof"]
    );
    let proof = &fields[1];
    let typed_trees::expression::ExpressionNode::Name(path) =
        typed.expression_table.expression(proof.value)
    else {
        panic!("omitted proof should elaborate to a semantic name term");
    };
    assert_eq!(
        typed
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Evidence", "Only"]
    );
    assert_eq!(path.head_symbol, evidence.symbol);
    assert_eq!(path.symbol, only.symbol);
    assert_eq!(
        typed
            .expression_table
            .name_path_member_symbols(path.member_symbols),
        [evidence.symbol, only.symbol]
    );
}

#[test]
fn preserves_field_relevance_through_resolved_and_typed_trees() {
    let source = r#"
        data Certified {
            value: i32;
            proof [erased]: i32;
            case Wrapped(witness [erased]: i32);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax_trees)).expect("resolve");

    let resolved_data = resolved
        .data_definitions
        .iter()
        .next()
        .expect("resolved data");
    let resolved_members = resolved.data_members(resolved_data.members);
    let symbol_resolved_trees::data::DataMember::Field(resolved_value) = &resolved_members[0]
    else {
        panic!("resolved value field");
    };
    let symbol_resolved_trees::data::DataMember::Field(resolved_proof) = &resolved_members[1]
    else {
        panic!("resolved proof field");
    };
    let symbol_resolved_trees::data::DataMember::Variant(resolved_wrapped) = &resolved_members[2]
    else {
        panic!("resolved wrapped case");
    };
    let [resolved_witness] = resolved.data_payload_fields(resolved_wrapped.payload) else {
        panic!("one resolved payload field");
    };
    assert_eq!(
        resolved_value.relevance,
        language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        resolved_proof.relevance,
        language_core::BindingRelevance::Erased
    );
    assert_eq!(
        resolved_witness.relevance,
        language_core::BindingRelevance::Erased
    );
    let resolved_snapshot = resolved.snapshot_json().expect("resolved snapshot");
    assert!(resolved_snapshot.contains("\"relevance\":\"relevant\""));
    assert!(resolved_snapshot.contains("\"relevance\":\"erased\""));

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let typed_data = typed.data_definitions().first().expect("typed data");
    let typed_members = typed.data_members(typed_data);
    let typed_trees::data::DataMember::Field(typed_value) = &typed_members[0] else {
        panic!("typed value field");
    };
    let typed_trees::data::DataMember::Field(typed_proof) = &typed_members[1] else {
        panic!("typed proof field");
    };
    let typed_trees::data::DataMember::Variant(typed_wrapped) = &typed_members[2] else {
        panic!("typed wrapped case");
    };
    let [typed_witness] = typed.data_payload_fields(typed_wrapped) else {
        panic!("one typed payload field");
    };
    assert_eq!(
        typed_value.relevance,
        language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        typed_proof.relevance,
        language_core::BindingRelevance::Erased
    );
    assert_eq!(
        typed_witness.relevance,
        language_core::BindingRelevance::Erased
    );
    let typed_snapshot = typed.snapshot_json().expect("typed snapshot");
    assert!(typed_snapshot.contains("\"relevance\":\"relevant\""));
    assert!(typed_snapshot.contains("\"relevance\":\"erased\""));
}

#[test]
fn retains_subjectless_conformance_and_exact_typed_rows() {
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
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let [conformance] = typed.conformances() else {
        panic!("one typed conformance");
    };
    assert!(matches!(
        conformance.subject,
        typed_trees::trait_definition::ConformanceSubject::Subjectless
    ));
    assert!(conformance.symbol.is_valid());
    assert_eq!(
        conformance.alias.as_ref().map(|name| name.as_str()),
        Some("ConcreteEvidence")
    );
    let Some(rows) = typed.closed_conformance_rows(conformance) else {
        panic!("closed rows retained");
    };
    let [row] = rows else {
        panic!("one exact typed row");
    };
    assert!(row.declaring_trait.is_valid());
    assert!(row.requirement.is_valid());
    assert!(row.realization_machine.is_valid());
    assert!(row.realization_state.is_valid());
}

#[test]
fn types_nested_index_hoists_from_explicit_local_collections() {
    let source = r#"
        data Main {}
        machine Main::main(
            &mut self,
            i: u64 [0..=1],
            j: u64 [0..=2]
        ) {
            let g: [[i32 in Wrapping; 3]; 2] = [[1, 2, 3], [4, 5, 6]];
            g[i][j] = g[i][j] + 1;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let machine = &typed.machines()[0];
    let state = &typed.machine_states(machine)[0];
    let hoisted = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__hoist_") =>
            {
                Some(local)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        hoisted.len(),
        1,
        "the indexed RHS should have one value hoist"
    );
    let typed_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = typed
        .type_reference_table
        .type_reference(hoisted[0].type_reference)
    else {
        panic!("the hoist must inherit the authored constrained element type");
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*base_type),
        typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "i32"
    ));
    assert!(matches!(
        typed.type_reference_table.constraints(*constraints),
        [typed_trees::types::TypeConstraintNode::ArithmeticDomain(
            numerics::arithmetic::ArithmeticDomain::Wrapping
        )]
    ));
}

#[test]
fn generic_proposition_applications_remain_proof_facts_when_typed() {
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
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let trait_definition = &typed.traits()[0];
    let [_, relation] = typed.trait_type_parameters(trait_definition) else {
        panic!("trait should retain a proposition parameter");
    };
    assert!(matches!(
        relation.kind,
        typed_trees::data::TypeParameterKind::Proposition { .. }
    ));
    let [signature] = typed.trait_machine_signatures(trait_definition) else {
        panic!("trait should retain one proof signature");
    };
    let [contract] = typed.state_signature_contracts(signature) else {
        panic!("proof signature should retain one ensures contract");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("Relation(value, value) should be a proposition proof fact");
    };
    assert_eq!(application.proposition, relation.symbol);
    assert!(
        typed
            .normalize_proposition_application(application)
            .is_some()
    );
}

#[test]
fn proposition_declarations_and_fact_applications_remain_distinct_when_typed() {
    let source = r#"
        pub proposition related(left: i32, right: i32);

        machine preserve(left: i32, right: i32)
        requires related(left, right)
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    assert_eq!(typed.propositions().len(), 1);
    assert!(typed.propositions()[0].is_public);
    assert!(
        typed
            .snapshot_json()
            .expect("typed proposition snapshot")
            .contains("\"is_public\":true")
    );
    assert!(matches!(
        typed.propositions()[0].body,
        typed_trees::proposition::PropositionBody::Primitive
    ));
    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its requires contract");
    };
    let [fact] = typed.proof_facts.span_or_empty(contract.facts) else {
        panic!("requires should retain one proposition fact");
    };
    let typed_trees::domain::ProofFact::Proposition(application) = fact else {
        panic!("proposition application must not become a Boolean expression");
    };
    assert_eq!(application.proposition, typed.propositions()[0].symbol);
    assert_eq!(
        typed
            .expression_table
            .expression_handles(application.arguments)
            .len(),
        2
    );
}

#[test]
fn const_declaration_visibility_survives_typed_lowering_and_snapshots() {
    let source = r#"
        pub const PUBLIC_LIMIT: u64 = 4;
        const Limits::PRIVATE_LIMIT: u64 = 2;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const visibility");
    let syntax = parse_syntax_trees(&tokens).expect("parse const visibility");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve const visibility");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type const visibility");

    assert_eq!(typed.const_declarations().len(), 2);
    assert!(typed.const_declarations()[0].is_public);
    assert!(!typed.const_declarations()[1].is_public);
    let snapshot = typed.snapshot_json().expect("typed const snapshot");
    assert!(snapshot.contains("\"name\":\"PUBLIC_LIMIT\""));
    assert!(snapshot.contains("\"is_public\":true"));
}

#[test]
fn proposition_type_and_const_arguments_retain_categories_and_identity() {
    let source = r#"
        proposition indexed<T, const N: i32>();
        proposition forwarded<T, const N: i32>() = indexed<T, N>();

        machine use_selected()
        requires forwarded<i32, 7>()
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its proposition requirement");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("requires should retain the proposition application");
    };
    assert!(matches!(
        application.binder_arguments[0].kind,
        typed_trees::proposition::PropositionBinderArgumentKind::Type
    ));
    assert!(matches!(
        application.binder_arguments[1].kind,
        typed_trees::proposition::PropositionBinderArgumentKind::Const
    ));
    assert_eq!(application.binder_arguments[0].display_name(), "i32");
    assert_eq!(application.binder_arguments[1].display_name(), "7");
    let normalized = typed
        .normalize_proposition_application(application)
        .expect("transparent application should normalize");
    assert_eq!(
        normalized.identity_label(),
        "proposition:fact:indexed<i32,7>()"
    );
}

#[test]
fn proposition_static_arguments_reject_wrong_binder_categories_and_const_types() {
    for (source, expected) in [
        (
            r#"
                proposition indexed<T, const N: i32>();
                machine wrong() requires indexed<7, i32>() {}
            "#,
            "type binder `T` received a const literal",
        ),
        (
            r#"
                proposition indexed<const N: bool>();
                machine wrong() requires indexed<1>() {}
            "#,
            "cannot receive integer literal `1` as `bool`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved_program =
            resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
        let diagnostic = lower_symbol_resolved_trees(&resolved_program)
            .expect_err("wrong proposition binder category must reject");
        assert!(
            diagnostic.message.contains(expected),
            "unexpected diagnostic: {}",
            diagnostic.message
        );
    }
}

#[test]
fn proposition_type_and_const_arguments_forward_through_machine_binders() {
    let source = r#"
        proposition indexed<T, const N: i32>();

        machine forward<T, const N: i32>()
        requires indexed<T, N>()
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program =
        resolve(ResolutionRequest::new(&syntax_trees)).expect("resolution should succeed");
    let [resolved_contract] =
        resolved_program.machine_contracts(&resolved_program.roots.machines[0])
    else {
        panic!("machine should retain one resolved contract");
    };
    let [symbol_resolved_trees::domain::ProofFact::Expression(resolved_application)] =
        resolved_program.proof_facts(resolved_contract.facts)
    else {
        panic!("requires should retain one resolved expression fact");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(resolved_call) = resolved_program
        .tables
        .bodies
        .expressions
        .expression(*resolved_application)
    else {
        panic!("resolved fact should remain the indexed application");
    };
    assert!(
        resolved_call.machine_arguments[0].symbol.is_valid(),
        "forwarded type argument should resolve"
    );
    assert!(
        resolved_call.machine_arguments[1].symbol.is_valid(),
        "forwarded const argument should resolve"
    );
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let machine = &typed.machines()[0];
    let parameters = typed.machine_type_parameters(machine);
    let [contract] = typed.machine_contracts(machine) else {
        panic!("generic machine should retain its proposition requirement");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("requires should retain the proposition application");
    };
    assert_eq!(application.binder_arguments[0].symbol, parameters[0].symbol);
    assert_eq!(application.binder_arguments[1].symbol, parameters[1].symbol);
    assert_eq!(
        typed
            .normalize_proposition_application(application)
            .expect("generic application should normalize")
            .identity_label(),
        "proposition:fact:indexed<T,N>()"
    );
}

#[test]
fn retains_exact_sealed_quotient_operation_request_without_admitting_it() {
    let source = r#"
        data Representative { value: i32; }

        machine representative(value: Representative) -> Representative { value }
        machine representative_respects(left: Representative, right: Representative) {}
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<representative, representative_respects>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let request = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.quotient_operation.as_ref(),
            _ => None,
        })
        .expect("sealed quotient request");

    assert_eq!(
        request.kind,
        typed_trees::expression::QuotientOperationKind::Lift
    );
    assert_eq!(
        typed.symbols.name(
            typed
                .symbols
                .get(request.representative_operation.symbol)
                .parent,
        ),
        "representative"
    );
    assert_eq!(
        typed
            .symbols
            .get(request.representative_operation.symbol)
            .kind,
        symbols::SymbolKind::State
    );
    assert_eq!(
        typed.symbols.name(
            typed
                .symbols
                .get(request.theorem_evidence[0].application.symbol)
                .parent,
        ),
        "representative_respects"
    );
    assert_eq!(
        typed
            .symbols
            .get(request.theorem_evidence[0].application.symbol)
            .kind,
        symbols::SymbolKind::State
    );
}
