use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn retains_public_conformance_visibility_and_snapshot_shape() {
    let source = "pub trait Ranked {} pub data Card {} pub PowerOrder: Card satisfies Ranked {}";
    let tokens = Lexer::new(source).tokenize().expect("tokenize conformance");
    let syntax = parse_syntax_trees(&tokens).expect("parse conformance");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve conformance");
    let conformance = program.conformances.iter().next().expect("conformance");

    assert!(conformance.is_public);
    assert_eq!(
        conformance.alias.as_ref().map(|name| name.as_str()),
        Some("PowerOrder")
    );
    assert!(conformance.symbol.is_valid());
    let snapshot = program.snapshot();
    assert_eq!(snapshot.roots.conformances.len(), 1);
    assert!(snapshot.roots.conformances[0].is_public);
    assert_eq!(snapshot.roots.conformances[0].name, "PowerOrder");
}

#[test]
fn retains_public_data_trait_and_wire_visibility() {
    let source = r#"
        pub data PublicRecord { value: u32; }
        pub data Packet { #1 value: u32; }
        pub trait PublicTrait {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize data");
    let syntax = parse_syntax_trees(&tokens).expect("parse data");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve data");
    let public = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "PublicRecord")
        .expect("public data");
    let wire_derived = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Packet")
        .expect("wire-derived data");
    let wire_schema = program
        .wire_schemas
        .iter()
        .find(|schema| schema.name.as_str() == "Packet")
        .expect("wire schema");
    let trait_definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "PublicTrait")
        .expect("public trait");

    assert!(public.is_public);
    assert!(wire_derived.is_public);
    assert!(wire_schema.is_public);
    assert!(trait_definition.is_public);
    let snapshot = program.snapshot();
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
fn resolves_wire_owned_nested_field_type_identity() {
    let source = r#"
        data Header { #0 value: u32; }
        data Message { #0 header: Header; }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/main.omg"),
            source.to_owned(),
            PathBuf::from("package"),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::Base,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize wire data");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse wire data");
    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve source-owned wire data");
    let header_symbol = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Header")
        .expect("wire-derived Header data")
        .symbol;
    let message = program
        .wire_schemas
        .iter()
        .find(|schema| schema.name.as_str() == "Message")
        .expect("Message wire schema");
    let [symbol_resolved_trees::wire::WireMember::Field(header)] =
        program.wire_members(message.members)
    else {
        panic!("Message should retain one wire field")
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
        &header.type_reference
    else {
        panic!("nested wire field should retain its named type")
    };

    assert_eq!(name.as_str(), "Header");
    assert_eq!(*symbol, header_symbol);
}

#[test]
fn retains_public_machine_visibility_in_symbol_resolved_trees() {
    let tokens = Lexer::new("pub machine Package::entry() { }")
        .tokenize()
        .expect("tokenize public machine");
    let syntax = parse_syntax_trees(&tokens).expect("parse public machine");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve public machine");
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
        language_semantics::MachineSupplyMode::CheckedBody
    );
}

#[test]
fn resolves_machine_and_trait_const_parameter_carrier_types() {
    let source = r#"
        pub machine measure<const Width: u64>() { }
        pub trait Sized<const Width: u64> { }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const parameters");
    let syntax = parse_syntax_trees(&tokens).expect("parse const parameters");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve const parameters");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "measure")
        .expect("measure machine");
    let trait_definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Sized")
        .expect("Sized trait");
    for parameter in [
        &program.machine_type_parameters(machine)[0],
        &program.trait_type_parameters(trait_definition)[0],
    ] {
        let symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
            &parameter.kind
        else {
            panic!("const parameter")
        };
        let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = type_reference
        else {
            panic!("named const carrier")
        };
        assert_eq!(name.as_str(), "u64");
        assert!(symbol.is_valid());
        assert_eq!(
            program.symbols.get(*symbol).kind,
            symbols::SymbolKind::BuiltinType
        );
    }
}

#[test]
fn resolves_provider_selection_type_paths_to_exact_symbols() {
    let source = r#"
        boundary trait Console { machine write(); }
        data ConsoleProvider { }
        machine build(builder: &mut Build) {
            builder.select_provider<Console, ConsoleProvider>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize provider selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse provider selection");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve provider selection");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let call = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                provider_selection_expression(&program, *expression)
            }
            _ => None,
        })
        .expect("provider-selection expression call with operand custody");
    let [boundary, provider] = call.machine_arguments.as_ref() else {
        panic!("two retained provider-selection arguments")
    };

    assert_eq!(
        program.symbols.get(boundary.symbol).kind,
        symbols::SymbolKind::Trait
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        symbols::SymbolKind::Data
    );
    assert_eq!(program.symbols.name(boundary.symbol), "Console");
    assert_eq!(program.symbols.name(provider.symbol), "ConsoleProvider");
}

#[test]
fn resolves_top_level_requirement_provider_selection_to_exact_machine_symbol() {
    let source = r#"
        pub boundary requirement InterruptAcknowledgement::complete();
        data LapicCompletion {}
        machine build(builder: &mut Build) {
            builder.select_provider<InterruptAcknowledgement::complete, LapicCompletion>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize top-level provider selection");
    let syntax = parse_syntax_trees(&tokens).expect("parse top-level provider selection");
    let program =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve top-level provider selection");
    let requirement = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level requirement");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let call = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                provider_selection_expression(&program, *expression)
            }
            _ => None,
        })
        .expect("provider-selection expression call with operand custody");
    let [subject, provider] = call.machine_arguments.as_ref() else {
        panic!("two retained provider-selection arguments")
    };
    assert_eq!(subject.symbol, requirement.symbol);
    assert_eq!(
        program.symbols.get(subject.symbol).kind,
        symbols::SymbolKind::Machine
    );
    assert_eq!(
        program.symbols.get(provider.symbol).kind,
        symbols::SymbolKind::Data
    );
}

#[test]
fn authored_build_selection_paths_cannot_fall_back_to_extension_declarations() {
    let base = r#"
        machine build(builder: &mut Build) {
            builder.select_provider<GeneratedBoundary, GeneratedProvider>();
            builder.select_representation<GeneratedOpaque, GeneratedRepresentation>();
        }
    "#;
    let extension = r#"
        boundary trait GeneratedBoundary { machine enter(); }
        data GeneratedProvider {}
        data GeneratedOpaque {}
        trait Shape {}
        GeneratedRepresentation: GeneratedOpaque satisfies Shape {}
    "#;
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
        .add(PathBuf::from("main.omg"), base.to_owned())
        .source_id;
    let extension_tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize extension");
    let mut syntax = parse_syntax_trees_with_id(extension_id, &extension_tokens)
        .expect("parse extension source");
    let base_tokens = Lexer::new(base).tokenize().expect("tokenize base");
    let base_syntax = parse_syntax_trees_with_id(base_id, &base_tokens).expect("parse base source");
    syntax.extend_from(&base_syntax);

    let program = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolved lowering retains invalid hidden build selections for diagnostics");
    let build = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = program.machine_state(program.machine_state_handles(build.states)[0]);
    let calls = program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::Call(call) => {
                Some((call.target.as_str(), call.machine_arguments.as_ref()))
            }
            symbol_resolved_trees::statement::StatementNode::Expression(expression) => {
                provider_selection_expression(&program, *expression)
                    .map(|call| (call.target.as_str(), call.machine_arguments.as_ref()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "select_provider");
    assert_eq!(calls[1].0, "select_representation");
    for (target, arguments) in calls {
        assert!(
            arguments.iter().all(|argument| !argument.symbol.is_valid()),
            "authored `{}` arguments must not resurrect extension declarations: {:?}",
            target,
            arguments
        );
    }
}

fn provider_selection_expression(
    program: &symbol_resolved_trees::SymbolResolvedTrees,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<&symbol_resolved_trees::expression::TableCallExpression> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
        AuthoredDeclarationSelectionTarget,
    };
    let expressions = &program.tables.bodies.expressions;
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expressions.expression(expression)
    else {
        return None;
    };
    if call.target.as_str() != "select_provider" {
        return None;
    }
    let selections = expressions
        .authored_selection_occurrences(expression)
        .map(|occurrence| {
            program
                .authored_declaration_selections()
                .get(occurrence)
                .expect("retained expression occurrence")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selections
            .iter()
            .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
            .count(),
        1
    );
    let operands = selections
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::StaticArgument)
        .collect::<Vec<_>>();
    assert_eq!(operands.len(), 2);
    assert!(operands.iter().all(|selection| selection.target()
        == AuthoredDeclarationSelectionTarget::LateBound(
            AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument
        )));
    Some(call)
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
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
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
    assert_eq!(conformance.carrier_symbol, parameters[0].symbol);
    let converter = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Converter")
        .expect("Converter trait");
    assert_eq!(conformance.trait_symbol, converter.symbol);

    let source_argument = program
        .tables
        .declarations
        .child_type_references
        .span_or_empty(conformance.arguments)
        .first()
        .expect("Source trait argument");
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = source_argument
    else {
        panic!("Source should remain a named reference");
    };
    assert_eq!(name.as_str(), "Source");
    assert_eq!(*symbol, parameters[0].symbol);

    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameters[2].kind
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
    let symbol_resolved_trees::types::TypeReference::Named { symbol, name } =
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
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve nominal requirement");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = program
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let symbol_resolved_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
    else {
        panic!("Selected should be a machine parameter");
    };
    let symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
        authored_path,
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
    assert_eq!(
        authored_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["WindowProcedure", "call"]
    );
    assert_ne!(parameter.symbol, requirement_row.symbol);
    let symbol_resolved_trees::data::MachineParameterContractView::Nominal {
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
    let diagnostic = resolve(ResolutionRequest::new(&syntax)).expect_err("overload must reject");

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
        let diagnostic =
            resolve(ResolutionRequest::new(&syntax)).expect_err("unknown path must reject");
        assert!(
            diagnostics::format_diagnostics(&diagnostic).contains(expected),
            "unexpected diagnostic for {path}: {diagnostic:?}"
        );
    }
}
