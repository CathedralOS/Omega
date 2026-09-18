use crate::lowerer::lower_symbol_resolved_trees;
use crate::lowerer::tests::lower_source;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

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
    // The authored `Binding::DllImport("module", "symbol")` bootstrap spelling
    // is retired, so the typed import identity no longer exists and has no source
    // producer. The remaining bootstrap spellings still exercise the interned
    // identity table until their carriers are removed.
    let source = r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via Binding::Syscall(4);
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
        Some(&language_semantics::ExternalBindingIdentity::Syscall { number: 4 })
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
