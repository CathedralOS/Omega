//! Constant resolution tests.

use super::{Diagnostic, SyntaxTrees};
use crate::constant::public_declaration_value_encoding;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
use source::SourceId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::StatementNode;
use symbols::SymbolKind;
use syntax_trees::item::Item;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn resolve(sources: &[&str]) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let mut syntax = SyntaxTrees::default();
    for (ordinal, text) in sources.iter().enumerate() {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize module constants");
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(ordinal), &tokens)
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
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(0), &tokens)
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
fn scoped_array_attachment_uses_the_exact_module_across_sources() {
    let sources = [
        "data Sizes { case SIZE; }",
        "module settings; data Sizes {}",
        "module settings; const Sizes::SIZE: [u8; 1] = [1];",
    ];
    for ordered in [sources, [sources[2], sources[1], sources[0]]] {
        let program = resolve(&ordered).expect("one exact carrier across module sources");
        let selection = program.authored_declaration_selections().iter().find(|selection| {
            matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                if program.symbols.display_path(target.selected_symbol(), "::") == "settings::Sizes")
        }).expect("attachment selects the module carrier");
        let text = ordered[selection.source_span().source_id.0];
        let span = selection.source_span().span;
        assert_eq!(&text[span.start..span.end], "Sizes");
    }
    resolve(&[
        "module settings; data Sizes {} const Sizes::SIZE: [u8; 1] = [1];",
        "module settings; const Sizes::SIZE: [u8; 1] = [1];",
    ])
    .expect_err("duplicate scoped declarations across sources reject");
}

#[test]
fn scoped_array_attachment_selects_a_foreign_module_carrier() {
    let declaring = "module settings; use other::Sizes; const Sizes::SIZE: [u8; 1] = [1];";
    let program = resolve(&["module other; data Sizes {}", declaring])
        .expect("an exact import exposes the foreign-module carrier");
    // The `use` import records its own `other::Sizes` selection; the
    // attachment's record must sit exactly on the scope token.
    let selection = program
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            let span = selection.source_span().span;
            selection.source_span().source_id == SourceId(1)
                && &declaring[span.start..span.end] == "Sizes"
                && matches!(
                    selection.target(),
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if program.symbols.display_path(target.selected_symbol(), "::") == "other::Sizes")
        })
        .expect("attachment retains the foreign-carrier selection on the scope token");
    let span = selection.source_span().span;
    assert_eq!(&declaring[span.start..span.end], "Sizes");
}

#[test]
fn scoped_attachment_prefers_the_module_local_carrier_over_imports() {
    let program = resolve(&[
        "module other; data Sizes {}",
        "module settings; data Sizes {} use other::Sizes; const Sizes::SIZE: [u8; 1] = [1];",
    ])
    .expect("the declaring module's own carrier outranks the import");
    assert!(
        program
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                matches!(
                    selection.target(),
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if program.symbols.display_path(target.selected_symbol(), "::") == "settings::Sizes")
            }),
        "attachment selects the module-local carrier"
    );
}

#[test]
fn scoped_attachment_falls_back_to_the_unmoduled_carrier() {
    let program = resolve(&[
        "data Sizes {}",
        "module settings; const Sizes::SIZE: [u8; 1] = [1];",
    ])
    .expect("an unmoduled carrier remains reachable from a module scope");
    assert!(
        program
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                matches!(
                    selection.target(),
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                        if program.symbols.display_path(target.selected_symbol(), "::") == "Sizes")
            }),
        "attachment selects the unmoduled carrier"
    );
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
fn module_constants_select_exact_local_qualified_and_imported_values() {
    let program = resolve(&[
        "module combat; pub const DAMAGE: u64 = 7; machine combat_value() -> u64 { let observed: u64 = DAMAGE; observed }",
        "module rooms; pub const DAMAGE: u64 = 9; machine room_value() -> u64 { let observed: u64 = DAMAGE; observed }",
        "use combat::DAMAGE; use rooms; machine imported() -> u64 { let observed: u64 = DAMAGE; observed } machine qualified() -> u64 { let observed: u64 = rooms::DAMAGE; observed }",
    ]).expect("resolve distinct module constants");
    for (machine, expected) in [
        ("combat_value", 7),
        ("room_value", 9),
        ("imported", 7),
        ("qualified", 9),
    ] {
        let expression = local_value(&program, machine, "observed");
        let ExpressionNode::Integer(value) =
            program.tables.bodies.expressions.expression(expression)
        else {
            panic!("substituted scalar constant");
        };
        assert_eq!(value.value_u64(), Some(expected));
        assert_eq!(
            program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .count(),
            1
        );
    }
    let declarations = program
        .roots
        .const_declarations
        .iter()
        .map(|declaration| program.symbols.display_path(declaration.symbol, "::"))
        .collect::<Vec<_>>();
    assert_eq!(declarations, ["combat::DAMAGE", "rooms::DAMAGE"]);
}

#[test]
fn module_constant_yields_to_state_parameter_and_preceding_local() {
    let program = resolve(&["module combat; const DAMAGE: u64 = 7;
        machine parameter(DAMAGE: u64) -> u64 { let observed: u64 = DAMAGE; observed }
        machine local() -> u64 { let before: u64 = DAMAGE; let DAMAGE: u64 = 12; let after: u64 = DAMAGE; after }
        machine initialize() -> u64 { let DAMAGE: u64 = DAMAGE; let observed: u64 = DAMAGE; observed }"]).expect("lexical bindings shadow constants");
    for (machine, local) in [
        ("parameter", "observed"),
        ("local", "after"),
        ("initialize", "observed"),
    ] {
        let expression = local_value(&program, machine, local);
        let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
        else {
            panic!("lexical value remains a Name");
        };
        assert!(matches!(
            program.symbols.get(path.symbol).kind,
            SymbolKind::Local | SymbolKind::Parameter
        ));
    }
    let ExpressionNode::Integer(value) = program
        .tables
        .bodies
        .expressions
        .expression(local_value(&program, "local", "before"))
    else {
        panic!("earlier constant use");
    };
    assert_eq!(value.value_u64(), Some(7));
    let ExpressionNode::Integer(value) =
        program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "initialize", "DAMAGE"))
    else {
        panic!("initializer selects preceding constant");
    };
    assert_eq!(value.value_u64(), Some(7));
}

#[test]
fn module_constant_yields_to_parser_generated_inferred_local() {
    let tokens = Lexer::new("module combat; const DAMAGE: u64 = 7; machine inferred() -> u64 { let DAMAGE: u64 = 12; let observed: u64 = DAMAGE; observed }")
        .tokenize().expect("tokenize inferred-local control");
    let mut syntax = SyntaxTrees::default();
    parse_syntax_trees_into_with_id(&mut syntax, SourceId(0), &tokens)
        .expect("parse inferred-local control");
    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("inferred machine");
    let state = syntax
        .items
        .state(syntax.items.state_handles(machine.states)[0]);
    let statement = syntax.items.statements(state.statements)[0];
    let syntax_trees::statement::StatementNode::LocalData(mut local) =
        syntax.statements.statement(statement).clone()
    else {
        panic!("first local");
    };
    // Parser-generated destructuring locals use this same absent-type
    // representation; no new inferred-let source syntax is introduced.
    local.type_reference = syntax_trees::types::TypeReferenceHandle::invalid();
    syntax.statements.replace_statement(
        statement,
        syntax_trees::statement::StatementNode::LocalData(local),
    );
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("resolve inferred local shadow");
    let ExpressionNode::Name(path) = program
        .tables
        .bodies
        .expressions
        .expression(local_value(&program, "inferred", "observed"))
    else {
        panic!("inferred local reference remains lexical");
    };
    assert_eq!(program.symbols.get(path.symbol).kind, SymbolKind::Local);
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
fn qualified_nominal_constants_keep_declaring_constructors_and_fresh_children() {
    let mut program = resolve(&[
        "module settings; pub data Leaf [copy] { count: u64; } pub data Value [copy] { leaf: Leaf; enabled: bool; }
         pub const VALUE: Value = Value { enabled: true, leaf: Leaf { count: 1 } };",
        "use settings; data Leaf [copy] { count: u64; } data Value [copy] { leaf: Leaf; enabled: bool; }
         machine keep() -> settings::Value {
            let first: settings::Value = settings::VALUE;
            let second: settings::Value = settings::VALUE;
            second
         }",
    ]).expect("qualified nominal body copies");
    let first = local_value(&program, "keep", "first");
    let second = local_value(&program, "keep", "second");
    let ExpressionNode::StructLiteral(first_literal) =
        program.tables.bodies.expressions.expression(first).clone()
    else {
        panic!("first nominal value");
    };
    let ExpressionNode::StructLiteral(second_literal) =
        program.tables.bodies.expressions.expression(second).clone()
    else {
        panic!("second nominal value");
    };
    assert_eq!(
        program
            .symbols
            .display_path(first_literal.type_symbol, "::"),
        "settings::Value"
    );
    assert_eq!(first_literal.type_symbol, second_literal.type_symbol);
    assert_ne!(first_literal.fields, second_literal.fields);
    let first_fields = program
        .tables
        .bodies
        .expressions
        .struct_fields(first_literal.fields)
        .to_vec();
    let second_fields = program
        .tables
        .bodies
        .expressions
        .struct_fields(second_literal.fields)
        .to_vec();
    for (first_field, second_field) in first_fields.iter().zip(&second_fields) {
        assert_eq!(first_field.field_symbol, second_field.field_symbol);
        assert!(first_field.field_symbol.is_valid());
        assert_ne!(first_field.value, second_field.value);
    }
    let ExpressionNode::StructLiteral(leaf) = program
        .tables
        .bodies
        .expressions
        .expression(first_fields[1].value)
    else {
        panic!("nested leaf");
    };
    assert_eq!(
        program.symbols.display_path(leaf.type_symbol, "::"),
        "settings::Leaf"
    );
    let unchanged = program
        .tables
        .bodies
        .expressions
        .expression(second_fields[0].value)
        .clone();
    *program
        .tables
        .bodies
        .expressions
        .expression_mut(first_fields[0].value) = ExpressionNode::Boolean(false);
    assert_eq!(
        program
            .tables
            .bodies
            .expressions
            .expression(second_fields[0].value),
        &unchanged
    );
    for root in [first, second] {
        let kinds = program
            .tables
            .bodies
            .expressions
            .authored_selection_occurrences(root)
            .map(|occurrence| {
                program
                    .authored_declaration_selections()
                    .iter()
                    .find(|selection| selection.occurrence_id() == occurrence)
                    .expect("retained selection")
                    .kind()
            })
            .collect::<Vec<_>>();
        assert!(kinds.contains(&AuthoredDeclarationSelectionKind::StructLiteralType));
        assert!(kinds.contains(&AuthoredDeclarationSelectionKind::StructLiteralField));
        let occurrences = program
            .tables
            .bodies
            .expressions
            .authored_selection_occurrences(root)
            .map(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let constant_uses = occurrences.iter().filter(|selection| {
            let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else { return false; };
            program.symbols.display_path(target.selected_symbol(), "::") == "settings::VALUE"
        }).collect::<Vec<_>>();
        assert_eq!(constant_uses.len(), 1);
        assert_eq!(
            constant_uses[0].kind(),
            AuthoredDeclarationSelectionKind::StaticPathSegment
        );
        assert_eq!(constant_uses[0].source_span().source_id, SourceId(1));
        for selection in occurrences.iter().filter(|selection| {
            matches!(
                selection.kind(),
                AuthoredDeclarationSelectionKind::StructLiteralType
                    | AuthoredDeclarationSelectionKind::StructLiteralField
            )
        }) {
            assert_eq!(selection.source_span().source_id, SourceId(0));
            let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else { panic!("declaring constructor selection resolved"); };
            assert!(
                program
                    .symbols
                    .display_path(target.selected_symbol(), "::")
                    .starts_with("settings::Value")
            );
        }
    }
}

#[test]
fn root_nominal_constants_share_lexical_selection_and_fresh_materialization() {
    let program = resolve(&["data Value [copy] { value:u64; }
        const VALUE:Value = Value { value:7 };
        machine keep()->Value {
            let before:Value = VALUE;
            let VALUE:Value = VALUE;
            let after:Value = VALUE;
            after
        }"])
    .expect("root nominal values use ordinary lexical selection");
    let before = local_value(&program, "keep", "before");
    let initializer = local_value(&program, "keep", "VALUE");
    let after = local_value(&program, "keep", "after");
    let ExpressionNode::StructLiteral(first) = program.tables.bodies.expressions.expression(before)
    else {
        panic!("earlier constant use");
    };
    let ExpressionNode::StructLiteral(second) =
        program.tables.bodies.expressions.expression(initializer)
    else {
        panic!("self initializer selects prior constant");
    };
    assert_eq!(first.type_symbol, second.type_symbol);
    assert_ne!(first.fields, second.fields);
    let ExpressionNode::Name(local) = program.tables.bodies.expressions.expression(after) else {
        panic!("later use selects local");
    };
    assert_eq!(program.symbols.get(local.symbol).kind, SymbolKind::Local);
}

#[test]
fn ordinary_module_presence_keeps_root_aggregate_substitution() {
    let program = resolve(&[
        "module other; data Marker {}",
        "data Pair { value: u64; } const PAIR: Pair = Pair { value: 7 }; machine read() -> Pair { let observed: Pair = PAIR; observed }",
    ]).expect("unrelated module declarations do not change aggregate substitution");
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "read", "observed")),
        ExpressionNode::StructLiteral(_)
    ));
}

#[test]
fn root_scalar_lexical_selection_coexists_with_aggregate_materialization() {
    let program = resolve(&["data Pair { value: u64; }
        const PAIR: Pair = Pair { value: 11 };
        machine read() -> u64 {
            let aggregate: Pair = PAIR;
            let before: u64 = SIZE;
            let SIZE: u64 = SIZE;
            let after: u64 = SIZE;
            after
        }
        machine parameter(SIZE: u64) -> u64 { let observed: u64 = SIZE; observed }
        const SIZE: u64 = 7;"])
    .expect("forward scalar selection preserves lexical scope beside an aggregate");
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "read", "aggregate")),
        ExpressionNode::StructLiteral(_)
    ));
    for local in ["before", "SIZE"] {
        let expression = local_value(&program, "read", local);
        let ExpressionNode::Integer(value) =
            program.tables.bodies.expressions.expression(expression)
        else {
            panic!("earlier use and self-initializer select the forward constant");
        };
        assert_eq!(value.value_u64(), Some(7));
        assert_eq!(
            program
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(expression)
                .count(),
            1
        );
    }
    for (machine, local, kind) in [
        ("read", "after", SymbolKind::Local),
        ("parameter", "observed", SymbolKind::Parameter),
    ] {
        let expression = local_value(&program, machine, local);
        let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
        else {
            panic!("actual lexical binding remains a name");
        };
        assert_eq!(program.symbols.get(path.symbol).kind, kind);
    }
}

#[test]
fn root_scalar_and_explicit_receiver_field_remain_distinct() {
    let program = resolve(&["const SIZE: u64 = 7; data Main { SIZE: u64; }
        machine Main::read(&self) -> u64 { let field: u64 = self.SIZE; let constant: u64 = SIZE; constant }"])
        .expect("bare fields do not alias explicit receiver projections");
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "Main::read", "field")),
        ExpressionNode::Member(_)
    ));
    let ExpressionNode::Integer(value) = program.tables.bodies.expressions.expression(local_value(
        &program,
        "Main::read",
        "constant",
    )) else {
        panic!("bare spelling selects the constant");
    };
    assert_eq!(value.value_u64(), Some(7));
}

#[test]
fn explicit_receiver_field_is_not_a_module_constant_reference() {
    let program = resolve(&[
        "module combat; const DAMAGE: u64 = 7; data Main { DAMAGE: u64; }
         machine Main::field(&self) -> u64 { let observed: u64 = self.DAMAGE; observed }
         machine Main::value(&self) -> u64 { let observed: u64 = DAMAGE; observed }",
    ])
    .expect("explicit receiver projection and bare constant have distinct meanings");
    assert!(matches!(
        program.tables.bodies.expressions.expression(local_value(
            &program,
            "Main::field",
            "observed"
        )),
        ExpressionNode::Member(_)
    ));
    let ExpressionNode::Integer(value) = program.tables.bodies.expressions.expression(local_value(
        &program,
        "Main::value",
        "observed",
    )) else {
        panic!("bare spelling selects module constant, not implicit field");
    };
    assert_eq!(value.value_u64(), Some(7));
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

#[test]
fn root_case_collisions_still_reject_in_module_constant_closures() {
    let errors = resolve(&[
        "module combat; const DAMAGE: u64 = 7;",
        "data Choice { case None; } const Choice::None: u64 = 1;",
    ])
    .expect_err("const cannot shadow an exact case constructor");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("collides with the case"))
    );
}

#[test]
fn colliding_imported_constant_leaves_do_not_choose_traversal_order() {
    let program = resolve(&[
        "module combat; pub const DAMAGE: u64 = 7;",
        "module rooms; pub const DAMAGE: u64 = 9;",
        "use combat::DAMAGE; use rooms::DAMAGE; machine ambiguous() -> u64 { let observed: u64 = DAMAGE; observed }",
    ]).expect("resolution preserves an unresolved ambiguous value for checking");
    assert!(matches!(
        program.tables.bodies.expressions.expression(local_value(
            &program,
            "ambiguous",
            "observed"
        )),
        ExpressionNode::Name(_)
    ));
}

fn resolve_seeded(
    extension: &str,
) -> Result<crate::resolution::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    resolve_seeded_with_base(
        "module combat; pub const DAMAGE: u64 = 7;",
        extension,
        |_| {},
    )
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
    parse_syntax_trees_into_with_id(&mut syntax, base_source, &tokens)
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
    parse_syntax_trees_into_with_id(&mut syntax, extension_source, &tokens)
        .expect("parse constant extension");
    crate::resolution::resolve_extension(crate::resolution::ExtensionRequest {
        base,
        syntax: &syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
}

#[test]
fn seeded_module_constant_references_copy_the_retained_value() {
    let extension =
        resolve_seeded("machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }")
            .expect("extension copies the exact retained base constant");
    let ExpressionNode::Integer(value) = extension
        .trees()
        .tables
        .bodies
        .expressions
        .expression(local_value(extension.trees(), "read", "observed"))
    else {
        panic!("retained scalar initializer");
    };
    assert_eq!(value.value_u64(), Some(7));
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

#[test]
fn seeded_raw_constant_spelling_cannot_erase_module_ambiguity() {
    let extension = resolve_seeded("const combat::DAMAGE: u64 = 9; machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }").expect("ambiguous path remains unresolved");
    let program = extension.trees();
    assert!(matches!(
        program
            .tables
            .bodies
            .expressions
            .expression(local_value(program, "read", "observed")),
        ExpressionNode::Name(_)
    ));
}
