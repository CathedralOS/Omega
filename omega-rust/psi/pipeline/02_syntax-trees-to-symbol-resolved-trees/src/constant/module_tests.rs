//! Constant resolution tests.

use super::{Diagnostic, SyntaxTrees};
use crate::constant::public_declaration_value_encoding;
use source::SourceId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::StatementNode;
use symbols::SymbolKind;
use syntax_trees::item::Item;

fn resolve(sources: &[&str]) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let mut syntax = SyntaxTrees::default();
    for (ordinal, text) in sources.iter().enumerate() {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize module constants");
        tokens_to_syntax_trees::parse(&mut syntax, SourceId(ordinal), &tokens)
            .expect("parse module constants");
    }
    crate::resolve(crate::ResolutionRequest::new(&syntax))
}

#[test]
fn public_float_identity_rejects_payloadless_nan() {
    for carrier in ["f32", "f64"] {
        let source = format!("pub const VALUE: {carrier} = 1.0;");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize float constant");
        let mut syntax = SyntaxTrees::default();
        tokens_to_syntax_trees::parse(&mut syntax, SourceId(0), &tokens)
            .expect("parse float constant");
        let Item::Const(definition) = syntax.root_items().next().expect("one declaration").clone()
        else {
            panic!("expected one constant declaration");
        };
        // Evaluator-produced NaN meaning carries no selected payload bits.
        syntax.expressions.replace_expression(
            definition.value,
            syntax_trees::expression::ExpressionNode::Float(source::SourceText::new(
                "NaN",
                definition.name.source_span(),
            )),
        );
        let error = public_declaration_value_encoding(&syntax, &definition, None)
            .expect_err("payloadless NaN cannot acquire public representation identity");
        assert!(
            error.contains("explicit NaN representation bits"),
            "{error}"
        );
    }
}

#[test]
fn unused_scalar_declarations_check_landing_before_substitution() {
    for namespace in ["", "module settings;"] {
        for (carrier, value, accepted) in [
            ("u8", "256", false),
            ("u8", "1u64", false),
            ("bool", "1", false),
            ("u64", "true", false),
            ("f32", "1.0f64", false),
            ("f32", "1u32", false),
            ("u8", "255", true),
            ("i8", "-128", true),
            ("bool", "true", true),
            ("f32", "1.0f32", true),
            ("f64", "1", true),
        ] {
            let source = format!("{namespace} const VALUE: {carrier} = {value};");
            let result = resolve(&[&source]);
            assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
        }
    }
}

#[test]
fn unused_array_declarations_check_shape_and_landing() {
    for namespace in ["", "module settings;"] {
        for visibility in ["", "pub "] {
            for (carrier, value, accepted) in [
                ("[u8; 2]", "[1]", false),
                ("[u8; 1]", "[256]", false),
                ("[u8; 1]", "[1u64]", false),
                ("[bool; 1]", "[1]", false),
                ("[[u8; 2]; 1]", "[[1]]", false),
                ("[u8; 2]", "[0, 255]", true),
                ("[[bool; 2]; 1]", "[[true, false]]", true),
                ("[u8; 0]", "[]", true),
            ] {
                let source = format!("{namespace} {visibility}const VALUE: {carrier} = {value};");
                let result = resolve(&[&source]);
                assert_eq!(result.is_ok(), accepted, "{source}: {:?}", result.err());
            }
        }
    }
}

fn local_value(
    program: &SymbolResolvedTrees,
    machine_name: &str,
    local_name: &str,
) -> ExpressionHandle {
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("constant test machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("constant test local")
}

#[test]
fn scoped_attachment_rejects_ambiguous_or_colliding_foreign_carriers() {
    for (sources, expected) in [
        (
            vec![
                "module a; data Sizes {}",
                "module b; data Sizes {}",
                "module settings; use a::Sizes; use b::Sizes; const Sizes::SIZE: [u8; 1] = [1];",
            ],
            "ambiguous",
        ),
        (
            vec![
                "module other; data Sizes { case SIZE; }",
                "module settings; use other::Sizes; const Sizes::SIZE: [u8; 1] = [1];",
            ],
            "collides with the case",
        ),
    ] {
        let diagnostics = resolve(&sources).expect_err("the carrier selection must stay exact");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expected}: {diagnostics:?}"
        );
    }
}

#[test]
fn scoped_array_attachment_cannot_borrow_a_generic_or_absent_owner() {
    for (sources, expected) in [
        (
            vec!["module settings; data Sizes<T> {} const Sizes::SIZE: [u8; 1] = [1];"],
            "exact nongeneric data carrier",
        ),
        (
            // The module-local generic carrier claims `Sizes`; the scope
            // head cannot shadow it away to the imported nongeneric owner.
            vec![
                "module other; data Sizes {}",
                "module settings; data Sizes<T> {} use other::Sizes; const Sizes::SIZE: [u8; 1] = [1];",
            ],
            "exact nongeneric data carrier",
        ),
        (
            vec!["module settings; const Sizes::SIZE: [u8; 1] = [1];"],
            "exact nongeneric data carrier",
        ),
    ] {
        let diagnostics = resolve(&sources).expect_err("attachment normalization is incomplete");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn same_module_duplicate_constants_reject_across_files() {
    let errors = resolve(&[
        "module combat; const DAMAGE: u64 = 7;",
        "module combat; const DAMAGE: u64 = 9;",
    ])
    .expect_err("duplicate exact module constant");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("duplicate const"))
    );
}

#[test]
fn conformance_parameter_cannot_be_replaced_by_module_constant() {
    let program = resolve(&[
        "trait Ranked {}",
        "module combat; const Order: u64 = 7; machine sort<Element, Order: Element satisfies Ranked>(values: &mut [Element]) -> u64 { let observed: u64 = Order; observed }",
    ]).expect("resolution preserves the proof-static machine binder");
    let ExpressionNode::Name(path) = program
        .tables
        .bodies
        .expressions
        .expression(local_value(&program, "sort", "observed"))
    else {
        panic!("conformance binder is not a constant value");
    };
    assert_eq!(
        program.symbols.get(path.symbol).kind,
        SymbolKind::ConformanceParameter
    );
}

#[test]
fn module_scalar_substitution_preserves_boolean_float_and_text_kinds() {
    let program = resolve(&[
        "module boolean; const VALUE: bool = true; machine boolean_value() -> bool { let observed: bool = VALUE; observed }",
        "module floating; const VALUE: f64 = 1.5; machine float_value() -> f64 { let observed: f64 = VALUE; observed }",
        "module text; const VALUE: string = \"value\"; machine text_value() -> string { let observed: string = VALUE; observed }",
    ]).expect("primitive literal kinds retain their module selections");
    assert!(matches!(
        program.tables.bodies.expressions.expression(local_value(
            &program,
            "boolean_value",
            "observed"
        )),
        ExpressionNode::Boolean(true)
    ));
    assert!(matches!(
        program.tables.bodies.expressions.expression(local_value(
            &program,
            "float_value",
            "observed"
        )),
        ExpressionNode::Float(_)
    ));
    assert!(matches!(
        program.tables.bodies.expressions.expression(local_value(
            &program,
            "text_value",
            "observed"
        )),
        ExpressionNode::String(_)
    ));
}

fn resolve_seeded_with_base(
    base_text: &str,
    extension: &str,
    change_base: impl FnOnce(&mut SymbolResolvedTrees),
) -> Result<crate::resolution::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    use std::{path::PathBuf, sync::Arc};
    let mut sources = source::SourceMap::default();
    let base_source = sources
        .add(PathBuf::from("combat.omg"), base_text.to_owned())
        .source_id;
    let tokens = Lexer::new(base_text)
        .tokenize()
        .expect("tokenize retained constant");
    let mut syntax = SyntaxTrees::default();
    tokens_to_syntax_trees::parse(&mut syntax, base_source, &tokens)
        .expect("parse retained constant");
    let mut base = crate::resolve(crate::ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve retained constant");
    change_base(&mut base);
    let extension_source = sources
        .add(PathBuf::from("extension.omg"), extension.to_owned())
        .source_id;
    let tokens = Lexer::new(extension)
        .tokenize()
        .expect("tokenize constant extension");
    let mut syntax = SyntaxTrees::default();
    tokens_to_syntax_trees::parse(&mut syntax, extension_source, &tokens)
        .expect("parse constant extension");
    crate::resolution::resolve_extension(crate::resolution::ExtensionRequest {
        base,
        syntax: &syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
}

#[test]
fn seeded_constant_initializer_requires_live_exact_declaration_root() {
    for mutation in ["missing", "stale", "out of bounds", "other declaration"] {
        let errors = resolve_seeded_with_base(
            "module combat; pub const DAMAGE: u64 = 7; pub const OTHER: u64 = 9;",
            "machine read() -> u64 { combat::DAMAGE }",
            |base| {
                let initializer = base.roots.const_declarations[0].initializer;
                base.roots.const_declarations[0].initializer = match mutation {
                    "missing" => ExpressionHandle::invalid(),
                    "stale" => ExpressionHandle::from_parts(
                        initializer.arena_index(),
                        initializer.generation() + 1,
                    ),
                    "out of bounds" => ExpressionHandle::from_arena_index(u32::MAX),
                    "other declaration" => base.roots.const_declarations[1].initializer,
                    _ => unreachable!(),
                };
            },
        )
        .expect_err("invalid initializer custody must diagnose before arena side-table access");
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("constant substitution lost its exact declaration initializer")),
            "{mutation}: {errors:?}"
        );
    }
}
