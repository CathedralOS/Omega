use super::lower_symbol_resolved_trees;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn lower_source(source: &str) -> Result<psi_typed_trees::TypedTrees, psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    lower_symbol_resolved_trees(&resolved)
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
            psi_typed_trees::expression::ExpressionNode::ArrayLiteral(elements) => Some(*elements),
            _ => None,
        })
        .expect("the contextual string must become an ordinary array literal");
    let values = typed
        .expression_table
        .expression_handles(array)
        .iter()
        .map(
            |element| match typed.expression_table.expression(*element) {
                psi_typed_trees::expression::ExpressionNode::Integer(literal) => {
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
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve trait machine requirement parameter");
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
        psi_typed_trees::data::TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::RequirementIdentity
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve private callback slot");
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
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(*argument)
    else {
        panic!("typed requirement argument remains a named identity")
    };
    assert_eq!(name.as_str(), "WindowProcedure::call");
    assert_eq!(*symbol, requirement.symbol);
}

#[test]
fn retains_public_conformance_visibility_snapshot_and_header_selections() {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionTarget,
    };

    let source = "pub trait Ranked {} pub data Card {} pub PowerOrder: Card satisfies Ranked {}";
    let tokens = Lexer::new(source).tokenize().expect("tokenize conformance");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance");
    let resolved = lower_syntax_trees(&syntax).expect("resolve conformance");
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
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
            let psi_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) = expression
            else {
                return None;
            };
            Some(resolved.child_type_reference(cast.target_type).clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(cast_targets.len(), 2, "cast targets: {cast_targets:#?}");
    let cast_target_spans = cast_targets
        .iter()
        .filter_map(|target| {
            let psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } = target
            else {
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
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        pub data Token [copy] { value: u64; }
        pub operator < Token::less(left: Token, right: Token) -> bool;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve public data");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve public machine");
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
        psi_language_semantics::MachineSupplyMode::CheckedBody
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
        Some(&psi_language_semantics::ExternalBindingIdentity::Import {
            library: "a,b".to_owned(),
            symbol: "c".to_owned(),
        })
    );
}

#[test]
fn settles_satisfied_operator_to_its_exact_overload_symbol() {
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
        psi_typed_trees::operator::declaration_by_symbol(&typed, conformance.requirement_symbol)
            .expect("settled exact operator");
    assert_eq!(typed.display_type_reference(operator.return_type), "f32");
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::StaticPathSegment
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == operator.symbol)
    }));
}

#[test]
fn retains_exact_nominal_machine_parameter_identity_in_typed_trees() {
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
        panic!("Selected should be a machine parameter")
    };
    let psi_typed_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
    } = contract
    else {
        panic!("Selected should retain nominal identity")
    };
    let psi_typed_trees::data::MachineParameterContractView::Nominal {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name }
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
            psi_symbol_resolved_trees::statement::Statement::Call(call)
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
            psi_typed_trees::statement::StatementNode::Call(call)
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve exact selections");
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
            psi_symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
            _ => None,
        })
        .expect("Issued case");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type exact selections");

    let authored_path = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::Name(path)
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
            psi_typed_trees::expression::ExpressionNode::StructLiteral(literal) => Some(literal),
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
            psi_typed_trees::expression::ExpressionNode::Name(path)
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
    let mut resolved = lower_syntax_trees(&syntax).expect("resolve literal");
    let literal = resolved
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(
                expression,
                psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(_)
            )
            .then_some(handle)
        })
        .expect("struct literal");
    let psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) =
        resolved.tables.bodies.expressions.expression_mut(literal)
    else {
        unreachable!();
    };
    literal.type_symbol = psi_symbols::SymbolHandle::invalid();

    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("typed lowering is not the authored package-admission gate");
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StructLiteralType
            && matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(_)
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
            psi_typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Only" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Only variant");
    let literal = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::StructLiteral(literal)
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
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
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
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");

    let resolved_data = resolved
        .data_definitions
        .iter()
        .next()
        .expect("resolved data");
    let resolved_members = resolved.data_members(resolved_data.members);
    let psi_symbol_resolved_trees::data::DataMember::Field(resolved_value) = &resolved_members[0]
    else {
        panic!("resolved value field");
    };
    let psi_symbol_resolved_trees::data::DataMember::Field(resolved_proof) = &resolved_members[1]
    else {
        panic!("resolved proof field");
    };
    let psi_symbol_resolved_trees::data::DataMember::Variant(resolved_wrapped) =
        &resolved_members[2]
    else {
        panic!("resolved wrapped case");
    };
    let [resolved_witness] = resolved.data_payload_fields(resolved_wrapped.payload) else {
        panic!("one resolved payload field");
    };
    assert_eq!(
        resolved_value.relevance,
        psi_language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        resolved_proof.relevance,
        psi_language_core::BindingRelevance::Erased
    );
    assert_eq!(
        resolved_witness.relevance,
        psi_language_core::BindingRelevance::Erased
    );
    let resolved_snapshot = resolved.snapshot_json().expect("resolved snapshot");
    assert!(resolved_snapshot.contains("\"relevance\":\"relevant\""));
    assert!(resolved_snapshot.contains("\"relevance\":\"erased\""));

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let typed_data = typed.data_definitions().first().expect("typed data");
    let typed_members = typed.data_members(typed_data);
    let psi_typed_trees::data::DataMember::Field(typed_value) = &typed_members[0] else {
        panic!("typed value field");
    };
    let psi_typed_trees::data::DataMember::Field(typed_proof) = &typed_members[1] else {
        panic!("typed proof field");
    };
    let psi_typed_trees::data::DataMember::Variant(typed_wrapped) = &typed_members[2] else {
        panic!("typed wrapped case");
    };
    let [typed_witness] = typed.data_payload_fields(typed_wrapped) else {
        panic!("one typed payload field");
    };
    assert_eq!(
        typed_value.relevance,
        psi_language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        typed_proof.relevance,
        psi_language_core::BindingRelevance::Erased
    );
    assert_eq!(
        typed_witness.relevance,
        psi_language_core::BindingRelevance::Erased
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
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let [conformance] = typed.conformances() else {
        panic!("one typed conformance");
    };
    assert!(matches!(
        conformance.subject,
        psi_typed_trees::trait_definition::ConformanceSubject::Subjectless
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let machine = &typed.machines()[0];
    let state = &typed.machine_states(machine)[0];
    let hoisted = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local)
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
    let psi_typed_trees::types::TypeReferenceNode::Constrained {
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
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "i32"
    ));
    assert!(matches!(
        typed.type_reference_table.constraints(*constraints),
        [
            psi_typed_trees::types::TypeConstraintNode::ArithmeticDomain(
                psi_numerics::arithmetic::ArithmeticDomain::Wrapping
            )
        ]
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let trait_definition = &typed.traits()[0];
    let [_, relation] = typed.trait_type_parameters(trait_definition) else {
        panic!("trait should retain a proposition parameter");
    };
    assert!(matches!(
        relation.kind,
        psi_typed_trees::data::TypeParameterKind::Proposition { .. }
    ));
    let [signature] = typed.trait_machine_signatures(trait_definition) else {
        panic!("trait should retain one proof signature");
    };
    let [contract] = typed.state_signature_contracts(signature) else {
        panic!("proof signature should retain one ensures contract");
    };
    let [psi_typed_trees::domain::ProofFact::Proposition(application)] =
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
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
        psi_typed_trees::proposition::PropositionBody::Primitive
    ));
    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its requires contract");
    };
    let [fact] = typed.proof_facts.span_or_empty(contract.facts) else {
        panic!("requires should retain one proposition fact");
    };
    let psi_typed_trees::domain::ProofFact::Proposition(application) = fact else {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve const visibility");
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its proposition requirement");
    };
    let [psi_typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("requires should retain the proposition application");
    };
    assert!(matches!(
        application.binder_arguments[0].kind,
        psi_typed_trees::proposition::PropositionBinderArgumentKind::Type
    ));
    assert!(matches!(
        application.binder_arguments[1].kind,
        psi_typed_trees::proposition::PropositionBinderArgumentKind::Const
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
            lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let [resolved_contract] =
        resolved_program.machine_contracts(&resolved_program.roots.machines[0])
    else {
        panic!("machine should retain one resolved contract");
    };
    let [psi_symbol_resolved_trees::domain::ProofFact::Expression(resolved_application)] =
        resolved_program.proof_facts(resolved_contract.facts)
    else {
        panic!("requires should retain one resolved expression fact");
    };
    let psi_symbol_resolved_trees::expression::ExpressionNode::Call(resolved_call) =
        resolved_program
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
    let [psi_typed_trees::domain::ProofFact::Proposition(application)] =
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let request = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::Call(call) => {
                call.quotient_operation.as_ref()
            }
            _ => None,
        })
        .expect("sealed quotient request");

    assert_eq!(
        request.kind,
        psi_typed_trees::expression::QuotientOperationKind::Lift
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
        psi_symbols::SymbolKind::State
    );
    assert_eq!(
        typed
            .symbols
            .name(typed.symbols.get(request.selected_theorem.symbol).parent,),
        "representative_respects"
    );
    assert_eq!(
        typed.symbols.get(request.selected_theorem.symbol).kind,
        psi_symbols::SymbolKind::State
    );
}

#[test]
fn sealed_quotient_request_rejects_conformance_shaped_proof_discovery() {
    let source = r#"
        data Representative { value: i32; }
        trait Respects {}
        RepresentativeRespect: Representative satisfies Respects {}
        machine representative(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<representative, RepresentativeRespect>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("a conformance must not stand in for an exact theorem machine");

    assert!(
        diagnostic
            .message
            .contains("must resolve exactly to one resultless theorem machine entry"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn sealed_quotient_define_requires_both_exact_static_identities() {
    let source = r#"
        data Representative { value: i32; }
        machine representative(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::define<representative>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("define without an exact named conformance must reject");

    assert!(
        diagnostic
            .message
            .contains("requires exactly two static arguments"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn sealed_quotient_namespace_cannot_be_shadowed() {
    let source = r#"
        data Representative { value: i32; }
        trait Respects {}
        RepresentativeRespect: Representative satisfies Respects {}
        machine representative(value: Representative) -> Representative { value }

        data Quotient {}
        machine Quotient::lift(value: Representative) -> Representative { value }
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<representative, RepresentativeRespect>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("an authored Quotient namespace must not capture the sealed wrapper");

    assert!(
        diagnostic.message.contains("cannot be shadowed"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_cannot_declare_structural_equatable_conformance() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;
        ExactQEquatable: ExactQ satisfies Equatable;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("a quotient must not synthesize representative equality");

    assert!(
        diagnostic
            .message
            .contains("cannot synthesize equality for a quotient type"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_cannot_choose_a_zero_value_representative() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        machine impossible_default()
        ensures zero_value<ExactQ>() == zero_value<ExactQ>();
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("a quotient must not expose a canonical zero representative");

    assert!(
        diagnostic
            .message
            .contains("cannot observe or choose a retained quotient representative"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn quotient_field_cannot_enter_synthesized_container_equality() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        data Wrapper { value: ExactQ; }
        WrapperEquatable: Wrapper satisfies Equatable;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("container synthesis must not compare a quotient representative field");

    assert!(
        diagnostic
            .message
            .contains("field `value` of `Wrapper` has quotient type `ExactQ`"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn runtime_quotient_equality_requires_a_named_lifted_operation() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        machine compare(left: ExactQ, right: ExactQ) -> bool {
            left == right
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let diagnostic = lower_symbol_resolved_trees(&resolved)
        .expect_err("runtime quotient equality must not observe representatives");

    assert!(
        diagnostic
            .message
            .contains("retained representatives are opaque"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn proof_position_quotient_equality_remains_for_congruence() {
    let source = r#"
        data Carrier {}
        proposition equivalent(left: Carrier, right: Carrier);
        trait Equivalence<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
        }
        CarrierEquivalence: satisfies Equivalence<Carrier, equivalent> {}
        data ExactQ = Carrier % equivalent
        where equivalent satisfies
            Equivalence<Carrier, equivalent>
            as CarrierEquivalence;

        machine cite(left: ExactQ, right: ExactQ)
        requires left == right
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .expect("logical quotient equality must remain available to congruence validation");
}

#[test]
fn proposition_application_rejects_in_runtime_value_position() {
    let source = r#"
        proposition related(left: i32, right: i32);
        machine bad(value: i32) {
            related(value, value);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let diagnostic = lower_symbol_resolved_trees(&resolved_program)
        .expect_err("runtime proposition use must fail closed");

    assert!(diagnostic.message.contains("proof-only"));
}

#[test]
fn transparent_proposition_alias_normalizes_to_its_expansion() {
    let source = r#"
        proposition related(left: i32, right: i32);
        proposition self_related(value: i32) = related(value, value);

        machine through_alias(value: i32)
        requires self_related(value)
        {
        }

        machine through_expansion(value: i32)
        requires related(value, value)
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let normalized = typed
        .machines()
        .iter()
        .map(|machine| {
            let [contract] = typed.machine_contracts(machine) else {
                panic!("machine should retain one contract");
            };
            let [psi_typed_trees::domain::ProofFact::Proposition(application)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("contract should retain one proposition application");
            };
            typed
                .normalize_proposition_application(application)
                .expect("application should normalize")
                .identity_label()
        })
        .collect::<Vec<_>>();

    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0], normalized[1]);
    assert!(!normalized[0].contains("self_related"));
    assert!(normalized[0].contains("related"));
}

#[test]
fn lowers_dungeon_style_machine_program() {
    let source = r#"
    data Inventory {
        gold: u32[exact];
    }

    pub machine Inventory::clear(&mut self, inventory: &mut Inventory) {
        inventory.gold = 0;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.data_definitions().len(), 1);
    assert_eq!(typed_trees.machines().len(), 1);
    assert_eq!(
        typed_trees.machine_states(&typed_trees.machines()[0]).len(),
        1
    );
    assert!(
        typed_trees
            .symbols
            .find_child_by_name(typed_trees.symbols.root(), "u32")
            .is_some()
    );
}

#[test]
fn lowers_slice_range_surface_into_typed_trees() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> usize {
        let values: [usize; 4] = [1, 2, 3, 4];
        let view: &[usize] = values.as_slice();
        let tail: &[usize] = view[1..];
        tail.len
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");

    assert!(
        typed_trees
            .machines()
            .first()
            .is_some_and(|machine| !typed_trees.machine_states(machine).is_empty())
    );
}

#[test]
fn preserves_structural_recast_targets_through_typed_lowering() {
    let source = r#"
    machine inspect(bytes: [u8; 4]) {
        let fixed: &[u8; 4] = &bytes as &[u8; 4];
        let slice: &[u8] = &bytes as &[u8];
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("typed lowering should succeed");
    let machine = &typed_trees.machines()[0];
    let state = &typed_trees.machine_states(machine)[0];
    let locals = typed_trees
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();

    let psi_typed_trees::expression::ExpressionNode::Borrow(fixed_borrow) = typed_trees
        .expression_table
        .expression(locals[0].initial_value)
    else {
        panic!("fixed-array initializer should retain its shared borrow");
    };
    assert_eq!(
        fixed_borrow.access,
        psi_language_semantics::ReferenceAccess::Shared
    );
    let psi_typed_trees::expression::ExpressionNode::Cast(fixed) =
        typed_trees.expression_table.expression(fixed_borrow.target)
    else {
        panic!("fixed-array shared-borrow target should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(fixed.target_type),
        psi_typed_trees::types::TypeReferenceNode::FixedArray {
            length: psi_typed_trees::types::FixedArrayLength::Literal(4),
            ..
        }
    ));

    let psi_typed_trees::expression::ExpressionNode::Borrow(slice_borrow) = typed_trees
        .expression_table
        .expression(locals[1].initial_value)
    else {
        panic!("slice initializer should retain its shared borrow");
    };
    assert_eq!(
        slice_borrow.access,
        psi_language_semantics::ReferenceAccess::Shared
    );
    let psi_typed_trees::expression::ExpressionNode::Cast(slice) =
        typed_trees.expression_table.expression(slice_borrow.target)
    else {
        panic!("slice shared-borrow target should remain a cast");
    };
    assert!(matches!(
        typed_trees
            .type_reference_table
            .type_reference(slice.target_type),
        psi_typed_trees::types::TypeReferenceNode::Slice { .. }
    ));
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
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.domain_definitions().len(), 3);
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Alive")
        .expect("alive domain should lower");
    assert!(domain.symbol.is_valid());
    assert_eq!(domain.name.as_str(), "Player::Alive");
    let facts = typed_trees.proof_facts(domain);
    assert_eq!(facts.len(), 2);
    let psi_typed_trees::domain::ProofFact::Membership(membership) = &facts[0] else {
        panic!("first domain fact should be membership")
    };
    assert!(membership.domain_symbol.is_valid());
    assert!(domain.semantic_clause_token_count >= 3);
    assert_eq!(
        domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Present
    );
    assert!(domain.target_type.is_valid());
    let resolved_domain = resolved_program
        .domain_definitions
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Alive")
        .expect("resolved alive domain");
    assert_eq!(domain.semantic_roles, resolved_domain.semantic_roles);
    assert_eq!(
        domain.establishment_routes,
        resolved_domain.establishment_routes
    );
    assert!(domain.semantic_roles.is_empty());
    let tagged = typed_trees
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.name.as_str() == "Player::Tagged")
        .expect("typed tagged domain");
    assert_eq!(
        tagged.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert!(tagged.semantic_roles.is_empty());
}

#[test]
fn lowers_case_union_domain_proofs_from_exact_resolved_symbols() {
    let source = r#"
    data Command {
        case Move(dx: i32);
        case Say(volume: i32);
    }

    domain Command::Interactive
    requires
        self in Command::Move | Command::Say;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let expected_symbols = resolved_program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Command")
        .map(|definition| {
            let cases = resolved_program
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    psi_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            (definition.symbol, cases)
        })
        .expect("Command data");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("case-union proof should lower");
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Command::Interactive")
        .expect("interactive domain");
    let [psi_typed_trees::domain::ProofFact::Expression(expression)] =
        typed_trees.proof_facts(domain)
    else {
        panic!("case union should lower as one proof expression");
    };
    let psi_typed_trees::expression::ExpressionNode::Binary(union) =
        typed_trees.expression_table.expression(*expression)
    else {
        panic!("proof expression should remain a union");
    };

    for (expression, expected_case) in [union.left, union.right]
        .into_iter()
        .zip(expected_symbols.1)
    {
        let psi_typed_trees::expression::ExpressionNode::Binary(equality) =
            typed_trees.expression_table.expression(expression)
        else {
            panic!("case membership should lower to exact tag equality");
        };
        let psi_typed_trees::expression::ExpressionNode::Name(case) =
            typed_trees.expression_table.expression(equality.right)
        else {
            panic!("tag equality should retain an exact case path");
        };
        assert_eq!(case.head_symbol, expected_symbols.0);
        assert_eq!(case.symbol, expected_case);
        assert_eq!(
            typed_trees
                .expression_table
                .name_path_member_symbols(case.member_symbols),
            [expected_symbols.0, expected_case]
        );
    }
}

#[test]
fn normalizes_domain_constraints_by_short_name_and_carrier() {
    let source = r#"
    data Box<T> {
        value: T;
    }

    data Holder {
        signed: i64 in Tagged;
        unsigned: u64 in Tagged;
        boxed_signed: Box<i64 in Tagged>;
    }

    domain i64::Tagged;

    domain u64::Tagged
    requires
        self >= 0;
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let signed_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "i64::Tagged")
        .expect("signed domain");
    let unsigned_domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "u64::Tagged")
        .expect("unsigned domain");
    assert_eq!(
        signed_domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(
        unsigned_domain.predicate_body,
        psi_language_semantics::DomainPredicateBody::Present
    );

    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();

    let constraint_for = |type_reference| {
        let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("constrained field")
        };
        let [psi_typed_trees::types::TypeConstraintNode::Domain(domain)] =
            typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one domain constraint")
        };
        domain
    };

    let signed = constraint_for(fields["signed"]);
    assert_eq!(signed.symbol, signed_domain.symbol);
    assert_eq!(signed.semantic_id, signed_domain.semantic_id);
    assert_eq!(signed.predicate_body, signed_domain.predicate_body);
    assert_eq!(signed.semantic_roles, signed_domain.semantic_roles);
    assert_eq!(
        signed.establishment_routes,
        signed_domain.establishment_routes
    );

    let unsigned = constraint_for(fields["unsigned"]);
    assert_eq!(unsigned.symbol, unsigned_domain.symbol);
    assert_eq!(unsigned.semantic_id, unsigned_domain.semantic_id);
    assert_eq!(unsigned.predicate_body, unsigned_domain.predicate_body);
    assert_eq!(unsigned.semantic_roles, unsigned_domain.semantic_roles);
    assert_eq!(
        unsigned.establishment_routes,
        unsigned_domain.establishment_routes
    );

    let psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = typed
        .type_reference_table
        .type_reference(fields["boxed_signed"])
    else {
        panic!("generic field")
    };
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("one generic argument")
    };
    let boxed_signed = constraint_for(*argument);
    assert_eq!(boxed_signed.symbol, signed_domain.symbol);
    assert_eq!(boxed_signed.predicate_body, signed_domain.predicate_body);
    assert_eq!(boxed_signed.semantic_roles, signed_domain.semantic_roles);
    assert_eq!(
        boxed_signed.establishment_routes,
        signed_domain.establishment_routes
    );
}

#[test]
fn retains_closed_compiler_domain_subjects_and_layout_schema_report_fingerprint() {
    use psi_typed_trees::types::{
        DomainConstraintSubject, OmegaLayoutGrammar, TypeConstraintNode, TypeReferenceNode,
    };

    let source = r#"
    data Save {
        #1 value: u32;
    }

    data Holder {
        finite: f64 in Finite;
        carry: u64 in Carry::AnyCpu;
        layout: [u8; 32] in OmegaLayout<Save>;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    let domain_for = |type_reference| {
        let TypeReferenceNode::Constrained { constraints, .. } =
            typed.type_reference_table.type_reference(type_reference)
        else {
            panic!("constrained field")
        };
        let [TypeConstraintNode::Domain(domain)] =
            typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one domain constraint")
        };
        domain
    };

    assert_eq!(
        domain_for(fields["finite"]).subject,
        DomainConstraintSubject::Value(psi_language_semantics::value_domain::ValueDomain::Finite)
    );
    assert_eq!(
        domain_for(fields["carry"]).subject,
        DomainConstraintSubject::Carry(psi_language_semantics::CarryPermission::AnyCpu)
    );
    let layout = domain_for(fields["layout"]);
    assert_eq!(
        layout.subject,
        DomainConstraintSubject::OmegaLayout {
            grammar: OmegaLayoutGrammar::Derived,
        }
    );
    let [schema] = layout.arguments.as_slice() else {
        panic!("one structural layout schema")
    };
    let save = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Save")
        .expect("Save");
    assert!(matches!(
        typed.type_reference_table.type_reference(*schema),
        TypeReferenceNode::Named { symbol, name }
            if *symbol == save.symbol && name.as_str() == "Save"
    ));

    let finite_identity = typed.normalized_type_identity(fields["finite"]);
    assert!(finite_identity.as_str().contains("compiler-domain"));
    assert!(finite_identity.as_str().contains("finite"));
    assert!(!finite_identity.as_str().contains("Finite"));
    let carry_identity = typed.normalized_type_identity(fields["carry"]);
    assert!(carry_identity.as_str().contains("any-cpu"));
    assert!(!carry_identity.as_str().contains("Carry::AnyCpu"));
    let layout_identity = typed.normalized_type_identity(fields["layout"]);
    assert!(layout_identity.as_str().contains("omega-layout"));
    assert!(layout_identity.as_str().contains("derived"));
    assert!(layout_identity.as_str().contains("Save"));
    assert!(!layout_identity.as_str().contains("OmegaLayout"));
}

#[test]
fn symbol_backed_domain_spelling_cannot_spoof_compiler_subject() {
    let source = r#"
    data Holder {
        value: f64 in Finite;
    }

    domain f64::Finite;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let field = typed
        .data_members(holder)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .expect("value field");
    let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("constrained field")
    };
    let [psi_typed_trees::types::TypeConstraintNode::Domain(domain)] =
        typed.type_reference_table.constraints(*constraints)
    else {
        panic!("one domain constraint")
    };

    assert!(domain.symbol.is_valid());
    assert_eq!(
        domain.subject,
        psi_typed_trees::types::DomainConstraintSubject::Declared
    );
}

#[test]
fn carry_alias_expansion_retains_closed_invalid_symbol_atoms() {
    let source = r#"
    data Token {}
    data Holder {
        value: Token in Token::Portable;
    }
    domain Token::Portable = Carry::Portable;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let field = typed
        .data_members(holder)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => Some(field),
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .expect("value field");
    let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("constrained field")
    };
    let subjects = typed
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .map(|constraint| match constraint {
            psi_typed_trees::types::TypeConstraintNode::Domain(domain) => {
                assert!(!domain.symbol.is_valid());
                domain.subject
            }
            _ => panic!("carry alias must expand only to domain constraints"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        subjects,
        psi_language_semantics::CarryPermission::ALL
            .map(psi_typed_trees::types::DomainConstraintSubject::Carry)
    );
}

#[test]
fn expands_transparent_domain_aliases_before_semantic_normalization() {
    let source = r#"
    data Socket {
        connected: bool;
        authenticated: bool;
    }

    domain Socket::Connected
    requires
        self.connected;
    domain Socket::Authenticated
    requires
        self.authenticated;
    domain Socket::Usable =
        Socket::Connected & Socket::Authenticated;
    domain Socket::Ready = Socket::Usable;
    domain Socket::Prepared
    requires
        self in Socket::Ready;

    data Holder {
        aliased: Socket in Usable;
        expanded: Socket in Connected & Authenticated;
    }

    machine is_usable(socket: Socket) -> bool {
        socket in Socket::Usable
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let symbol_named = |name: &str| {
        typed
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .map(|domain| domain.symbol)
            .expect("declared domain")
    };
    let connected = symbol_named("Socket::Connected");
    let authenticated = symbol_named("Socket::Authenticated");
    let usable = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Socket::Usable")
        .expect("retained alias declaration");
    assert_eq!(usable.alias.as_ref().expect("alias").constituents.len(), 2);

    let prepared = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Socket::Prepared")
        .expect("prepared domain");
    let imported = typed
        .proof_facts(prepared)
        .iter()
        .map(|fact| match fact {
            psi_typed_trees::domain::ProofFact::Membership(membership) => membership.domain_symbol,
            psi_typed_trees::domain::ProofFact::Expression(_) => {
                panic!("alias should expand to membership atoms")
            }
            psi_typed_trees::domain::ProofFact::Proposition(_) => {
                panic!("domain alias should not become a proposition application")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(imported, [connected, authenticated]);

    let holder = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .expect("Holder");
    let fields = typed
        .data_members(holder)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.name.as_str(), field.type_reference))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        typed.normalized_type_identity(fields["aliased"]),
        typed.normalized_type_identity(fields["expanded"]),
        "alias and explicit conjunction must have one normalized identity"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "is_usable")
        .expect("membership machine");
    let has_atomic_conjunction = typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .any(|statement| {
            let psi_typed_trees::statement::StatementNode::Expression(expression) = statement
            else {
                return false;
            };
            matches!(
                typed.expression_table.expression(*expression),
                psi_typed_trees::expression::ExpressionNode::Binary(binary)
                    if binary.operator
                        == psi_typed_trees::expression::BinaryOperator::And
            )
        });
    assert!(
        has_atomic_conjunction,
        "executable alias membership must lower to an atomic conjunction"
    );
}

#[test]
fn parameter_domain_conjunction_synthesizes_each_membership_contract() {
    let source = r#"
    domain [u8]::Meaning;
    domain [u8]::Utf8
    requires
        valid_utf8(self);
    domain [u8]::NoNul
    requires
        no_nul(self);

    machine inspect(bytes: &[u8] in Meaning & Utf8 & NoNul) {
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("inspect machine");
    let state = typed.machine_states(machine).first().expect("entry state");
    let names: Vec<_> = typed
        .state_contracts(state)
        .iter()
        .map(|contract| {
            let [psi_typed_trees::domain::ProofFact::Membership(membership)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("one synthesized membership fact")
            };
            typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .expect("normalized declared domain")
                .name
                .as_str()
                .to_owned()
        })
        .collect();

    assert_eq!(
        names,
        ["[u8]::Meaning", "[u8]::Utf8", "[u8]::NoNul"],
        "bodyless and predicate-bearing constraints are all call-boundary obligations"
    );
}

#[test]
fn internal_state_domain_constraint_does_not_leak_to_machine_entry() {
    let source = r#"
    data Token {
        value: u64;
    }

    domain Token::Issued;

    machine carry(seed: u64) {
        transition { _ -> hold(Token { value: seed }) }

        state hold(token: Token in Issued) {
        }
    }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed.machines().first().expect("carry machine");
    assert!(
        typed.machine_contracts(machine).is_empty(),
        "an internal state's constraint must not become a machine-wide entry contract"
    );
    let [entry, hold] = typed.machine_states(machine) else {
        panic!("entry and hold states")
    };
    assert!(typed.state_contracts(entry).is_empty());
    assert_eq!(
        typed.state_contracts(hold).len(),
        1,
        "the constrained state retains its own implicit membership requirement"
    );
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    assert_eq!(typed_trees.operators().len(), 1);
    let operator = &typed_trees.operators()[0];
    assert_eq!(
        typed_trees
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Slice", "index"]
    );
    assert_eq!(
        typed_trees
            .data_type_parameters
            .span_or_empty(operator.type_parameters)
            .len(),
        1
    );
    assert_eq!(
        typed_trees
            .state_parameters
            .span_or_empty(operator.parameters)
            .len(),
        2
    );
    assert!(operator.symbol.is_valid());
    assert!(operator.return_type.is_valid());
    assert_eq!(
        typed_trees
            .signature_contracts
            .span_or_empty(operator.contracts)
            .len(),
        1
    );
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let domain = typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Quantity::Additive")
        .expect("domain should lower");
    let operators = typed_trees.domain_operators(domain);

    assert_eq!(operators.len(), 1);
    assert_eq!(
        domain.semantic_roles.denotation_dimension,
        Some(domain.semantic_id)
    );
    assert!(domain.semantic_roles.arithmetic_policy.is_none());
    assert!(operators[0].symbol.is_valid());
    assert_eq!(
        typed_trees
            .operator_path_members(operators[0].name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["add"]
    );
    assert_eq!(typed_trees.proof_facts(domain).len(), 1);
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
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
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");
    let machine = &typed_trees.machines()[0];
    let entry = &typed_trees.machine_states(machine)[0];
    let statements = typed_trees
        .statement_table
        .statements(entry.statement_nodes);

    let psi_typed_trees::statement::StatementNode::Transition(transition) = &statements[0] else {
        panic!("entry should lower to transition statement");
    };
    let psi_typed_trees::statement::TransitionTargetNode::Named {
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    for definition in typed_trees.data_definitions() {
        assert_eq!(
            definition.properties.multiplicity,
            psi_language_semantics::Multiplicity::Linear
        );
    }
    let holder = &typed_trees.data_definitions()[1];
    assert_eq!(
        typed_trees.data_type_parameters(holder)[0]
            .bounds
            .multiplicity,
        psi_language_semantics::Multiplicity::Linear
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
    let resolved_program = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
    let typed_trees =
        lower_symbol_resolved_trees(&resolved_program).expect("lowering should succeed");

    let machine = typed_trees.machines().first().expect("retag machine");
    let [parameter] = typed_trees.machine_type_parameters(machine) else {
        panic!("retag should retain one const parameter");
    };
    assert_eq!(parameter.name.as_str(), "To");
    assert!(matches!(
        parameter.kind,
        psi_typed_trees::data::TypeParameterKind::Const { .. }
    ));
    let state = &typed_trees.machine_states(machine)[0];
    let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed_trees
        .type_reference_table
        .type_reference(state.return_type)
    else {
        panic!("return should retain Quantity<To>");
    };
    let [psi_typed_trees::types::TypeConstraintNode::Domain(return_domain)] =
        typed_trees.type_reference_table.constraints(*constraints)
    else {
        panic!("return should carry one declared domain");
    };
    let psi_typed_trees::types::TypeReferenceNode::Named {
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
            psi_typed_trees::expression::ExpressionNode::Cast(cast) => Some((handle, cast)),
            _ => None,
        })
        .expect("retag body should retain its qualification cast");
    let [cast_argument] = typed_trees
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments)
    else {
        panic!("cast should retain one index argument");
    };
    let psi_typed_trees::types::TypeReferenceNode::Named {
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
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership
    );
    assert!(matches!(
        selection.target(),
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == cast.semantic_domain_symbol
    ));

    let conversion = typed_trees.traits().first().expect("Conversion trait");
    let [requirement] = typed_trees.trait_machine_signatures(conversion) else {
        panic!("Conversion should retain one requirement");
    };
    let [requirement_parameter] = typed_trees.state_signature_type_parameters(requirement) else {
        panic!("generic requirement should retain its const binder");
    };
    let psi_typed_trees::types::TypeReferenceNode::Constrained {
        constraints: requirement_constraints,
        ..
    } = typed_trees
        .type_reference_table
        .type_reference(requirement.return_type)
    else {
        panic!("generic requirement result should retain Quantity<To>");
    };
    let [psi_typed_trees::types::TypeConstraintNode::Domain(requirement_domain)] = typed_trees
        .type_reference_table
        .constraints(*requirement_constraints)
    else {
        panic!("generic requirement result should carry one domain");
    };
    let psi_typed_trees::types::TypeReferenceNode::Named {
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
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let resolved = lower_syntax_trees(&syntax_trees).expect("resolution should succeed");
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
