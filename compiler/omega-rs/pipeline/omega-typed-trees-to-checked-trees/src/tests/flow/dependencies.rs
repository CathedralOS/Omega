use super::*;

#[test]
fn materializes_domain_dependency_facts() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        domain Player::Valid {
            self.health >= 0;
            self.health <= 100;
        }

        domain Player::Ready {
            self in Player::Valid;
            self.mana >= 0;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);

    let ready_symbol = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Player::Ready")
        .map(|domain| domain.symbol)
        .expect("ready domain");
    let ready_fact = domains
        .dependencies
        .iter()
        .find_map(|(_, fact)| (fact.domain_symbol == ready_symbol).then_some(fact))
        .expect("ready dependency fact");

    let paths = domains
        .dependency_paths
        .span_or_empty(ready_fact.dependencies);
    assert_eq!(paths.len(), 2);

    let mut field_symbols = paths
        .iter()
        .filter_map(|path| {
            let segments = domains.segments.span_or_empty(path.segments);
            match segments {
                [omega_facts::PlaceSegment::Field { symbol }] => Some(*symbol),
                _ => None,
            }
        })
        .collect::<Vec<omega_core::symbols::SymbolHandle>>();
    field_symbols.sort_by_key(|symbol| symbol.arena_index());

    let player = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Player")
        .expect("player data");
    let mut expected = typed
        .data_members(player)
        .iter()
        .filter_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == "health" || field.name.as_str() == "mana" =>
            {
                Some(field.symbol)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|symbol| symbol.arena_index());

    assert_eq!(field_symbols, expected);
}
