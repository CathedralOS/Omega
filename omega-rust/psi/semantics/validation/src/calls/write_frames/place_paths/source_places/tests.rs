use super::*;
use arena::HandleSpan;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::expression::{
    ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use typed_trees::name::Identifier;

fn name(program: &mut TypedTrees, spelling: &str, symbol: SymbolHandle) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated(spelling));
    let mut member_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, symbol);
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            head_symbol: symbol,
            symbol,
        }))
}

fn field_after_index(
    program: &mut TypedTrees,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    field: SymbolHandle,
) -> (ExpressionHandle, ExpressionHandle) {
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));
    let whole = program
        .expression_table
        .insert(ExpressionNode::Member(TableMemberExpression {
            receiver: indexed,
            member_symbol: field,
            member: Identifier::generated("view"),
            case_variant: None,
        }));
    (indexed, whole)
}

fn fixture() -> (TypedTrees, Vec<SymbolHandle>) {
    let mut symbols = SymbolTableBuilder::default();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = symbols.insert_children(
        root,
        [
            (SymbolKind::Local, SymbolNameRef::Static("values")),
            (SymbolKind::Local, SymbolNameRef::Static("index")),
            (SymbolKind::Field, SymbolNameRef::Static("view")),
            (SymbolKind::Parameter, SymbolNameRef::Static("caller")),
        ],
    );
    let handles = SymbolTableBuilder::child_handles(children).collect();
    (
        TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        },
        handles,
    )
}

fn typed_source(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("builtin_coordinates.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"))
}

fn selected_expression(program: &TypedTrees) -> (&Machine, &State, ExpressionHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("source machine");
    let state = &program.machine_states(machine)[0];
    let expression = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "selected" => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("source initializer");
    (machine, state, expression)
}

fn check_source_coordinates(source: &str, expected: bool) {
    let program = typed_source(source);
    let (machine, state, expression) = selected_expression(&program);
    assert!(matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Indexed(_)
    ));
    assert_eq!(
        crate::place_has_builtin_coordinates(&program, machine, Some(state), expression),
        expected,
        "query: {source}"
    );
    let candidate = FrameSourcePlace::from_expression(&program, expression);
    assert_eq!(candidate.builtin_coordinates, expected, "source: {source}");
    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(&program, expression);
    let place = facts.places.get(place);
    assert!(candidate.root.is_valid());
    assert_eq!(PlaceRoot::Symbol(candidate.root), place.root);
    assert_eq!(
        candidate.segments,
        facts.place_segments.span_or_empty(place.segments),
        "guard failures retain candidate geometry: {source}"
    );
}

#[test]
fn authored_index_meaning_gates_precision_without_discarding_candidates() {
    for collection in ["[i32; 3]", "&mut [i32; 3]"] {
        for (declaration, expected) in [
            ("", true),
            (
                "boundary operator [] Collection::read(items: &[i32], index: u64) -> i32;",
                false,
            ),
            (
                "boundary operator [] Collection::read(items: &[u8], index: u64) -> u8;",
                true,
            ),
            (
                "boundary operator [] Collection::read(items: &[i32], index: i32) -> i32;",
                true,
            ),
        ] {
            check_source_coordinates(
                &format!(
                    "{declaration}
                     machine inspect(items: {collection}, index: u64) {{
                         let selected: i32 = items[index];
                     }}"
                ),
                expected,
            );
        }
    }
}

#[test]
fn selector_arithmetic_meaning_gates_runtime_and_foldable_candidates() {
    // Parameter-only arithmetic is hoisted before typing. Local operands
    // keep the Binary selector visible, so this pins its meaning gate.
    for selector in ["left + right", "0u64 + 1u64"] {
        for (declaration, expected) in [
            ("", true),
            (
                "operator + u64::custom(left: u64, right: u64) -> u64;",
                false,
            ),
            (
                "operator + f64::unrelated(left: f64, right: f64) -> f64;",
                true,
            ),
        ] {
            check_source_coordinates(
                &format!(
                    "{declaration}
                     machine inspect(items: &mut [i32; 3], index: u64, offset: u64) {{
                         let left: u64 = index;
                         let right: u64 = offset;
                         let selected: i32 = items[{selector}];
                     }}"
                ),
                expected,
            );
        }
    }
}

#[test]
fn hoisted_authored_arithmetic_remains_a_dynamic_selector() {
    let source = "operator + u64::custom(left: u64, right: u64) -> u64;
        machine inspect(items: &mut [i32; 3], index: u64, offset: u64) {
            let selected: i32 = items[index + offset];
        }";
    let program = typed_source(source);
    let (machine, state, expression) = selected_expression(&program);
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("indexed source");
    };
    let ExpressionNode::Name(selector) = program.expression_table.expression(indexed.index) else {
        panic!("hoisted selector");
    };
    let temporary = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == selector.symbol => Some(local),
            _ => None,
        })
        .expect("selector's exact binding");
    assert!(!temporary.is_mutable);
    assert!(matches!(
        program.expression_table.expression(temporary.initial_value),
        ExpressionNode::Binary(_)
    ));
    assert!(!crate::has_builtin_bound_expression_meaning(
        &program,
        machine,
        Some(state),
        temporary.initial_value,
    ));
    // The query admits builtin indexing of the captured runtime integer;
    // it grants no permission to reinterpret its authored initializer.
    check_source_coordinates(source, true);
    let candidate = FrameSourcePlace::from_expression(&program, expression);
    assert_eq!(
        candidate.segments,
        vec![PlaceSegment::Index {
            expression: indexed.index,
        }]
    );
}

#[test]
fn source_composition_conjoins_permission_without_changing_candidates() {
    let (_, symbols) = fixture();
    for left in [false, true] {
        for right in [false, true] {
            let parent = FrameSourcePlace {
                root: symbols[0],
                segments: vec![PlaceSegment::FixedIndex { index: 1 }],
                builtin_coordinates: left,
            };
            let suffix = FrameSourcePlace {
                root: symbols[3],
                segments: vec![PlaceSegment::Field { symbol: symbols[2] }],
                builtin_coordinates: right,
            };
            for composed in [
                parent.append_source(&suffix),
                parent.append_relative(&suffix),
            ] {
                assert_eq!(composed.root, parent.root);
                assert_eq!(
                    composed.segments,
                    [parent.segments.as_slice(), suffix.segments.as_slice()].concat()
                );
                assert_eq!(composed.builtin_coordinates, left && right);
            }
            assert_eq!(
                parent.append_segments(&suffix.segments).builtin_coordinates,
                left,
                "schema selectors retain the parent's permission"
            );
        }
    }
}

#[test]
fn helper_result_projection_keeps_metadata_when_index_meaning_is_unknown() {
    for (source, expected) in [
        (
            "data Cell { value: i32; }
             machine identity(cell: &Cell) -> &Cell { cell }
             machine inspect(cell: &Cell) {
                 let selected: i32 = identity(cell).value;
             }",
            true,
        ),
        (
            "machine identity(items: &[i32; 3]) -> &[i32; 3] { items }
             machine inspect(items: &[i32; 3], index: u64) {
                 let selected: i32 = identity(items)[index];
             }",
            false,
        ),
    ] {
        let program = typed_source(source);
        let (_, state, whole) = selected_expression(&program);
        let base = match program.expression_table.expression(whole) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => panic!("source projection: {source}"),
        };
        assert!(matches!(
            program.expression_table.expression(base),
            ExpressionNode::Call(_)
        ));
        let origin = FrameSourcePlace {
            root: program.state_parameters(state)[0].symbol,
            segments: Vec::new(),
            builtin_coordinates: true,
        };
        let projected = origin.projected(&program, whole, base);
        assert_eq!(projected.root, origin.root);
        assert_eq!(projected.segments.len(), 1);
        assert_eq!(projected.builtin_coordinates, expected, "{source}");
    }
}

#[test]
fn nonindexed_sources_leave_declaration_checks_with_the_consumer() {
    let (mut program, symbols) = fixture();
    let expression = name(&mut program, "values", symbols[0]);
    assert_eq!(
        FrameSourcePlace::from_expression(&program, expression),
        FrameSourcePlace {
            root: symbols[0],
            segments: Vec::new(),
            builtin_coordinates: true,
        }
    );
}

#[test]
fn missing_owner_keeps_candidate_coordinates_without_builtin_permission() {
    let (mut program, symbols) = fixture();
    let collection = name(&mut program, "values", symbols[0]);
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let (_, expression) = field_after_index(&mut program, collection, index, symbols[2]);
    let origin = super::super::frame_place_path(&program, expression).expect("coarse place");
    assert_eq!(origin.path, "values");
    assert_eq!(origin.source.root, symbols[0]);
    assert_eq!(
        origin.source.segments,
        vec![
            PlaceSegment::FixedIndex { index: 1 },
            PlaceSegment::Field { symbol: symbols[2] },
        ]
    );
    assert!(!origin.source.builtin_coordinates);
}

#[test]
fn coarse_write_path_retains_structural_field_after_index() {
    for runtime in [false, true] {
        let (mut program, symbols) = fixture();
        let collection = name(&mut program, "values", symbols[0]);
        let index = if runtime {
            name(&mut program, "index", symbols[1])
        } else {
            program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(1),
            ))
        };
        let (_, expression) = field_after_index(&mut program, collection, index, symbols[2]);
        let origin = super::super::frame_place_path(&program, expression).expect("place");
        assert_eq!(origin.path, "values");
        assert_eq!(
            origin.precision,
            super::super::FramePathPrecision::CollectionCoarse
        );
        assert_eq!(origin.source.root, symbols[0]);
        assert_eq!(
            origin.source.segments,
            vec![
                if runtime {
                    PlaceSegment::Index { expression: index }
                } else {
                    PlaceSegment::FixedIndex { index: 1 }
                },
                PlaceSegment::Field { symbol: symbols[2] },
            ]
        );
        assert_eq!(
            super::super::coarse_place_path(&program, expression),
            Some(origin.path)
        );
    }
}

#[test]
fn structural_projection_appends_only_the_normalized_suffix() {
    let (mut program, symbols) = fixture();
    let collection = name(&mut program, "values", symbols[0]);
    let index = name(&mut program, "index", symbols[1]);
    let (indexed, whole) = field_after_index(&mut program, collection, index, symbols[2]);
    let source = FrameSourcePlace {
        root: symbols[3],
        segments: vec![PlaceSegment::FixedIndex { index: 5 }],
        builtin_coordinates: true,
    };
    assert_eq!(
        source.projected(&program, whole, indexed),
        FrameSourcePlace {
            root: symbols[3],
            builtin_coordinates: true,
            segments: vec![
                PlaceSegment::FixedIndex { index: 5 },
                PlaceSegment::Field { symbol: symbols[2] },
            ],
        }
    );
    assert_eq!(
        source.projected(&program, indexed, whole),
        FrameSourcePlace::default()
    );
    let foreign = name(&mut program, "caller", symbols[3]);
    assert_eq!(
        source.projected(&program, whole, foreign),
        FrameSourcePlace::default()
    );
    assert_eq!(
        FrameSourcePlace::default().projected(&program, whole, indexed),
        FrameSourcePlace::default()
    );
}

#[test]
fn relative_source_erases_only_callee_runtime_index_handles() {
    let (mut program, symbols) = fixture();
    let index = name(&mut program, "index", symbols[1]);
    let caller = FrameSourcePlace {
        root: symbols[3],
        segments: vec![PlaceSegment::Index { expression: index }],
        builtin_coordinates: true,
    };
    let relative = FrameSourcePlace {
        root: symbols[0],
        builtin_coordinates: true,
        segments: vec![
            PlaceSegment::Index { expression: index },
            PlaceSegment::FixedIndex { index: 2 },
            PlaceSegment::Field { symbol: symbols[2] },
        ],
    };
    assert_eq!(
        caller.append_relative(&relative),
        FrameSourcePlace {
            root: symbols[3],
            builtin_coordinates: true,
            segments: vec![
                PlaceSegment::Index { expression: index },
                PlaceSegment::Index {
                    expression: ExpressionHandle::invalid()
                },
                PlaceSegment::FixedIndex { index: 2 },
                PlaceSegment::Field { symbol: symbols[2] },
            ],
        }
    );
    assert_eq!(
        caller.append_relative(&FrameSourcePlace::default()),
        FrameSourcePlace::default()
    );
    assert_eq!(
        FrameSourcePlace::default().append_relative(&relative),
        FrameSourcePlace::default()
    );
    assert_eq!(
        FrameSourcePlace::from_expression(&program, ExpressionHandle::invalid()),
        FrameSourcePlace::default()
    );
}
