use super::{
    SeededContinuationError, exact_field_symbol, exact_top_level_data_symbol,
    lower_seeded_extension, lower_symbol_resolved_trees,
    lower_symbol_resolved_trees_to_seeded_base, plain_data_extension_shape_is_supported,
    resolved_root_shape_is_supported,
};
use psi_source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees_to_symbol_resolved_trees::{
    RebasedSeededSymbolResolvedTrees, lower_syntax_extension_with_authored_selection_frontier,
    lower_syntax_trees, lower_syntax_trees_with_sources,
};
use psi_tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};
use std::path::PathBuf;
use std::sync::Arc;

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
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
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
                psi_typed_trees::statement::StatementNode::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        call.target_symbol
    });
    assert_ne!(applications[0], applications[1]);
}

fn seeded_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (super::SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let base_syntax = parse_syntax_trees_with_id(
        base_id,
        &Lexer::new(base_source).tokenize().expect("tokenize base"),
    )
    .expect("parse base");
    let resolved = lower_syntax_trees_with_sources(&base_syntax, Arc::new(base_sources.clone()))
        .expect("resolve base");
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    assert_eq!(typing_base.typed().symbols.source_files().count(), 1);
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source)
            .tokenize()
            .expect("tokenize extension"),
    )
    .expect("parse extension");
    let seeded = lower_syntax_extension_with_authored_selection_frontier(
        typing_base.resolved_base_for_extension(),
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
    .expect("resolve seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase extension selections");
    assert_eq!(rebased.trees().symbols.source_files().count(), 2);
    (typing_base, rebased)
}

fn seeded_normalized_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (super::SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let base_syntax = parse_syntax_trees_with_id(
        base_id,
        &Lexer::new(base_source).tokenize().expect("tokenize base"),
    )
    .expect("parse base");
    let resolved = lower_syntax_trees_with_sources(&base_syntax, Arc::new(base_sources.clone()))
        .expect("resolve base");
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source)
            .tokenize()
            .expect("tokenize extension"),
    )
    .expect("parse extension");
    let extension_syntax = psi_generic_instances::normalize_pre_resolution(extension_syntax)
        .expect("normalize extension unit");
    let seeded = lower_syntax_extension_with_authored_selection_frontier(
        typing_base.resolved_base_for_extension(),
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
    .expect("resolve normalized seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase normalized extension selections");
    (typing_base, rebased)
}

fn lower_source(source: &str) -> Result<psi_typed_trees::TypedTrees, psi_diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    lower_symbol_resolved_trees(&resolved)
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
        psi_language_semantics::MachineSupplyMode::ExternalRealization {
            binding: None,
            mechanism: None,
        }
    );
    let [conformance] = typed.machine_trait_conformances(leaf) else {
        panic!("one exact external conformance")
    };
    assert!(conformance.external_binding.is_none());
    let psi_typed_trees::expression::ExpressionNode::Call(call) = typed
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
fn settles_satisfied_top_level_requirement_to_its_exact_machine_symbol() {
    use psi_language_semantics::declaration_selection::{
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
        psi_language_semantics::MachineSupplyMode::TopLevelRequirement
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
        psi_typed_trees::machine::resolve_satisfied_declaration(
            &typed,
            satisfier,
            conformance,
        ),
        Some(psi_typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(selected))
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
    let mut resolved = lower_syntax_trees(&syntax).expect("resolve");
    let requirement = resolved
        .machines
        .find_mut(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement");
    let ordinary_symbol = requirement.symbol;
    requirement.supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;

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
        psi_typed_trees::machine::resolve_satisfied_declaration(&typed, satisfier, conformance,)
            .is_none()
    );
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
            .contains("requires exactly `F, Congruence`"),
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

#[test]
fn seeded_plain_data_continuation_appends_named_data_and_preserves_typed_sidecars() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated { base: Authored; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(psi_typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            statement_index: 7,
            source_statement_index: 11,
            target: psi_typed_trees::name::Identifier::generated_static("target"),
            source: psi_typed_trees::name::Identifier::generated_static("source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_members = before
        .data_members
        .iter()
        .map(|(handle, member)| (handle, member.clone()))
        .collect::<Vec<_>>();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed =
        lower_seeded_extension(extension, base).expect("plain generated data should append");

    assert_eq!(
        typed.data_definitions().len(),
        before.data_definitions().len() + 1
    );
    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert_eq!(
        typed
            .data_members
            .iter()
            .take(before_members.len())
            .map(|(handle, member)| (handle, member.clone()))
            .collect::<Vec<_>>(),
        before_members
    );
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = psi_arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    let generated = typed.data_definitions().last().expect("generated data");
    let [psi_typed_trees::data::DataMember::Field(generated_field)] = typed.data_members(generated)
    else {
        panic!("one generated field")
    };
    assert!(generated_field.symbol.arena_index() > before.symbols.symbols().len() as u32);
    assert!(generated_field.type_reference.arena_index() > before_type_count as u32);
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );
    assert!(typed.authored_declaration_selections().len() > resolved_ledger.len());
}

#[test]
fn seeded_plain_data_continuation_appends_exact_erased_lifetime_data_graph() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "pub data Main { value: u32; }",
        r#"
            pub data View<'buf> { body: &'buf Main; }
            pub data Envelope<'msg> { view: View<'msg>; tail: [u8; 2]; }
        "#,
    );
    base.typed_mut()
        .evidence_forwardings
        .push(psi_typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            statement_index: 13,
            source_statement_index: 17,
            target: psi_typed_trees::name::Identifier::generated_static("lifetime-target"),
            source: psi_typed_trees::name::Identifier::generated_static("lifetime-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_members = before
        .data_members
        .iter()
        .map(|(handle, member)| (handle, member.clone()))
        .collect::<Vec<_>>();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("erased lifetime-only generated data should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert_eq!(
        typed
            .data_members
            .iter()
            .take(before_members.len())
            .map(|(handle, member)| (handle, member.clone()))
            .collect::<Vec<_>>(),
        before_members
    );
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = psi_arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );

    let main = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("retained Main data");
    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("generated View data");
    let envelope = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Envelope")
        .expect("generated Envelope data");
    assert_eq!(
        view.lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["buf"]
    );
    assert_eq!(
        envelope
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );

    let [psi_typed_trees::data::DataMember::Field(body)] = typed.data_members(view) else {
        panic!("View has one body field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = typed
        .type_reference_table
        .type_reference(body.type_reference)
    else {
        panic!("View.body remains a reference")
    };
    assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("buf"));
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*referee)
    else {
        panic!("View.body referee remains nominal")
    };
    assert_eq!(*symbol, main.symbol);

    let [psi_typed_trees::data::DataMember::Field(view_field), _] = typed.data_members(envelope)
    else {
        panic!("Envelope has view and tail fields")
    };
    let psi_typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(view_field.type_reference)
    else {
        panic!("Envelope.view remains an erased lifetime application")
    };
    assert_eq!(*base_symbol, view.symbol);
    assert_eq!(
        lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );
    assert!(
        typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    );
}

#[test]
fn seeded_plain_data_continuation_appends_owner_local_type_parameter_data() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated<T> { value: T; pair: [T; 2]; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(psi_typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            statement_index: 19,
            source_statement_index: 23,
            target: psi_typed_trees::name::Identifier::generated_static("generic-target"),
            source: psi_typed_trees::name::Identifier::generated_static("generic-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("owner-local type-parameter data should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );

    let generated = typed.data_definitions().last().expect("generated data");
    let [parameter] = typed.data_type_parameters(generated) else {
        panic!("Generated has one type parameter")
    };
    assert!(matches!(
        parameter.kind,
        psi_typed_trees::data::TypeParameterKind::Type
    ));
    assert_eq!(
        parameter.bounds,
        psi_typed_trees::data::DataProperties::default()
    );
    let [
        psi_typed_trees::data::DataMember::Field(value),
        psi_typed_trees::data::DataMember::Field(pair),
    ] = typed.data_members(generated)
    else {
        panic!("Generated has value and pair fields")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("Generated.value remains the owner-local type parameter")
    };
    assert_eq!(*symbol, parameter.symbol);
    let psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } = typed
        .type_reference_table
        .type_reference(pair.type_reference)
    else {
        panic!("Generated.pair remains a fixed array")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*element_type)
    else {
        panic!("Generated.pair element remains the owner-local type parameter")
    };
    assert_eq!(*symbol, parameter.symbol);
}

#[test]
fn seeded_plain_data_continuation_appends_one_exact_local_primitive_instance() {
    let (mut base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        r#"
            data Cell<T> { value: T; }
            data Generated { first: Cell<u32>; second: Cell<u32>; base: Authored; }
        "#,
    );
    base.typed_mut()
        .evidence_forwardings
        .push(psi_typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            statement_index: 29,
            source_statement_index: 31,
            target: psi_typed_trees::name::Identifier::generated_static("instance-target"),
            source: psi_typed_trees::name::Identifier::generated_static("instance-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_members = before
        .data_members
        .iter()
        .map(|(handle, member)| (handle, member.clone()))
        .collect::<Vec<_>>();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("one local primitive instance should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert_eq!(
        typed
            .data_members
            .iter()
            .take(before_members.len())
            .map(|(handle, member)| (handle, member.clone()))
            .collect::<Vec<_>>(),
        before_members
    );
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = psi_arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );
    let template = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("local generic template");
    let instance = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.generic_instance.is_some())
        .expect("one synthesized instance");
    let wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    let origin = instance.generic_instance.expect("instance origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = typed.type_reference_table.type_reference(origin)
    else {
        panic!("instance retains its exact generic origin")
    };
    assert_eq!(*base_symbol, template.symbol);
    assert!(lifetime_arguments.is_empty());
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("one exact instance argument")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named {
        symbol: argument_symbol,
        ..
    } = typed.type_reference_table.type_reference(*argument)
    else {
        panic!("primitive instance argument remains nominal")
    };
    assert_eq!(
        typed.symbols.get(*argument_symbol).kind,
        psi_symbols::SymbolKind::BuiltinType
    );
    let [psi_typed_trees::data::DataMember::Field(value)] = typed.data_members(instance) else {
        panic!("instance has one substituted field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("instance field is the primitive argument")
    };
    assert_eq!(*symbol, *argument_symbol);
    let wrapper_instance_uses = typed
        .data_members(wrapper)
        .iter()
        .filter(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return false;
            };
            matches!(
                typed.type_reference_table.type_reference(field.type_reference),
                psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                    if *symbol == instance.symbol
            )
        })
        .count();
    assert_eq!(
        wrapper_instance_uses, 2,
        "repeated uses deduplicate to one instance"
    );
}

#[test]
fn seeded_local_instance_gate_rejects_origin_and_declaration_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { value: T; } data Generated { first: Cell<u32>; second: Cell<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));
    let instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].generic_instance.is_some())
        .expect("one instance index");
    let template_symbol = match resolved.data_definitions[instance_index]
        .generic_instance
        .as_ref()
    {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => {
            origin.base_symbol
        }
        _ => unreachable!(),
    };
    let template_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].symbol == template_symbol)
        .expect("one template index");
    let wrapper_index = (frontier..resolved.data_definitions.len())
        .find(|index| *index != instance_index && *index != template_index)
        .expect("one wrapper index");
    let template = &resolved.data_definitions[template_index];
    let instance = &resolved.data_definitions[instance_index];
    let wrapper = &resolved.data_definitions[wrapper_index];
    let instance_members = instance.members;
    let wrapper_members = wrapper.members;
    assert!(exact_top_level_data_symbol(&resolved, template));
    assert!(exact_top_level_data_symbol(&resolved, instance));
    assert!(exact_top_level_data_symbol(&resolved, wrapper));
    let [psi_symbol_resolved_trees::data::DataMember::Field(template_field)] =
        resolved.data_members(template.members)
    else {
        panic!("one template field")
    };
    let [psi_symbol_resolved_trees::data::DataMember::Field(instance_field)] =
        resolved.data_members(instance.members)
    else {
        panic!("one instance field")
    };
    let [
        psi_symbol_resolved_trees::data::DataMember::Field(first_wrapper_field),
        psi_symbol_resolved_trees::data::DataMember::Field(second_wrapper_field),
    ] = resolved.data_members(wrapper.members)
    else {
        panic!("two wrapper fields")
    };
    assert_ne!(template_field.symbol, instance_field.symbol);
    assert!(exact_field_symbol(
        &resolved,
        template.symbol,
        template_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        instance.symbol,
        instance_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        wrapper.symbol,
        first_wrapper_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        wrapper.symbol,
        second_wrapper_field
    ));
    assert!(
        !exact_field_symbol(&resolved, wrapper.symbol, template_field),
        "a coordinated field-row retarget must not erase its exact parent"
    );

    let mut wrong_origin = resolved.clone();
    let Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_origin
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_origin,
        frontier
    ));

    let mut missing_argument = resolved.clone();
    let Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) = missing_argument
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.arguments = psi_arena::HandleSpan::empty();
    assert!(!plain_data_extension_shape_is_supported(
        &missing_argument,
        frontier
    ));

    let mut wrong_instance_identity = resolved.clone();
    wrong_instance_identity.data_definitions[instance_index].name =
        psi_symbol_resolved_trees::name::DiagnosticName::generated("Cell<u64>");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_instance_identity,
        frontier
    ));

    let mut wrong_retired_identity = resolved.clone();
    wrong_retired_identity.data_definitions[instance_index]
        .retired_identities
        .push(71);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_retired_identity, frontier),
        "the instance must retain the template's exact retired-identity set"
    );

    let mut wrong_substitution_name = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(instance_field) =
        wrong_substitution_name
            .tables
            .declarations
            .data_members
            .get_mut(instance_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Named { name, .. } =
        &mut instance_field.type_reference
    else {
        unreachable!()
    };
    *name = psi_symbol_resolved_trees::name::DiagnosticName::generated("u64");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_substitution_name, frontier),
        "the substituted builtin spelling must remain joined to its symbol"
    );

    let mut wrong_wrapper_type_name = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(wrapper_field) = wrong_wrapper_type_name
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Named { name, .. } =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    *name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Cell<u64>");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_wrapper_type_name, frontier),
        "a wrapper use's diagnostic name cannot drift from the selected instance"
    );

    let authored_symbol = resolved.data_definitions[0].symbol;
    let mut wrong_parent = resolved.clone();
    let template_span = wrong_parent.data_definitions[template_index]
        .name
        .source_span();
    let instance_span = wrong_parent.data_definitions[instance_index]
        .name
        .source_span();
    wrong_parent.data_definitions[template_index].symbol = authored_symbol;
    wrong_parent.data_definitions[template_index].name =
        psi_symbol_resolved_trees::name::DiagnosticName::new("Authored", template_span);
    wrong_parent.data_definitions[instance_index].name =
        psi_symbol_resolved_trees::name::DiagnosticName::new("Authored<u32>", instance_span);
    let Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_parent
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_symbol = authored_symbol;
    origin.base_name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Authored");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_parent, frontier),
        "coordinated top-level identity retarget cannot detach parameter/field children"
    );

    let mut wrong_kind = resolved.clone();
    wrong_kind.data_definitions[wrapper_index].symbol = template_field.symbol;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_kind, frontier),
        "a Field symbol cannot impersonate the wrapper Data declaration"
    );

    let mut wrong_field_parent = resolved.clone();
    wrong_field_parent.data_definitions[wrapper_index].members =
        wrong_field_parent.data_definitions[template_index].members;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_field_parent, frontier),
        "a coordinated member-span retarget cannot reuse another owner's fields"
    );

    let mut lifetime_template = resolved.clone();
    lifetime_template.data_definitions[template_index]
        .lifetime_parameters
        .push(psi_symbol_resolved_trees::name::DiagnosticName::generated(
            "scope",
        ));
    assert!(!plain_data_extension_shape_is_supported(
        &lifetime_template,
        frontier
    ));

    let mut fact_instance = resolved.clone();
    fact_instance.data_definitions[instance_index].where_facts =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(1), 1);
    assert!(!plain_data_extension_shape_is_supported(
        &fact_instance,
        frontier
    ));

    let mut quotient_wrapper = resolved.clone();
    quotient_wrapper.data_definitions[wrapper_index].quotient =
        Some(psi_symbol_resolved_trees::data::QuotientDefinition {
            carrier: psi_symbol_resolved_trees::types::TypeReference::Unit,
            relation: Vec::new(),
            relation_symbol: psi_symbols::SymbolHandle::invalid(),
            equivalence: None,
        });
    assert!(!plain_data_extension_shape_is_supported(
        &quotient_wrapper,
        frontier
    ));

    let mut zero_gated_instance = resolved;
    zero_gated_instance.data_definitions[instance_index].zero_gated = true;
    assert!(!plain_data_extension_shape_is_supported(
        &zero_gated_instance,
        frontier
    ));
}

#[test]
fn seeded_nested_local_instance_gate_rejects_dependency_and_reachability_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { values: [T; 2]; } data Outer<T> { inner: Cell<T>; direct: T; } data Generated { value: Outer<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let cell_instance_index = index("Cell<u32>");
    let outer_template_index = index("Outer");
    let outer_instance_index = index("Outer<u32>");
    let wrapper_index = index("Generated");

    let mut wrong_nested_member = resolved.clone();
    let outer_members = wrong_nested_member.data_definitions[outer_instance_index].members;
    let psi_symbol_resolved_trees::data::DataMember::Field(inner) = wrong_nested_member
        .tables
        .declarations
        .data_members
        .get_mut(outer_members.start())
    else {
        unreachable!()
    };
    inner.type_reference = psi_symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_nested_member, frontier),
        "a nested synthesized field must replay the exact inner instance"
    );

    let mut wrong_template_application = resolved.clone();
    let outer_template_members =
        wrong_template_application.data_definitions[outer_template_index].members;
    let psi_symbol_resolved_trees::data::DataMember::Field(inner) = wrong_template_application
        .tables
        .declarations
        .data_members
        .get_mut(outer_template_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut inner.type_reference
    else {
        unreachable!()
    };
    application.base_name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_template_application, frontier),
        "a local template application cannot drift from its exact base symbol"
    );

    let mut wrong_inner_origin = resolved.clone();
    let Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_inner_origin
        .data_definitions[cell_instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_inner_origin, frontier),
        "a transitive dependency must retain its exact synthesis origin"
    );

    let mut unreachable_instances = resolved;
    let wrapper_members = unreachable_instances.data_definitions[wrapper_index].members;
    let psi_symbol_resolved_trees::data::DataMember::Field(value) = unreachable_instances
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    value.type_reference = psi_symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&unreachable_instances, frontier),
        "an internally coherent but unreachable synthesized subgraph is not admitted"
    );
}

#[test]
fn seeded_local_sum_instance_gate_rejects_case_and_payload_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Maybe<T> { case #1 None; case #2 Some(#1 value: T, retired #3); retired #4; } data Generated { value: Maybe<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let template_index = index("Maybe");
    let instance_index = index("Maybe<u32>");
    let template_members = resolved.data_definitions[template_index].members;
    let instance_members = resolved.data_definitions[instance_index].members;
    let template_some = match &resolved.data_members(template_members)[1] {
        psi_symbol_resolved_trees::data::DataMember::Variant(variant) => variant,
        _ => panic!("Maybe::Some template case"),
    };
    let instance_some = match &resolved.data_members(instance_members)[1] {
        psi_symbol_resolved_trees::data::DataMember::Variant(variant) => variant,
        _ => panic!("Maybe<u32>::Some instance case"),
    };
    assert_ne!(template_some.symbol, instance_some.symbol);
    assert_eq!(template_some.identity, instance_some.identity);
    assert_eq!(
        template_some.retired_payload_identities,
        instance_some.retired_payload_identities
    );

    let mut reordered_cases = resolved.clone();
    reordered_cases
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(instance_members)
        .swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_cases, frontier),
        "case declaration order is part of the synthesized instance"
    );

    let mut wrong_case_parent = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_case_parent
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.symbol = template_some.symbol;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_case_parent, frontier),
        "a synthesized case cannot reuse the template case symbol"
    );

    let mut wrong_case_identity = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_case_identity
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.identity = Some(71);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_case_identity, frontier),
        "case identity must replay exactly"
    );

    let mut wrong_retired_payload = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_retired_payload
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.retired_payload_identities.push(72);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_retired_payload, frontier),
        "retired payload identities must replay exactly"
    );

    let mut wrong_payload_substitution = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &wrong_payload_substitution.data_members(instance_members)[1]
    else {
        unreachable!()
    };
    let payload = instance_some_mut.payload;
    wrong_payload_substitution
        .tables
        .declarations
        .data_payload_fields
        .span_mut_or_empty(payload)[0]
        .type_reference = psi_symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_payload_substitution, frontier),
        "payload substitution must replay the exact type argument"
    );

    let mut wrong_payload_parent = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_payload_parent
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.payload = template_some.payload;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_payload_parent, frontier),
        "a synthesized case cannot reuse template-owned payload fields"
    );
}

#[test]
fn seeded_lifetime_instance_gate_rejects_binder_and_application_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'scope, T> { value: &'scope T; } data Generated<'scope> { value: Borrowed<'scope, u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let instance_index = index("Borrowed<u32>");
    let wrapper_index = index("Generated");
    let instance_members = resolved.data_definitions[instance_index].members;
    let wrapper_members = resolved.data_definitions[wrapper_index].members;

    let mut wrong_instance_binder = resolved.clone();
    wrong_instance_binder.data_definitions[instance_index].lifetime_parameters[0] =
        psi_symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_instance_binder, frontier),
        "the instance must retain the template's exact erased lifetime binder"
    );

    let mut missing_application_lifetime = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(wrapper_field) =
        &mut missing_application_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(wrapper_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.clear();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_application_lifetime, frontier),
        "the selected local instance requires its complete erased lifetime arity"
    );

    let mut unknown_application_lifetime = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(wrapper_field) =
        &mut unknown_application_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(wrapper_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments[0] =
        psi_symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&unknown_application_lifetime, frontier),
        "a local instance application can name only an owning lifetime binder"
    );

    let mut wrong_reference_lifetime = resolved;
    let psi_symbol_resolved_trees::data::DataMember::Field(instance_field) =
        &mut wrong_reference_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &mut instance_field.type_reference
    else {
        unreachable!()
    };
    reference.lifetime = Some(psi_symbol_resolved_trees::name::DiagnosticName::generated(
        "other",
    ));
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_reference_lifetime, frontier),
        "the substituted reference must retain the template lifetime exactly"
    );
}

#[test]
fn seeded_nested_lifetime_instance_gate_rejects_internal_edge_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Nested<'outer, 'inner, T> { value: Borrowed<'inner, 'outer, T>; } data Generated<'one, 'two> { value: Nested<'one, 'two, u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let nested_template_index = index("Nested");
    let nested_instance_index = index("Nested<u32>");
    let borrowed_instance_index = index("Borrowed<u32>");
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let nested_instance_members = resolved.data_definitions[nested_instance_index].members;
    let borrowed_instance_symbol = resolved.data_definitions[borrowed_instance_index].symbol;
    let nested_instance_symbol = resolved.data_definitions[nested_instance_index].symbol;
    let nested_instance_name = resolved.data_definitions[nested_instance_index]
        .name
        .clone();

    let mut unknown_template_lifetime = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut unknown_template_lifetime
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments[0] =
        psi_symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&unknown_template_lifetime, frontier),
        "a nested template application can name only an owning lifetime binder"
    );

    let mut missing_instance_lifetime = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut missing_instance_lifetime
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.clear();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_instance_lifetime, frontier),
        "the synthesized nested edge must retain its exact lifetime arity"
    );

    let mut reordered_instance_lifetimes = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) =
        &mut reordered_instance_lifetimes
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_instance_lifetimes, frontier),
        "the synthesized nested edge must retain exact lifetime argument order"
    );

    let mut redirected_instance = resolved;
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut redirected_instance
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    assert_eq!(application.base_symbol, borrowed_instance_symbol);
    application.base_symbol = nested_instance_symbol;
    application.base_name = nested_instance_name;
    assert!(
        !plain_data_extension_shape_is_supported(&redirected_instance, frontier),
        "a nested lifetime edge cannot be redirected to another validated instance"
    );
}

#[test]
fn seeded_lifetime_type_argument_gate_rejects_origin_routing_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'one, 'two, u32>>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let holder_instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            matches!(
                resolved.data_definitions[*index].generic_instance.as_ref(),
                Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin))
                    if origin.base_name.as_str() == "Holder"
            )
        })
        .expect("closed Holder instance");
    let holder_instance_symbol = resolved.data_definitions[holder_instance_index].symbol;
    let holder_instance_name = resolved.data_definitions[holder_instance_index]
        .name
        .clone();
    let origin_arguments = match resolved.data_definitions[holder_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut reordered_lifetimes = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Generic(argument) =
        &mut reordered_lifetimes
            .tables
            .declarations
            .child_type_references
            .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.lifetime_arguments.swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_lifetimes, frontier),
        "a lifetime-bearing Type argument must forward the owner's exact binder order"
    );

    let mut missing_lifetime = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Generic(argument) = &mut missing_lifetime
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.lifetime_arguments.pop();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_lifetime, frontier),
        "a lifetime-bearing Type argument must retain complete erased arity"
    );

    let mut redirected_argument = resolved;
    let psi_symbol_resolved_trees::types::TypeReference::Generic(argument) =
        &mut redirected_argument
            .tables
            .declarations
            .child_type_references
            .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.base_symbol = holder_instance_symbol;
    argument.base_name = holder_instance_name;
    assert!(
        !plain_data_extension_shape_is_supported(&redirected_argument, frontier),
        "the Type argument cannot redirect to another local lifetime instance"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'two, 'one, u32>>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a permuted nested lifetime route remains rejected until it has distinct identity"
    );
}

#[test]
fn seeded_arithmetic_domain_argument_gate_rejects_identity_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { value: T; } data Generated { value: Cell<u32 in Wrapping>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let instance = resolved
        .data_definitions
        .iter()
        .skip(frontier)
        .find(|definition| definition.name.as_str() == "Cell<u32 in Wrapping>")
        .expect("closed constrained Cell instance");
    let origin_arguments = match instance.generic_instance.as_ref() {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let instance_members = instance.members;

    let mut changed_origin_domain = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &mut changed_origin_domain
            .tables
            .declarations
            .child_type_references
            .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let [constraint] = changed_origin_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(argument.constraints)
    else {
        unreachable!()
    };
    *constraint = psi_symbol_resolved_trees::types::TypeConstraint::ArithmeticDomain(
        psi_numerics::arithmetic::ArithmeticDomain::Saturating,
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_origin_domain, frontier),
        "the arithmetic-domain argument participates in canonical instance identity"
    );

    let mut changed_field_domain = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut changed_field_domain
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(field_type) =
        &field.type_reference
    else {
        unreachable!()
    };
    let field_constraints = field_type.constraints;
    let [constraint] = changed_field_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(field_constraints)
    else {
        unreachable!()
    };
    *constraint = psi_symbol_resolved_trees::types::TypeConstraint::ArithmeticDomain(
        psi_numerics::arithmetic::ArithmeticDomain::Saturating,
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_field_domain, frontier),
        "the substituted field must retain the origin's exact arithmetic domain"
    );

    let mut changed_base_identity = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &changed_base_identity
            .tables
            .declarations
            .child_type_references
            .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let base_type = argument.base_type;
    let psi_symbol_resolved_trees::types::TypeReference::Named { name, .. } = changed_base_identity
        .tables
        .declarations
        .child_type_references
        .get_mut(base_type)
    else {
        unreachable!()
    };
    *name = psi_symbol_resolved_trees::name::DiagnosticName::generated("u64");
    assert!(
        !plain_data_extension_shape_is_supported(&changed_base_identity, frontier),
        "a constrained argument cannot spoof its exact carrier identity"
    );

    let mut changed_constraint_kind = resolved;
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &changed_constraint_kind
            .tables
            .declarations
            .child_type_references
            .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [constraint] = changed_constraint_kind
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    *constraint = psi_symbol_resolved_trees::types::TypeConstraint::Named(
        psi_symbol_resolved_trees::name::DiagnosticName::generated("Wrapping"),
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_constraint_kind, frontier),
        "a spelling-identical named constraint cannot replace the arithmetic-domain tag"
    );
}

#[test]
fn seeded_unindexed_declared_domain_argument_rejoins_exact_identity() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; } data Token { value: u8; } domain Token::Issued; domain Token::Other;",
        "data Cell<T> { value: T; } data Generated { value: Cell<Token in Issued>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let instance = resolved
        .data_definitions
        .iter()
        .skip(frontier)
        .find(|definition| definition.name.as_str() == "Cell<Token in Issued>")
        .expect("closed declared-domain Cell instance");
    let origin_arguments = match instance.generic_instance.as_ref() {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let instance_members = instance.members;
    let issued_symbol = resolved
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("Issued domain declaration")
        .symbol;
    let other_name = resolved
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Other")
        .expect("Other domain declaration")
        .name
        .clone();

    let mut changed_origin_domain = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &changed_origin_domain
            .tables
            .declarations
            .child_type_references
            .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [psi_symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = changed_origin_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    domain.name = other_name.clone();
    assert!(
        !plain_data_extension_shape_is_supported(&changed_origin_domain, frontier),
        "the declared-domain symbol participates in canonical instance identity"
    );

    let mut changed_field_domain = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &changed_field_domain
        .tables
        .declarations
        .data_members
        .span_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(field_type) =
        &field.type_reference
    else {
        unreachable!()
    };
    let field_constraints = field_type.constraints;
    let [psi_symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = changed_field_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(field_constraints)
    else {
        unreachable!()
    };
    domain.name = other_name;
    assert!(
        !plain_data_extension_shape_is_supported(&changed_field_domain, frontier),
        "the substituted field must retain the origin's exact declared domain"
    );

    let mut detached_domain_name = resolved.clone();
    let psi_symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &detached_domain_name
            .tables
            .declarations
            .child_type_references
            .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [psi_symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = detached_domain_name
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    domain.name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Issued");
    assert!(
        !plain_data_extension_shape_is_supported(&detached_domain_name, frontier),
        "a same-spelled domain without an authored selection cannot mint identity"
    );

    let typed = lower_seeded_extension(extension, base)
        .expect("the exact declared-domain instance should use the seeded continuation");
    let instance = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell<Token in Issued>")
        .expect("typed declared-domain Cell instance");
    let [psi_typed_trees::data::DataMember::Field(field)] = typed.data_members(instance) else {
        panic!("typed declared-domain Cell instance retains one field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("typed instance field retains its constraint")
    };
    let [psi_typed_trees::types::TypeConstraintNode::Domain(domain)] =
        typed.type_reference_table.constraints(*constraints)
    else {
        panic!("typed instance field retains one declared domain")
    };
    assert_eq!(domain.symbol, issued_symbol);
    assert!(domain.arguments.is_empty());

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; } data Token { value: u8; } domain Token::Root; domain Token::Issued = Token::Root;",
        "data Cell<T> { value: T; } data Generated { value: Cell<Token in Issued>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "transparent domain aliases remain outside the retained continuation cohort"
    );
}

#[test]
fn seeded_integer_const_instance_gate_rejects_carrier_origin_and_shape_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Block<T, const N: u64> { values: [T; N]; } data Nested<T, const N: u64> { value: Block<T, N>; } data Generated { value: Nested<u16, 2>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let block_template_index = index("Block");
    let nested_template_index = index("Nested");
    let block_instance_index = index("Block<u16, 2>");
    let block_parameters = resolved.data_definitions[block_template_index].type_parameters;
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let block_instance_members = resolved.data_definitions[block_instance_index].members;
    let block_origin_arguments = match resolved.data_definitions[block_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut unsupported_carrier = resolved.clone();
    unsupported_carrier
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(block_parameters)[1]
        .kind = psi_symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: psi_symbol_resolved_trees::types::TypeReference::Unit,
    };
    assert!(
        !plain_data_extension_shape_is_supported(&unsupported_carrier, frontier),
        "the scalar const rung cannot silently widen to an unsupported carrier"
    );

    let mut noncanonical_origin = resolved.clone();
    noncanonical_origin
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(block_origin_arguments)[1] =
        psi_symbol_resolved_trees::types::TypeReference::Named {
            symbol: psi_symbols::SymbolHandle::invalid(),
            name: psi_symbol_resolved_trees::name::DiagnosticName::generated("02"),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&noncanonical_origin, frontier),
        "a closed const origin must retain canonical decimal spelling"
    );

    let mut wrong_substituted_length = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_substituted_length
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(block_instance_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::FixedArray(array) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    array.length = psi_symbol_resolved_trees::types::FixedArrayLength::Literal(3);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_substituted_length, frontier),
        "the instance array length must replay the exact const argument"
    );

    let mut wrong_forwarded_binder = resolved;
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_forwarded_binder
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &field.type_reference
    else {
        unreachable!()
    };
    let arguments = application.arguments;
    let arguments = wrong_forwarded_binder
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(arguments);
    arguments[1] = arguments[0].clone();
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_forwarded_binder, frontier),
        "a const slot cannot be redirected to an ordinary Type binder"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Tiny<const N: u8> { tag: u8; } data Generated { value: Tiny<256>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a closed scalar const argument must fit its exact declared carrier"
    );
}

#[test]
fn seeded_boolean_const_instance_gate_rejects_carrier_origin_and_forwarding_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Flag<T, const ENABLED: bool> { marker: u8; } data Nested<T, const ENABLED: bool> { value: Flag<T, ENABLED>; } data Generated { value: Nested<u16, true>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let flag_template_index = index("Flag");
    let nested_template_index = index("Nested");
    let flag_parameters = resolved.data_definitions[flag_template_index].type_parameters;
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let flag_instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            resolved.data_definitions[*index]
                .generic_instance
                .as_ref()
                .is_some_and(|origin| {
                    matches!(
                        origin,
                        psi_symbol_resolved_trees::types::TypeReference::Generic(origin)
                            if origin.base_name.as_str() == "Flag"
                    )
                })
        })
        .expect("closed Flag instance");
    let flag_origin_arguments = match resolved.data_definitions[flag_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut unsupported_carrier = resolved.clone();
    unsupported_carrier
        .tables
        .declarations
        .data_type_parameters
        .span_mut_or_empty(flag_parameters)[1]
        .kind = psi_symbol_resolved_trees::data::TypeParameterKind::Const {
        type_reference: psi_symbol_resolved_trees::types::TypeReference::Unit,
    };
    assert!(
        !plain_data_extension_shape_is_supported(&unsupported_carrier, frontier),
        "the Boolean const rung cannot silently widen to an unsupported carrier"
    );

    let mut noncanonical_origin = resolved.clone();
    noncanonical_origin
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(flag_origin_arguments)[1] =
        psi_symbol_resolved_trees::types::TypeReference::Named {
            symbol: psi_symbols::SymbolHandle::invalid(),
            name: psi_symbol_resolved_trees::name::DiagnosticName::generated(
                psi_language_semantics::const_value::CanonicalConstValue::new(
                    "bool",
                    "boolean4:true",
                    "TRUE",
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&noncanonical_origin, frontier),
        "a Boolean const origin must retain the exact canonical atom"
    );

    let mut wrong_forwarded_binder = resolved;
    let psi_symbol_resolved_trees::data::DataMember::Field(field) = &mut wrong_forwarded_binder
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &field.type_reference
    else {
        unreachable!()
    };
    let arguments = application.arguments;
    let arguments = wrong_forwarded_binder
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(arguments);
    arguments[1] = arguments[0].clone();
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_forwarded_binder, frontier),
        "a Boolean const slot cannot be redirected to an ordinary Type binder"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Wrong<const N: bool> { values: [u8; N]; } data Generated { value: Wrong<true>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a Boolean const binder cannot become an array length"
    );
}

#[test]
fn seeded_structured_const_instance_gate_replays_declarations_values_and_carriers_exactly() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Config { count: u8; enabled: bool; } data Configs {} const Configs::PRIMARY: Config = Config { count: 7, enabled: true }; data Indexed<const C: Config> { marker: u8; } data Generated { value: Indexed<Configs::PRIMARY>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(resolved_root_shape_is_supported(&resolved, &base.resolved));
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let config_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].name.as_str() == "Config")
        .expect("Config carrier");
    let config_symbol = resolved.data_definitions[config_index].symbol;
    let config_members = resolved.data_definitions[config_index].members;
    let instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            matches!(
                resolved.data_definitions[*index].generic_instance.as_ref(),
                Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin))
                    if origin.base_name.as_str() == "Indexed"
            )
        })
        .expect("closed Indexed instance");
    let origin_arguments = match resolved.data_definitions[instance_index]
        .generic_instance
        .as_ref()
    {
        Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let original_atom = match &resolved
        .tables
        .declarations
        .child_type_references
        .span_or_empty(origin_arguments)[0]
    {
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name }
            if !symbol.is_valid() =>
        {
            psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .expect("canonical structured const atom")
        }
        _ => unreachable!(),
    };

    let mut display_drift = resolved.clone();
    display_drift
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0] =
        psi_symbol_resolved_trees::types::TypeReference::Named {
            symbol: psi_symbols::SymbolHandle::invalid(),
            name: psi_symbol_resolved_trees::name::DiagnosticName::generated(
                psi_language_semantics::const_value::CanonicalConstValue::new(
                    original_atom.type_name.clone(),
                    original_atom.encoding.clone(),
                    "Config { enabled: true, count: 7 }",
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&display_drift, frontier),
        "diagnostic display cannot drift from the decoded canonical value"
    );

    let mut type_claim_drift = resolved.clone();
    type_claim_drift
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0] =
        psi_symbol_resolved_trees::types::TypeReference::Named {
            symbol: psi_symbols::SymbolHandle::invalid(),
            name: psi_symbol_resolved_trees::name::DiagnosticName::generated(
                psi_language_semantics::const_value::CanonicalConstValue::new(
                    "Other",
                    original_atom.encoding.clone(),
                    original_atom.display.clone(),
                )
                .atom(),
            ),
        };
    assert!(
        !plain_data_extension_shape_is_supported(&type_claim_drift, frontier),
        "the encoded value must claim the exact resolved carrier"
    );

    let mut recursive_carrier = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(first_field) = &mut recursive_carrier
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(config_members)[0]
    else {
        unreachable!()
    };
    first_field.type_reference = psi_symbol_resolved_trees::types::TypeReference::Named {
        symbol: config_symbol,
        name: psi_symbol_resolved_trees::name::DiagnosticName::generated("Config"),
    };
    assert!(
        !plain_data_extension_shape_is_supported(&recursive_carrier, frontier),
        "recursive structured const carriers remain fenced"
    );

    let mut public_support_const = resolved;
    public_support_const.const_declarations[0].is_public = true;
    assert!(
        !resolved_root_shape_is_supported(&public_support_const, &base.resolved),
        "the bounded data continuation cannot grow the public const surface"
    );
}

#[test]
fn seeded_plain_data_continuation_accepts_local_instance_collections() {
    for (name, extension_source) in [
        (
            "multiple_instances",
            "data Cell<T> { value: T; } data Generated { left: Cell<u32>; right: Cell<u64>; }",
        ),
        (
            "one_wrapper_use",
            "data Cell<T> { value: T; } data Generated { value: Cell<u32>; }",
        ),
        (
            "template_wrapper_cycle",
            "data Cell<T> { value: T; companion: Generated; } data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "wrapper_self_cycle",
            "data Cell<T> { value: T; } data Generated { first: Cell<u32>; second: Cell<u32>; next: Generated; }",
        ),
        (
            "nominal_argument",
            "data Cell<T> { value: T; } data Generated { value: Cell<Authored>; }",
        ),
        (
            "multiple_parameters",
            "data Cell<T, U> { left: T; right: U; } data Generated { value: Cell<u32, u64>; }",
        ),
        (
            "phantom_parameter",
            "data Cell<T> { tag: u8; } data Generated { value: Cell<u32>; }",
        ),
        (
            "indirect_wrapper_use",
            "data Cell<T> { value: T; } data Generated { values: [Cell<u32>; 2]; }",
        ),
        (
            "multiple_templates_instances_and_wrappers",
            "data Cell<T> { value: T; } data Pair<A, B> { first: A; second: B; } data Item { value: u8; } data First { one: Cell<u32>; pair: Pair<u16, u64>; indirect: [Cell<u32>; 2]; } data Second { nominal: Cell<Item>; repeated: Pair<u16, u64>; }",
        ),
        (
            "nested_instances",
            "data Cell<T> { value: T; } data Outer<T> { value: T; } data Generated { value: Outer<Cell<u32>>; }",
        ),
        (
            "indirect_template_parameter",
            "data Cell<T> { values: [T; 2]; } data Generated { value: Cell<u32>; }",
        ),
        (
            "nested_fixpoint_and_indirect_substitution",
            "data Cell<T> { values: [T; 2]; } data Outer<T> { inner: Cell<T>; direct: T; } data Generated { nested: Outer<u32>; repeated: Outer<u32>; }",
        ),
        (
            "nondefault_bound",
            "data Cell<T [copy]> [copy] { value: T; } data Generated { value: Cell<u32>; }",
        ),
        (
            "nested_bound_forwarding",
            "data Cell<T [copy]> [copy] { value: T; } data Outer<U [copy]> [copy] { cell: Cell<U>; direct: U; } data Generated { value: Outer<u32>; }",
        ),
        (
            "generic_sum_instance",
            "data Maybe<T> { case None; case Some(value: T); } data Generated { value: Maybe<u32>; }",
        ),
        (
            "generic_mixed_instance",
            "data Outcome<T> { tag: u8; case Empty; case Value(value: T); } data Generated { value: Outcome<u32>; }",
        ),
        (
            "nested_generic_sum_instances",
            "data Maybe<T> { case None; case Some(values: [T; 2]); } data Outer<T> { case Empty; case Nested(value: Maybe<T>); } data Generated { value: Outer<u32>; }",
        ),
        (
            "reference_and_slice_parameter_shells",
            "data Shell<T> { shared: &T; values: [T]; nested: &[T; 2]; } data Generated { value: Shell<u32>; }",
        ),
        (
            "lifetime_bearing_reference_instance",
            "data Borrowed<'scope, T> { value: &'scope T; } data Generated<'scope> { value: Borrowed<'scope, u32>; }",
        ),
        (
            "nested_lifetime_instance_graph",
            "data Borrowed<'scope, T> { value: &'scope T; } data Nested<'scope, T> { value: Borrowed<'scope, T>; } data Generated<'scope> { value: Nested<'scope, u32>; }",
        ),
        (
            "deep_permuted_lifetime_instance_graph",
            "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Middle<'outer, 'inner, T> { values: [Borrowed<'inner, 'outer, T>; 2]; } data Outer<'first, 'second, T> { value: Middle<'second, 'first, T>; } data Generated<'one, 'two> { value: Outer<'one, 'two, u32>; }",
        ),
        (
            "nested_lifetime_sum_payload",
            "data Borrowed<'scope, T> { value: &'scope T; } data MaybeBorrow<'scope, T> { case None; case Some(value: Borrowed<'scope, T>); } data Generated<'scope> { value: MaybeBorrow<'scope, u32>; }",
        ),
        (
            "lifetime_instance_as_type_argument",
            "data Borrowed<'borrow, T> { value: &'borrow T; } data BorrowBox<'boxed, T> { value: T; } data Generated<'call> { value: BorrowBox<'call, Borrowed<'call, u32>>; }",
        ),
        (
            "nested_lifetime_instances_as_type_arguments",
            "data Borrowed<'borrow, T> { value: &'borrow T; } data BorrowBox<'boxed, T> { value: T; } data Outer<'outer, T> { value: T; } data Generated<'call> { value: Outer<'call, BorrowBox<'call, Borrowed<'call, u32>>>; }",
        ),
        (
            "ordered_multi_lifetime_instance_as_type_argument",
            "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'one, 'two, u32>>; }",
        ),
        (
            "integer_const_instance_graph",
            "data Block<T, const N: u64> { values: [T; N]; } data Nested<T, const N: u64> { value: Block<T, N>; } data Generated { value: Nested<u16, 2>; }",
        ),
        (
            "signed_integer_const_instance",
            "data Offset<const N: i64> { tag: u8; } data Generated { value: Offset<-2>; }",
        ),
        (
            "zero_integer_const_array_instance",
            "data Block<const N: u64> { values: [u8; N]; } data Generated { value: Block<0>; }",
        ),
        (
            "closed_expression_const_instance",
            "data Block<const N: u64> { values: [u8; N]; } data Generated { value: Block<1 + 1>; }",
        ),
        (
            "const_instance_as_type_argument",
            "data Block<const N: u64> { values: [u8; N]; } data Box<T> { value: T; } data Generated { value: Box<Block<2> >; }",
        ),
        (
            "boolean_const_instance_graph",
            "data Flag<T, const ENABLED: bool> { marker: u8; } data Nested<T, const ENABLED: bool> { value: Flag<T, ENABLED>; } data Generated { value: Nested<u16, true>; }",
        ),
        (
            "boolean_const_instance_as_type_argument",
            "data Flag<const ENABLED: bool> { marker: u8; } data Box<T> { value: T; } data Generated { value: Box<Flag<false> >; }",
        ),
        (
            "structured_record_const_instance_graph",
            "data Leaf { count: u8; enabled: bool; } data Config { leaves: [Leaf; 2]; } data Configs {} const Configs::PRIMARY: Config = Config { leaves: [Leaf { count: 1, enabled: true }, Leaf { count: 2, enabled: false }] }; data Indexed<const C: Config> { marker: u8; } data Nested<const C: Config> { value: Indexed<C>; } data Generated { value: Nested<Configs::PRIMARY>; }",
        ),
        (
            "structured_sum_const_instance_as_type_argument",
            "data Mode { case Left(value: u8); case Right; } data Modes {} const Modes::LEFT: Mode = Mode::Left { value: 7 }; data Indexed<const M: Mode> { marker: u8; } data Box<T> { value: T; } data Generated { value: Box<Indexed<Modes::LEFT> >; }",
        ),
        (
            "arithmetic_domain_constrained_argument",
            "data Cell<T> { value: T; } data Generated { value: Cell<u32 in Wrapping>; }",
        ),
    ] {
        let (base, extension) =
            seeded_normalized_plain_data_inputs("data Authored { value: u16; }", extension_source);
        let before = base.typed().data_definitions().len();
        let const_before = base.typed().const_declarations().len();
        let typed = lower_seeded_extension(extension, base)
            .unwrap_or_else(|_| panic!("{name} should use the seeded continuation"));
        assert!(typed.data_definitions().len() > before, "{name}");
        assert_eq!(
            typed.const_declarations().len(),
            const_before + usize::from(name.starts_with("structured_")),
            "{name} retains only its exact supporting const provenance"
        );
    }
}

#[test]
fn seeded_plain_data_continuation_fences_unsupported_normalized_generic_instances() {
    for (name, extension_source) in [
        (
            "cyclic_instances",
            "data Left<T> { right: Right<T>; } data Right<T> { left: Left<T>; } data Generated { value: Left<u32>; }",
        ),
        (
            "attached_method",
            "data Cell<T> { value: T; } machine Cell::clear<T>(&self) {} data Generated { value: Cell<u32>; }",
        ),
    ] {
        let (base, extension) =
            seeded_normalized_plain_data_inputs("data Authored { value: u16; }", extension_source);
        let expected = base.typed().clone();
        let Err((returned, error)) = lower_seeded_extension(extension, base) else {
            panic!("{name} must reject transactionally")
        };
        assert_eq!(
            error,
            SeededContinuationError::UnsupportedExtensionShape,
            "{name}"
        );
        assert_eq!(returned.into_typed(), expected, "{name}");
    }
}

#[test]
fn seeded_plain_data_continuation_retains_base_owned_type_application_graph() {
    let (mut base, extension) = seeded_normalized_plain_data_inputs(
        "data Cell<T> { value: T; } data Pair<A, B> { first: A; second: B; } data Main { value: u8; }",
        "data Generated { one: Cell<u32>; two: Cell<u64>; nested: Pair<Cell<u32>, u64>; indirect: [Cell<u16>; 2]; base: Main; } data AlsoGenerated { only: Cell<u8>; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(psi_typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            statement_index: 37,
            source_statement_index: 41,
            target: psi_typed_trees::name::Identifier::generated_static("base-application-target"),
            source: psi_typed_trees::name::Identifier::generated_static("base-application-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("the complete base-owned type-application graph should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = psi_arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );
    let template = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("retained base template");
    let pair = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Pair")
        .expect("retained two-parameter base template");
    let wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    assert!(
        typed
            .data_definitions()
            .iter()
            .all(|definition| definition.generic_instance.is_none()),
        "a cross-unit application stays structurally generic instead of inventing an extension-local instance"
    );
    let applications = typed
        .data_members(wrapper)
        .iter()
        .filter_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let psi_typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = typed
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                return None;
            };
            Some((*base_symbol, lifetime_arguments, *arguments))
        })
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 3);
    assert_eq!(
        applications
            .iter()
            .filter(|(base_symbol, _, _)| *base_symbol == template.symbol)
            .count(),
        2
    );
    let (_, pair_lifetimes, pair_arguments) = applications
        .iter()
        .find(|(base_symbol, _, _)| *base_symbol == pair.symbol)
        .expect("nested pair application remains explicit");
    assert!(pair_lifetimes.is_empty());
    let pair_arguments = typed
        .type_reference_table
        .type_reference_handles(*pair_arguments);
    assert_eq!(pair_arguments.len(), 2);
    let pair_argument_nodes = pair_arguments
        .iter()
        .map(|argument| typed.type_reference_table.type_reference(*argument))
        .collect::<Vec<_>>();
    assert!(matches!(
        pair_argument_nodes[0],
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));
    assert!(matches!(
        pair_argument_nodes[1],
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. }
            if typed.symbols.name(*symbol) == "u64"
    ));

    let indirect = typed
        .data_members(wrapper)
        .iter()
        .find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == "indirect").then_some(field.type_reference)
        })
        .expect("indirect generic field");
    let psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
        typed.type_reference_table.type_reference(indirect)
    else {
        panic!("indirect application retains its fixed-array shell")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*element_type),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));

    let second_wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "AlsoGenerated")
        .expect("second generated wrapper");
    let [psi_typed_trees::data::DataMember::Field(only)] = typed.data_members(second_wrapper)
    else {
        panic!("second wrapper retains one field")
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(only.type_reference),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));
}

#[test]
fn seeded_base_owned_type_application_validator_rejects_identity_and_arity_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Cell<T> { value: T; } data Main { value: u8; }",
        "data Generated { first: Cell<u32>; second: Cell<u32>; base: Main; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));
    let wrapper = resolved.data_definitions.iter().nth(frontier).unwrap();
    let wrapper_members = wrapper.members;

    let mut wrong_base_name = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(first) = wrong_base_name
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.base_name = psi_symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_base_name,
        frontier
    ));

    let mut wrong_base_symbol = resolved.clone();
    let wrapper_symbol = wrong_base_symbol
        .data_definitions
        .iter()
        .nth(frontier)
        .unwrap()
        .symbol;
    let psi_symbol_resolved_trees::data::DataMember::Field(first) = wrong_base_symbol
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.base_symbol = wrapper_symbol;
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_base_symbol,
        frontier
    ));

    let mut missing_argument = resolved.clone();
    let psi_symbol_resolved_trees::data::DataMember::Field(first) = missing_argument
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let psi_symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut first.type_reference
    else {
        unreachable!()
    };
    application.arguments = psi_arena::HandleSpan::empty();
    assert!(!plain_data_extension_shape_is_supported(
        &missing_argument,
        frontier
    ));

    let mut wrong_parameter_name = resolved;
    let parameter_span = wrong_parameter_name.data_definitions[0].type_parameters;
    wrong_parameter_name
        .tables
        .declarations
        .data_type_parameters
        .get_mut(parameter_span.start())
        .name = psi_symbol_resolved_trees::name::DiagnosticName::generated("U");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_parameter_name,
        frontier
    ));
}

#[test]
fn seeded_plain_data_continuation_accepts_broader_base_owned_generic_applications() {
    for (name, base_source, extension_source) in [
        (
            "single_use",
            "data Cell<T> { value: T; }",
            "data Generated { value: Cell<u32>; }",
        ),
        (
            "distinct_arguments",
            "data Cell<T> { value: T; }",
            "data Generated { first: Cell<u32>; second: Cell<u64>; }",
        ),
        (
            "indirect_use",
            "data Cell<T> { value: T; }",
            "data Generated { first: [Cell<u32>; 2]; second: Cell<u32>; }",
        ),
        (
            "attached_method",
            "data Cell<T> { value: T; } machine Cell::clear<T>(&self) {}",
            "data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "indirect_parameter",
            "data Cell<T> { values: [T; 2]; }",
            "data Generated { first: Cell<u32>; second: Cell<u32>; }",
        ),
        (
            "nominal_argument",
            "data Item { value: u8; } data Cell<T> { value: T; }",
            "data Generated { value: Cell<Item>; }",
        ),
        (
            "nondefault_bound",
            "data Cell<T [copy]> { value: T; }",
            "data Generated { value: Cell<u32>; }",
        ),
        (
            "lifetime_and_type_arguments",
            "data Cell<'item, T> { value: &'item T; }",
            "data Generated<'owner> { value: Cell<'owner, u32>; }",
        ),
    ] {
        let (base, extension) = seeded_normalized_plain_data_inputs(base_source, extension_source);
        let before = base.typed().data_definitions().len();
        let typed = lower_seeded_extension(extension, base)
            .unwrap_or_else(|(_, error)| panic!("{name} should continue: {error:?}"));
        assert!(typed.data_definitions().len() > before, "{name}");
    }
}

#[test]
fn seeded_continuation_appends_a_generated_machine_without_relowering_the_base() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; } machine authored() -> u32 { 1 }",
        "pub machine generated() -> u64 { 3 }",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("ordinary generated machine should append from the retained base");

    assert_eq!(typed.machines().len(), expected.machines().len() + 1);
    assert_eq!(
        &typed.machines()[..expected.machines().len()],
        expected.machines()
    );
    assert_eq!(typed.machines().last().unwrap().name.as_str(), "generated");
    assert_eq!(typed.data_definitions(), expected.data_definitions());
}

#[test]
fn seeded_continuation_attaches_a_monomorphic_method_to_its_exact_generated_data() {
    let (base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated { value: u32; } machine Generated::read(&self) -> u32 { self.value }",
    );
    let expected = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("ordinary attached method should append from the retained base");

    let generated = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Generated")
        .expect("generated data");
    let method = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Generated::read")
        .expect("generated attached method");
    assert_eq!(method.attached_data_symbol, generated.symbol);
    assert_eq!(
        typed.symbols.get(method.symbol).parent,
        typed.symbols.root()
    );
    assert_eq!(
        &typed.data_definitions()[..expected.data_definitions().len()],
        expected.data_definitions()
    );
}

#[test]
fn seeded_plain_data_continuation_fences_runtime_generic_and_invalid_lifetime_fields() {
    for extension_source in [
        "data Generated { value: Missing; }",
        "data Generated { value: Generic<u32 in Wrapping>; }",
        "data Generated<'scope> { value: &'missing Plain; }",
        "data Generated { value: Borrowed; }",
        "data Generated<'scope> { value: Borrowed<'scope, 'scope>; }",
        "data Generated<'scope> { value: Plain<'scope>; }",
        "data Generated<'scope> { value: Generic<'scope>; }",
    ] {
        let (base, extension) = seeded_plain_data_inputs(
            r#"
                data Plain {}
                data Borrowed<'scope> { value: &'scope Plain; }
                data Generic<T> { value: T; }
            "#,
            extension_source,
        );
        let expected = base.typed().clone();
        let (returned, error) = lower_seeded_extension(extension, base).expect_err(
            "runtime-generic or invalid lifetime fields are rejected by the retained continuation",
        );
        assert_eq!(error, SeededContinuationError::UnsupportedExtensionShape);
        assert_eq!(returned.into_typed(), expected);
    }
}

#[test]
fn seeded_plain_data_continuation_rejects_cross_paired_resolved_base_transactionally() {
    let (left, _) = seeded_plain_data_inputs("data Left {}", "data Added {}");
    let (_, right_extension) = seeded_plain_data_inputs("data Right {}", "data Added {}");
    let expected = left.typed().clone();

    let (returned, error) = lower_seeded_extension(right_extension, left)
        .expect_err("resolved and typed bases cannot be cross-paired");

    assert_eq!(error, SeededContinuationError::CrossPairedResolvedBase);
    assert_eq!(returned.into_typed(), expected);
}
