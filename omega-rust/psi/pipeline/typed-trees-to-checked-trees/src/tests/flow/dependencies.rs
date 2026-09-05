use super::*;

#[test]
fn constrained_type_composes_predicate_bodies_without_flow_minting_role_only_domains() {
    let source = r#"
        domain [u8]::Meaning;
        domain [u8]::Utf8
        requires
            valid_utf8(self);
        domain [u8]::NoNul
        requires
            no_nul(self);

        data Packet {
            bytes: &[u8] in Meaning & Utf8 & NoNul;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let packet = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Packet")
        .expect("packet data");
    let field_type = typed
        .data_members(packet)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "bytes" => {
                Some(field.type_reference)
            }
            _ => None,
        })
        .expect("bytes field");

    let names: Vec<_> =
        crate::field_domain::predicate_domain_constraint_symbols(&typed, field_type)
            .into_iter()
            .map(|symbol| {
                typed
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == symbol)
                    .expect("normalized domain symbol")
                    .name
                    .as_str()
                    .to_owned()
            })
            .collect();

    assert_eq!(names, ["[u8]::Utf8", "[u8]::NoNul"]);
}

#[test]
fn domain_conjunction_write_checks_every_predicate_facet() {
    let source = r#"
        domain [u8]::Utf8
        requires
            valid_utf8(self);
        domain [u8]::AsciiOnly
        requires
            ascii_only(self);

        data Main {
            text: &[u8] in Utf8 & AsciiOnly;
        }

        machine Main::main(&mut self) {
            self.text = "é";
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a UTF-8 but non-ASCII literal must fail the second predicate facet");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("domain `[u8]::AsciiOnly`") })
    );
}

#[test]
fn semantic_domain_ids_mint_and_propagate() {
    // STR4 checked plans, slice 1: every declared domain carries a VALID
    // normalized identity minted ONCE at syntax->resolved (declaration
    // order) and copied verbatim to the typed layer, where the table
    // resolves it back to the declared name. Distinct declarations get
    // distinct ids.
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        domain Player::Valid
        requires
            self.health >= 0;

        domain Player::Ready
        requires
            self.mana >= 0;
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let ids: Vec<_> = typed
        .domain_definitions()
        .iter()
        .map(|domain| (domain.name.as_str().to_owned(), domain.semantic_id))
        .collect();
    assert_eq!(ids.len(), 2, "both domains lowered");
    for (name, id) in &ids {
        assert!(id.is_valid(), "domain `{name}` minted a valid id");
        assert_eq!(
            typed.semantic_domains.name(*id),
            Some(name.as_str()),
            "the typed table resolves `{name}`'s id back to its name"
        );
        // The resolved-layer table agrees (copied verbatim).
        assert_eq!(resolved.semantic_domains.lookup(name), Some(*id));
    }
    assert_ne!(ids[0].1, ids[1].1, "distinct declarations, distinct ids");
}

#[test]
fn materializes_domain_dependency_facts() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        domain Player::Valid
        requires
            self.health >= 0;
            self.health <= 100;

        domain Player::Ready
        requires
            self in Player::Valid;
            self.mana >= 0;
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = proof::obligations::build_proof_plan(&typed);
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
                [facts::PlaceSegment::Field { symbol }] => Some(*symbol),
                _ => None,
            }
        })
        .collect::<Vec<symbols::SymbolHandle>>();
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
            typed_trees::data::DataMember::Field(field)
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
