use super::{ExpressionHandle, ExpressionNode, TypedTrees, closed_record_scalar_projection};

fn fixture(additional_declarations: &str) -> (TypedTrees, ExpressionHandle) {
    let source = format!(
        "data Cell [copy] {{ value: u64; }}
        data Other [copy] {{ value: u64; }}
        const VALUES: [Cell; 2] = [Cell {{ value: 7 }}, Cell {{ value: 9 }}];
        machine read() -> u64 {{ VALUES[0].value }}
        machine helper() -> u64 {{ 3 }}
        machine call() -> u64 {{ helper() }}
        {additional_declarations}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let projection = program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Member(_)).then_some(handle))
        .unwrap();
    (program, projection)
}

#[test]
fn array_record_projection_rejects_changed_identity_bounds_and_siblings() {
    let (original, projection) = fixture("");
    let (leaf, carrier) =
        closed_record_scalar_projection(&original, projection).expect("closed table value");
    assert_eq!(carrier, typed_trees::types::PrimitiveType::U64);
    assert!(
        matches!(original.expression_table.expression(leaf), ExpressionNode::Integer(value) if value.value_u64() == Some(7))
    );
    let ExpressionNode::Member(member) = original.expression_table.expression(projection) else {
        panic!("member");
    };
    let indexed = member.receiver;
    let ExpressionNode::Indexed(index) = original.expression_table.expression(indexed) else {
        panic!("index");
    };
    let root = index.collection;
    let selector = index.index;
    let ExpressionNode::ArrayLiteral(elements) = original.expression_table.expression(root) else {
        panic!("table");
    };
    let sibling = original.expression_table.expression_handles(*elements)[1];
    let ExpressionNode::StructLiteral(record) = original.expression_table.expression(sibling)
    else {
        panic!("record");
    };
    let sibling_value = original.expression_table.struct_fields(record.fields)[0].value;
    for mutation in [
        "foreign-field",
        "foreign-record",
        "out-of-bounds",
        "sibling-call",
        "sibling-cycle",
        "index-custody",
    ] {
        let mut changed = original.clone();
        match mutation {
            "foreign-field" | "foreign-record" => {
                let foreign = changed
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Other")
                    .unwrap();
                let foreign_symbol = foreign.symbol;
                let typed_trees::data::DataMember::Field(field) = &changed.data_members(foreign)[0]
                else {
                    panic!("field");
                };
                let field_symbol = field.symbol;
                if mutation == "foreign-field" {
                    let ExpressionNode::Member(member) =
                        changed.expression_table.expression_mut(projection)
                    else {
                        panic!("member");
                    };
                    member.member_symbol = field_symbol;
                } else {
                    let ExpressionNode::StructLiteral(record) =
                        changed.expression_table.expression_mut(sibling)
                    else {
                        panic!("record");
                    };
                    record.type_symbol = foreign_symbol;
                }
            }
            "out-of-bounds" => {
                *changed.expression_table.expression_mut(selector) =
                    ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(2));
            }
            "sibling-call" => {
                let call = changed
                    .expression_table
                    .expression_entries()
                    .find_map(|(_, node)| {
                        matches!(node, ExpressionNode::Call(_)).then(|| node.clone())
                    })
                    .unwrap();
                *changed.expression_table.expression_mut(sibling_value) = call;
            }
            "sibling-cycle" => {
                let cycle = changed.expression_table.expression(root).clone();
                *changed.expression_table.expression_mut(sibling) = cycle;
            }
            "index-custody" => {
                let selections = changed
                    .expression_table
                    .authored_selection_occurrences(root)
                    .collect::<Vec<_>>();
                assert!(!selections.is_empty());
                changed
                    .expression_table
                    .attach_authored_selection_occurrences(indexed, selections);
            }
            _ => unreachable!(),
        }
        assert!(
            closed_record_scalar_projection(&changed, projection).is_none(),
            "{mutation}"
        );
    }
}

#[test]
fn array_record_projection_does_not_replace_a_matching_declared_operator() {
    for (declaration, expected) in [
        (
            "operator [] Indexing::index(values: &[Cell], index: u64) -> Cell;",
            false,
        ),
        (
            "operator [] Indexing::index(values: &[Other], index: u64) -> Other;",
            true,
        ),
    ] {
        let (program, projection) = fixture(&format!("data Indexing {{}} {declaration}"));
        assert_eq!(
            closed_record_scalar_projection(&program, projection).is_some(),
            expected,
            "{declaration}"
        );
    }
}
