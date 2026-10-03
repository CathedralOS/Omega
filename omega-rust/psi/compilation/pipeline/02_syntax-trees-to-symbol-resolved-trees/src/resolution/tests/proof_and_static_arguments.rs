use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn retains_exact_expression_selection_symbols() {
    let source = r#"
        data Token {
            value: u32;
            case Issued(code: u32);
        }
        machine path() -> u32 { Token::Issued::code }
        machine record() -> Token { Token { value: 1 } }
        machine issue() -> Token { Token::Issued { value: 1, code: 2 } }
        machine is_issued(token: Token) -> bool { token in Token::Issued }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize exact selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse exact selections");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve exact selections");
    let expressions = &program.tables.bodies.expressions;

    let path = expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Name(path)
                if expressions.name_path_members(path.members).len() == 3 =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("three-segment case payload path");
    let path_symbols = expressions.name_path_member_symbols(path.member_symbols);
    assert_eq!(path_symbols.len(), 3);
    assert!(path_symbols.iter().all(|symbol| symbol.is_valid()));
    assert_eq!(
        program.symbols.get(path_symbols[0]).kind,
        symbols::SymbolKind::Data
    );
    assert_eq!(
        program.symbols.get(path_symbols[1]).kind,
        symbols::SymbolKind::Variant
    );
    assert_eq!(
        program.symbols.get(path_symbols[2]).kind,
        symbols::SymbolKind::Field
    );

    let literals = expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) => {
                Some(literal)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(literals.len(), 2);
    for literal in literals {
        assert!(literal.type_symbol.is_valid());
        assert_eq!(
            literal.case_name.is_some(),
            literal.case_symbol.is_some_and(|symbol| symbol.is_valid())
        );
        assert!(
            expressions
                .struct_fields(literal.fields)
                .iter()
                .all(|field| field.field_symbol.is_valid())
        );
    }

    let membership = expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
                Some(membership)
            }
            _ => None,
        })
        .expect("case membership");
    assert!(!membership.domain_symbol.is_valid());
    assert!(membership.case_type_symbol.is_valid());
    assert!(membership.case_symbol.is_valid());
    assert_eq!(
        program.symbols.get(membership.case_type_symbol).kind,
        symbols::SymbolKind::Data
    );
    assert_eq!(
        program.symbols.get(membership.case_symbol).kind,
        symbols::SymbolKind::Variant
    );

    let selections = program.authored_declaration_selections();
    assert!(!selections.is_empty());
    assert!(selections.iter().all(|selection| {
        selection.exposure()
            == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    }));
    for required_kind in [
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticPathSegment,
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralType,
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralCase,
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::StructLiteralField,
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::CaseReference,
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::CaseMembership,
    ] {
        assert!(
            selections
                .iter()
                .any(|selection| selection.kind() == required_kind),
            "missing authored selection kind {required_kind:?}"
        );
    }
    assert!(expressions.iter_expressions().any(|(expression, _)| {
        expressions
            .authored_selection_occurrences(expression)
            .next()
            .is_some()
    }));
}

#[test]
fn authored_outcome_specific_contract_uses_the_exact_base_result_sum() {
    let base = r#"
        data Outcome { case Success; }
        machine choose() -> Outcome
        ensures Outcome::Success -> { true; }
        { Outcome::Success }
    "#;
    let extension = "data Outcome { case Success; case GeneratedOnly; }";
    let mut sources = SourceMap::default();
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_id = sources
        .add(PathBuf::from("base.omg"), base.to_owned())
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("authored outcome contract must retain the base result sum");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose machine");
    let [contract] = program.machine_contracts(machine) else {
        panic!("one outcome-specific contract")
    };
    let symbol_resolved_trees::signature::SignatureContractKind::EnsuresForResultCase {
        result_data,
        result_case,
    } = contract.kind
    else {
        panic!("outcome-specific contract kind")
    };
    for symbol in [result_data, result_case] {
        assert_eq!(
            program
                .symbols
                .symbol_provenance_source_span(symbol)
                .expect("source-backed result declaration")
                .source_id,
            base_id
        );
    }
}

#[test]
fn outcome_specific_ensures_rejects_non_sum_and_foreign_cases() {
    for (source, expected) in [
        (
            "data Record {} machine choose() -> Record ensures Record::Success -> { true; } { Record {} }",
            "requires a sum result",
        ),
        (
            "data Outcome { case Success; } machine choose() -> Outcome ensures Outcome::Missing -> { true; } { Outcome::Success }",
            "unknown case `Missing`",
        ),
        (
            "data Outcome { case Success; } data Other { case Success; } machine choose() -> Outcome ensures Other::Success -> { true; } { Outcome::Success }",
            "does not belong to declared result sum `Outcome`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize invalid guarded guarantee");
        let syntax = parse_syntax_trees(&tokens).expect("parse invalid guarded guarantee");
        let diagnostics =
            resolve(ResolutionRequest::new(syntax)).expect_err("guard resolution must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?}, got: {diagnostics:?}"
        );
    }
}

#[test]
fn guard_hoist_copies_share_one_authored_call_occurrence() {
    let source = r#"
        data Main { }
        machine Main::value(&mut self) -> bool { true }
        machine Main::done(&mut self) { }
        machine Main::run(&mut self) {
            let seed: bool = true;
            transition self.value() == seed {
                true -> done()
                false -> done()
            }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize guard hoist");
    let syntax = parse_syntax_trees(&tokens).expect("parse guard hoist");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve guard hoist");
    let target_start = source
        .find("self.value() == seed")
        .expect("guard call source")
        + "self.".len();
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
                && selection.source_span().span.start == target_start
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one authored occurrence must own every compiler copy: {selections:?}");
    };
    assert!(matches!(
        selection.target(),
        symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
    ));
    assert!(
        program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .any(|(expression, _)| program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .any(|occurrence| occurrence == selection.occurrence_id()))
    );
}

#[test]
fn retains_authored_expression_exposure_for_embedded_type_lowering() {
    let source = r#"
        data Marker {}
        pub proposition public_zero() =
            zero_value<Marker>() == zero_value<Marker>();
        proposition private_zero() =
            zero_value<Marker>() == zero_value<Marker>();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize expression type exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse expression type exposure");
    let program =
        resolve(ResolutionRequest::new(syntax)).expect("resolve expression type exposure");
    let mut exposures = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter(|&(_, node)| {
            matches!(
                node,
                symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_)
            )
        })
        .map(|(expression, _)| {
            program
                .tables
                .bodies
                .expressions
                .authored_expression_exposure(expression)
                .expect("authored zero-value expression exposure")
        })
        .collect::<Vec<_>>();
    exposures.sort_by_key(|exposure| match exposure {
        symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation => 0,
        symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface => 1,
    });
    assert_eq!(
        exposures,
        [
            symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
            symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface,
        ]
    );
}

#[test]
fn retains_nested_unary_operator_custody_in_public_propositions() {
    let source = "pub proposition inverted(value: u8, expected: u8) = ~value == expected;";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize public unary proposition");
    let syntax = parse_syntax_trees(&tokens).expect("parse public unary proposition");
    let program =
        resolve(ResolutionRequest::new(syntax)).expect("resolve public unary proposition");
    let proposition = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "inverted")
        .expect("inverted proposition");
    let symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
        proposition.body
    else {
        panic!("inverted proposition must remain transparent")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Binary(binary) =
        program.tables.bodies.expressions.expression(proposition)
    else {
        panic!("inverted proposition must retain its binary root")
    };
    let unary = binary.left;
    assert!(matches!(
        program.tables.bodies.expressions.expression(unary),
        symbol_resolved_trees::expression::ExpressionNode::Unary(_)
    ));
    let occurrences = program
        .tables
        .bodies
        .expressions
        .authored_selection_occurrences(unary)
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        panic!("nested unary operator must retain one exact authored selection")
    };
    let selection = program
        .authored_declaration_selections()
        .get(*occurrence)
        .expect("nested unary occurrence must rejoin its selection");
    assert_eq!(
        selection.kind(),
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::Operator
    );
    assert_eq!(
        selection.exposure(),
        symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    );
}

#[test]
fn static_and_proof_static_arguments_obey_current_activation_resolution_strata() {
    let base = r#"
        trait Marker {}
        data Box<Element> {}
        data Item {}
        const Limits::VALUE: u64 = 1;
        Marked: Item satisfies Marker;
        machine Item::work() {}

        data BaseBox<Element> {}
        data BaseItem {}
        const BaseLimits::VALUE: u64 = 2;
        BaseMarked: BaseItem satisfies Marker;
        machine BaseItem::work() {}

        machine sink<A, B, C, D>() {}
        proposition prove<A, B, C, D>();
        machine base_call() {
            sink<Box<Item>, Limits::VALUE, Marked, Item::work>();
        }
        machine base_hidden_call() {
            sink<ExtensionBox<ExtensionItem>, ExtensionLimits::VALUE, ExtensionMarked, ExtensionItem::work>();
        }
        proposition base_proof() =
            prove<Box<Item>, Limits::VALUE, Marked, Item::work>();
        proposition base_hidden_proof() =
            prove<ExtensionBox<ExtensionItem>, ExtensionLimits::VALUE, ExtensionMarked, ExtensionItem::work>();
    "#;
    let extension = r#"
        data Box<Element> {}
        data Item {}
        const Limits::VALUE: u64 = 3;
        Marked: Item satisfies Marker;
        machine Item::work() {}

        data ExtensionBox<Element> {}
        data ExtensionItem {}
        const ExtensionLimits::VALUE: u64 = 4;
        ExtensionMarked: ExtensionItem satisfies Marker;
        machine ExtensionItem::work() {}

        machine extension_call() {
            sink<Box<Item>, Limits::VALUE, Marked, Item::work>();
        }
        machine extension_reads_base() {
            sink<BaseBox<BaseItem>, BaseLimits::VALUE, BaseMarked, BaseItem::work>();
        }
        proposition extension_proof() =
            prove<Box<Item>, Limits::VALUE, Marked, Item::work>();
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/extension.omg"),
            extension.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax =
        parse_syntax_trees_with_id(extension_id, &extension_tokens).expect("parse extension first");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    syntax.extend_from(
        &parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse authored base"),
    );
    let program = resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve extension-first static arguments");

    fn statement_arguments<'a>(
        program: &'a symbol_resolved_trees::SymbolResolvedTrees,
        machine_name: &str,
    ) -> &'a [symbol_resolved_trees::expression::StaticMachineArgument] {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("caller machine");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                symbol_resolved_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() == "sink" =>
                {
                    Some(call.machine_arguments.as_ref())
                }
                _ => None,
            })
            .expect("sink call")
    }

    fn proof_arguments<'a>(
        program: &'a symbol_resolved_trees::SymbolResolvedTrees,
        proposition_name: &str,
    ) -> &'a [symbol_resolved_trees::expression::StaticMachineArgument] {
        let proposition = program
            .propositions
            .iter()
            .find(|proposition| proposition.name.as_str() == proposition_name)
            .expect("transparent proposition");
        let symbol_resolved_trees::proposition::PropositionBody::Transparent { proposition } =
            proposition.body
        else {
            panic!("transparent proposition body")
        };
        let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
            program.tables.bodies.expressions.expression(proposition)
        else {
            panic!("proposition call")
        };
        &call.machine_arguments
    }

    fn assert_argument_sources(
        program: &symbol_resolved_trees::SymbolResolvedTrees,
        arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
        expected: source::SourceId,
    ) {
        assert_eq!(arguments.len(), 4);
        let nested = &arguments[0]
            .application
            .as_ref()
            .expect("nested data application")
            .arguments[0];
        for argument in arguments.iter().chain(std::iter::once(nested)) {
            assert!(
                argument.symbol.is_valid(),
                "argument={argument:#?}, spans={:?}",
                argument
                    .path
                    .iter()
                    .map(|name| name.source_span())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                program
                    .symbols
                    .symbol_provenance_source_span(argument.symbol)
                    .expect("selected static declaration provenance")
                    .source_id,
                expected,
                "argument={argument:#?}",
            );
        }
    }

    assert_argument_sources(
        &program,
        statement_arguments(&program, "base_call"),
        base_id,
    );
    assert_argument_sources(&program, proof_arguments(&program, "base_proof"), base_id);
    assert_argument_sources(
        &program,
        statement_arguments(&program, "extension_call"),
        extension_id,
    );
    assert_argument_sources(
        &program,
        proof_arguments(&program, "extension_proof"),
        extension_id,
    );
    assert_argument_sources(
        &program,
        statement_arguments(&program, "extension_reads_base"),
        base_id,
    );

    for arguments in [
        statement_arguments(&program, "base_hidden_call"),
        proof_arguments(&program, "base_hidden_proof"),
    ] {
        assert!(arguments.iter().all(|argument| !argument.symbol.is_valid()));
        let nested = &arguments[0]
            .application
            .as_ref()
            .expect("nested hidden data application")
            .arguments[0];
        assert!(!nested.symbol.is_valid());
    }
}

#[test]
fn abs_desugar_subtraction_retains_authored_operator_custody() {
    // `abs(x)` lowers to `max(x, 0 - x)`: the synthesized subtraction is a
    // boundary operator application once typed, so it must retain one authored
    // selection occurrence minted at the authored `abs` token. Otherwise the
    // package review projection cannot bind the realized `Float::subtract`.
    let source = r#"
        data Main { x: f64; }
        machine Main::main(&mut self) {
            let magnitude: f64 = abs(self.x);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize abs desugar custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse abs desugar custody");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve abs desugar custody");
    let table = &program.tables.bodies.expressions;
    let subtractions = table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            matches!(
                node,
                symbol_resolved_trees::expression::ExpressionNode::Binary(_)
            )
            .then_some(expression)
        })
        .collect::<Vec<_>>();
    let [subtraction] = subtractions.as_slice() else {
        panic!("abs desugar must produce exactly one synthesized subtraction")
    };
    let occurrences = table
        .authored_selection_occurrences(*subtraction)
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        panic!("synthesized subtraction must retain one authored selection")
    };
    let selection = program
        .authored_declaration_selections()
        .get(*occurrence)
        .expect("synthesized subtraction occurrence must rejoin its selection");
    assert_eq!(
        selection.kind(),
        symbol_resolved_trees::AuthoredDeclarationSelectionKind::Operator
    );
    assert_eq!(
        selection.exposure(),
        symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    );
    let abs_offset = source.find("abs(").expect("authored abs token");
    assert_eq!(selection.source_span().span.start, abs_offset);
    assert_eq!(selection.source_span().span.end, abs_offset + 3);
}

#[test]
fn contract_clause_calls_naming_no_declaration_skip_call_selection() {
    let source = r#"
        machine cited() -> bool { true }
        machine carries(before: &[u64], items: &[u64])
        requires cited() && Bag(items) == Bag(before)
        ensures Bag(items) == Bag(before)
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize uninterpreted contract calls");
    let syntax = parse_syntax_trees(&tokens).expect("parse uninterpreted contract calls");
    let program =
        resolve(ResolutionRequest::new(syntax)).expect("resolve uninterpreted contract calls");
    let call_spans = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
        })
        .map(|selection| selection.source_span())
        .collect::<Vec<_>>();
    assert!(
        call_spans
            .iter()
            .all(|span| { &source[span.span.start..span.span.end] != "Bag" }),
        "uninterpreted contract atoms name no declaration and carry no Call occurrence"
    );
    let cited_spans = call_spans
        .iter()
        .filter(|span| &source[span.span.start..span.span.end] == "cited")
        .collect::<Vec<_>>();
    assert!(
        !cited_spans.is_empty(),
        "a declared contract callee still records its Call occurrence"
    );
}
