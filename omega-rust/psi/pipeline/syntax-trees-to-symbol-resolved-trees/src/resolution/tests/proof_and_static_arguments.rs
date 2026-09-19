use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn transparent_proposition_zero_value_target_resolves_in_its_binder_scope() {
    let source = r#"
        data Optional<Element> { case #0 None; }
        proposition zero_reflexive<Item>() =
            zero_value<Optional<Item>>() == zero_value<Optional<Item>>();
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let optional = program.data_definitions.first().expect("Optional data");
    let proposition = program.propositions.first().expect("zero proposition");
    let [binder] = program
        .tables
        .declarations
        .proposition_binders
        .span_or_empty(proposition.binders)
    else {
        panic!("one proposition type binder")
    };

    let targets = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            symbol_resolved_trees::expression::ExpressionNode::ZeroValue(target) => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    for target in targets {
        let symbol_resolved_trees::types::TypeReference::Generic(target) =
            program.child_type_reference(target)
        else {
            panic!("zero-value target should remain a generic type")
        };
        assert_eq!(target.base_symbol, optional.symbol);
        let [argument] = program.child_type_references(target.arguments) else {
            panic!("one exact zero-value type argument")
        };
        assert!(matches!(
            argument,
            symbol_resolved_trees::types::TypeReference::Named { symbol, name }
                if *symbol == binder.symbol && name.as_str() == "Item"
        ));
    }
}

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
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve exact selections");
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
fn outcome_specific_ensures_normalizes_only_against_declared_result_sum() {
    let source = r#"
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        ensures Outcome::Success -> { true; }
        { Outcome::Success }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize guarded guarantee");
    let syntax = parse_syntax_trees(&tokens).expect("parse guarded guarantee");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve guarded guarantee");
    let outcome = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Outcome")
        .expect("Outcome data");
    let success = program
        .data_members(outcome.members)
        .iter()
        .find_map(|member| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Success" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Success case");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose machine");
    let [contract] = program.machine_contracts(machine) else {
        panic!("one guarded guarantee row")
    };
    assert_eq!(
        contract.kind,
        symbol_resolved_trees::signature::SignatureContractKind::EnsuresForResultCase {
            result_data: outcome.symbol,
            result_case: success.symbol,
        }
    );
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
        syntax: &syntax,
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
            resolve(ResolutionRequest::new(&syntax)).expect_err("guard resolution must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?}, got: {diagnostics:?}"
        );
    }
}

#[test]
fn captures_resolved_calls_and_late_checked_operators_in_private_bodies() {
    let source = r#"
        machine identity(value: u32) -> u32 { value }
        machine calculate(value: u32) -> u32 { identity(value) + 1 }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize authored selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse authored selections");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve authored selections");
    let selections = program.authored_declaration_selections();
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Operator
            && selection.target()
                == symbol_resolved_trees::AuthoredDeclarationSelectionTarget::LateBound(
                    symbol_resolved_trees::AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                )
    }));
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
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve guard hoist");
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
fn const_specialization_copies_share_the_authored_member_declaration() {
    let source = r#"
        data FixedBuffer<const N: u64> { items: [i32; N]; }
        machine FixedBuffer::first(&self) -> i32 { self.items[0] }
        data Main { small: FixedBuffer<2>; large: FixedBuffer<4>; }
        machine Main::small(&self) -> i32 { self.small.first() }
        machine Main::large(&self) -> i32 { self.large.first() }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const copies");
    let syntax = parse_syntax_trees(&tokens).expect("parse const copies");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve const copies");
    let items_start = source.find("self.items").expect("member source") + "self.".len();
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == symbol_resolved_trees::AuthoredDeclarationSelectionKind::MemberAccess
                && selection.source_span().span.start == items_start
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one source member must own every specialized copy: {selections:?}");
    };
    let symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
        selection.target()
    else {
        panic!("specialized member selection must resolve exactly")
    };
    assert_eq!(
        program.symbols.display_path(target.selected_symbol(), "::"),
        "FixedBuffer::first::items"
    );
}

#[test]
fn distinguishes_public_contract_expressions_from_public_machine_bodies() {
    let source = r#"
        machine helper() -> bool { true }
        pub machine api() -> bool
        requires helper()
        {
            helper()
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize expression exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse expression exposure");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve expression exposure");
    let call_exposures = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
        })
        .map(|selection| selection.exposure())
        .collect::<Vec<_>>();

    assert!(
        call_exposures.contains(
            &symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
        )
    );
    assert!(call_exposures.contains(
        &symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
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
        resolve(ResolutionRequest::new(&syntax)).expect("resolve expression type exposure");
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
fn qualification_cast_domains_retain_exact_expression_custody() {
    let source = r#"
        domain u16::Tagged;
        pub machine api(value: u8)
        requires (value as u16 in Tagged) == 1
        { }
        machine helper(value: u8)
        requires (value as u16 in Tagged) == 1
        { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize qualification casts");
    let syntax = parse_syntax_trees(&tokens).expect("parse qualification casts");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve qualification casts");

    let casts = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            symbol_resolved_trees::expression::ExpressionNode::Cast(cast)
                if !cast.semantic_domain.is_empty() =>
            {
                Some(expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(casts.len(), 2);

    let mut exposures = casts
        .into_iter()
        .map(|cast| {
            let occurrences = program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(cast)
                .collect::<Vec<_>>();
            let [occurrence] = occurrences.as_slice() else {
                panic!("each qualification cast must retain one exact authored selection")
            };
            let selection = program
                .authored_declaration_selections()
                .get(*occurrence)
                .expect("qualification-cast occurrence must rejoin its selection");
            assert_eq!(
                selection.kind(),
                symbol_resolved_trees::AuthoredDeclarationSelectionKind::DomainMembership
            );
            assert_eq!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::LateBound(
                    symbol_resolved_trees::AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                )
            );
            let source_span = selection.source_span();
            assert!(source_span.span.start < source_span.span.end);
            assert_eq!(source_span.span.end - source_span.span.start, "Tagged".len());
            selection.exposure()
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
        resolve(ResolutionRequest::new(&syntax)).expect("resolve public unary proposition");
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
fn retains_exact_establishment_route_declarations_with_domain_exposure() {
    let source = r#"
        data Ticket { }
        pub trait Issues {
            machine issue() -> Ticket in Ticket::Issued;
            machine hide() -> Ticket in Ticket::Internal;
        }
        pub domain Ticket::Issued
        established by Issues::issue, Issues::issue;
        domain Ticket::Internal
        established by Issues::hide;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize establishment selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse establishment selections");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve establishment selections");
    let issues = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Issues")
        .expect("Issues trait");
    let requirements = program.trait_machine_signatures(issues.machines);
    let issue = requirements
        .iter()
        .find(|requirement| requirement.name.as_str() == "issue")
        .expect("issue requirement");
    let hide = requirements
        .iter()
        .find(|requirement| requirement.name.as_str() == "hide")
        .expect("hide requirement");
    let rows = program
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| match selection.target() {
            symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                if [issues.symbol, issue.symbol, hide.symbol]
                    .contains(&target.selected_symbol()) =>
            {
                Some((
                    selection.kind(),
                    selection.exposure(),
                    target.selected_symbol(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let issued = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Ticket::Issued")
        .expect("Issued domain");
    assert_eq!(
        issued.establishment_routes.len(),
        1,
        "semantic alternatives should deduplicate"
    );
    assert_eq!(rows.len(), 6, "rows={rows:#?}");
    assert_eq!(
        rows.iter()
            .filter(|(kind, exposure, symbol)| {
                *kind
                    == symbol_resolved_trees::AuthoredDeclarationSelectionKind::DomainIssuerAuthorization
                    && *exposure
                        == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
                    && *symbol == issues.symbol
            })
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|(kind, exposure, symbol)| {
                *kind
                    == symbol_resolved_trees::AuthoredDeclarationSelectionKind::DomainIssuerAuthorization
                    && *exposure
                        == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
                    && *symbol == issue.symbol
            })
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|(_, exposure, _)| {
                *exposure
                    == symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
            })
            .count(),
        2
    );
}

#[test]
fn distinguishes_boundary_contract_expressions_from_boundary_adapter_bodies() {
    let source = r#"
        machine helper() -> bool { true }
        boundary machine api() -> bool
        requires helper()
        {
            helper()
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize boundary expression exposure");
    let syntax = parse_syntax_trees(&tokens).expect("parse boundary expression exposure");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve boundary expression exposure");
    let call_exposures = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
        })
        .map(|selection| selection.exposure())
        .collect::<Vec<_>>();

    assert!(
        call_exposures.contains(
            &symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
        )
    );
    assert!(call_exposures.contains(
        &symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
}

#[test]
fn captures_expression_static_type_and_machine_arguments() {
    let source = r#"
        data Card { }
        machine chosen(value: &Card) -> bool { true }
        machine apply<T, machine Selected>(value: &T) -> bool
        where machine Selected(value: &T) -> bool
        {
            Selected(value)
        }
        machine caller(value: &Card) -> bool {
            apply<Card, chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize static arguments");
    let syntax = parse_syntax_trees(&tokens).expect("parse static arguments");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve static arguments");
    let selected_kinds = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
        })
        .filter_map(|selection| match selection.target() {
            symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) => {
                Some(program.symbols.get(target.selected_symbol()).kind)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(selected_kinds.contains(&symbols::SymbolKind::Data));
    assert!(selected_kinds.contains(&symbols::SymbolKind::State));
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
        syntax: &syntax,
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
fn resolves_named_const_static_arguments_with_exact_authored_custody() {
    let source = r#"
        pub const Limits::LIMIT: u64 = 7;
        pub machine constant<const Value: u64>() -> u64 { 0 }
        boundary machine trusted_constant() -> u64
        ensures result == constant<Limits::LIMIT>();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize named const static argument");
    let syntax = parse_syntax_trees(&tokens).expect("parse named const static argument");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve named const static argument");
    let selections = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.kind()
                == symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
                && matches!(
                    selection.target(),
                    symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if program.symbols.get(target.selected_symbol()).kind
                            == symbols::SymbolKind::Const
                )
        })
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        panic!("one exact named const static-argument selection: {selections:#?}")
    };
    assert_eq!(
        selection.exposure(),
        symbol_resolved_trees::AuthoredDeclarationSelectionExposure::PublicInterface
    );
}

#[test]
fn captures_statement_calls_and_their_explicit_conformance_arguments() {
    let source = r#"
        trait Marker { }
        data Good { }
        GoodMarker: Good satisfies Marker;

        machine effect() { }
        machine consume<Element, Evidence: Element satisfies Marker>(value: Element) { }
        machine caller(value: Good) {
            effect();
            consume<Good, GoodMarker>(value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize statement selections");
    let syntax = parse_syntax_trees(&tokens).expect("parse statement selections");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve statement selections");
    let selected = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "GoodMarker")
        })
        .expect("GoodMarker conformance")
        .symbol;
    let selected_type = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Good")
        .expect("Good data")
        .symbol;
    let selections = program.authored_declaration_selections();
    let statement_calls = program
        .machines
        .iter()
        .flat_map(|machine| program.machine_state_handles(machine.states))
        .flat_map(|state| {
            program
                .tables
                .bodies
                .statements
                .statements(program.machine_state(*state).statement_nodes)
        })
        .filter_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(statement_calls.len(), 2, "calls={statement_calls:#?}");
    assert!(
        statement_calls.iter().all(|call| {
            call.target.is_source_backed()
                && call.operational_acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::Source
        }),
        "calls={statement_calls:#?}"
    );

    assert!(
        selections.iter().any(|selection| {
            selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Call
                && matches!(
                    selection.target(),
                    symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
                )
        }),
        "selections={selections:#?}"
    );
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::StaticArgument
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == selected_type
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == symbol_resolved_trees::AuthoredDeclarationSelectionKind::Conformance
            && matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == selected
            )
    }));
}

#[test]
fn substituted_const_retains_authored_declaration_selection_custody() {
    let source = r#"
        const ROOT_SIZE: u64 = 4;
        machine selected_size() -> u64 { ROOT_SIZE }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse const selection");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve const selection");
    let selection = program
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            matches!(
                selection.target(),
                symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if program.symbols.get(target.selected_symbol()).kind
                        == symbols::SymbolKind::Const
            )
        })
        .expect("retained const selection");
    let occurrence = program
        .authored_declaration_selections()
        .iter()
        .position(|candidate| candidate == selection)
        .expect("const selection ordinal");

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
                .any(|attached| attached.ordinal() == occurrence as u64))
    );
}

#[test]
fn const_substitution_obeys_current_activation_resolution_strata() {
    let base_source = r#"
        const Limits::VALUE: u64 = 1;
        const Limits::BASE_ONLY: u64 = 3;
        machine authored_value() -> u64 { Limits::VALUE }
    "#;
    let extension_source = r#"
        const Limits::VALUE: u64 = 2;
        const Generated::ONLY: u64 = 4;
        machine extension_reads_base() -> u64 { Limits::BASE_ONLY }
    "#;
    let second_extension_source =
        "machine second_extension_reads_first() -> u64 { Generated::ONLY }";
    let mut sources = SourceMap::default();
    let base_source_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let second_extension_source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated-second.omg"),
            second_extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;

    // Deliberately put the extension first: source order must not let its
    // same-named const retarget the authored occurrence.
    let extension_tokens = Lexer::new(extension_source)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_source_id, &extension_tokens)
        .expect("parse extension");
    let base_tokens = Lexer::new(base_source).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_source_id, &base_tokens).expect("parse base");
    syntax.extend_from(&base_syntax);
    let second_extension_tokens = Lexer::new(second_extension_source)
        .tokenize()
        .expect("tokenize second extension");
    let second_extension_syntax =
        parse_syntax_trees_with_id(second_extension_source_id, &second_extension_tokens)
            .expect("parse second extension");
    syntax.extend_from(&second_extension_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("cross-stratum duplicate consts remain separate");
    let const_targets = program
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            let symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
                selection.target()
            else {
                return None;
            };
            (program.symbols.get(target.selected_symbol()).kind == symbols::SymbolKind::Const).then(
                || {
                    (
                        selection.source_span().source_id,
                        program
                            .symbols
                            .symbol_source_span(target.selected_symbol())
                            .expect("source-backed const")
                            .source_id,
                    )
                },
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(const_targets.len(), 3, "const targets={const_targets:?}");
    assert!(const_targets.contains(&(base_source_id, base_source_id)));
    assert!(const_targets.contains(&(extension_source_id, base_source_id)));
    assert!(const_targets.contains(&(second_extension_source_id, extension_source_id)));
    assert!(!const_targets.contains(&(base_source_id, extension_source_id)));
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
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve abs desugar custody");
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
