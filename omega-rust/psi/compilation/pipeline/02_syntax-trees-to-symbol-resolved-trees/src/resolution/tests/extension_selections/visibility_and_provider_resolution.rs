use crate::resolution::{ResolutionRequest, resolve};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

#[test]
fn retains_public_data_trait_and_wire_visibility() {
    let source = r#"
        pub data PublicRecord { value: u32; }
        data PrivateRecord {}
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
    let private = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "PrivateRecord")
        .expect("private data");
    assert!(!private.is_public);
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
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::CheckedBody
    );
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
