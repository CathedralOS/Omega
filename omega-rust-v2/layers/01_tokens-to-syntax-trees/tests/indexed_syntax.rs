use omega_lexer::lex;
use omega_parser::{NodeKind, parse_file};

#[test]
fn termination_parts_keep_their_source_expressions_and_clause_parent() {
    use omega_parser::TerminationClause;
    for (clause, expected) in [
        ("terminates", None),
        ("terminates by x", Some(("x", None, None))),
        ("terminates by x -> order(k)", Some(("x", Some("order(k)"), None))),
        ("terminates by x /* -> */ in limit", Some(("x", None, Some("limit")))),
        ("terminates by x -> order(k) in 0..=limit", Some(("x", Some("order(k)"), Some("0..=limit")))),
    ] {
        let source = format!("machine f() {clause} {{}}");
        let tokens = lex(source.as_bytes()).unwrap();
        let parsed = parse_file(source.as_bytes(), &tokens).unwrap();
        let tree = parsed.index(&source, &tokens).unwrap();
        let machine = tree.child(0, NodeKind::Machine).unwrap();
        let termination = tree.child(machine, NodeKind::Terminates).unwrap();
        let expression = |n| {
            assert_eq!(tree.ancestors(n).collect::<Vec<_>>(), [termination, machine, 0]);
            std::str::from_utf8(tree.span(n).slice(source.as_bytes()).unwrap()).unwrap()
        };
        let actual = match tree.nodes[termination].termination_clause().unwrap() {
            TerminationClause::Bare => None,
            TerminationClause::By { measure, order, bound } => Some((expression(measure), order.map(expression), bound.map(expression))),
        };
        assert_eq!(actual, expected, "{clause}");
    }
}

#[test]
fn pause_flags_preserve_exactly_what_was_written() {
    use omega_parser::PauseFlag;
    for (word, kind) in [("blocks", NodeKind::Blocks), ("suspends", NodeKind::Suspends)] {
        for (suffix, expected) in [("", PauseFlag::Omitted), (" true", PauseFlag::Explicit(true)), (" false", PauseFlag::Explicit(false))] {
            let source = format!("machine f() {word}{suffix} {{}}");
            let tokens = lex(source.as_bytes()).unwrap();
            let parsed = parse_file(source.as_bytes(), &tokens).unwrap();
            let tree = parsed.index(&source, &tokens).unwrap();
            let machine = tree.child(0, NodeKind::Machine).unwrap();
            let clause = tree.child(machine, kind).unwrap();
            assert_eq!(tree.nodes[clause].pause_flag(), Some(expected));
            assert_eq!(tree.nodes[machine].pause_flag(), None);
        }
    }
}

#[test]
fn indexed_nodes_preserve_parser_spelling_ranges_and_parentage() {
    let text = "machine f<T, const N:u8>(x:T)->T { /* gap */ x }";
    let tokens = lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = tree.index(text, &tokens).unwrap();
    let machine = syntax.child(0, NodeKind::Machine).unwrap();
    let generics = syntax.child(machine, NodeKind::GenericParams).unwrap();
    let binders = &syntax.nodes[generics].children;
    assert_eq!(binders.len(), 2);
    for (&id, name) in binders.iter().zip(["T", "N"]) {
        let leaf = syntax.nodes[id].binder_name().unwrap();
        assert_eq!(leaf.text, name);
        assert_eq!(&text[leaf.span.get_start_offset()..leaf.span.get_end_offset()], name);
        assert_eq!(syntax.nodes[id].parent, Some(generics));
        assert!(syntax.nodes[id].end <= syntax.nodes[generics].end);
    }
    let span = syntax.span(machine);
    assert_eq!(&text[span.get_start_offset()..span.get_end_offset()], text);
    assert_eq!(syntax.tokens.len(), tree.get_meaningful_token_indices().len());
    assert!(tree.index("", &tokens).is_none());
    assert!(tree.index(text, &[]).is_none());
}

#[test]
fn operator_spellings_and_spans_belong_to_the_parsed_tree() {
    let text = "machine f(x:u8,a:@[u8]){ _ = !x; _ = x >>% 1; _ = a[1 ..= 2]; }";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = parsed.index(text, &tokens).unwrap();
    let operators: Vec<_> = (0..syntax.nodes.len())
        .filter_map(|n| syntax.operator(n))
        .map(|op| (op.token, &text[op.span.get_start_offset()..op.span.get_end_offset()]))
        .collect();
    assert_eq!(operators, [("!", "!"), (">>%", ">>%"), ("[]", "[1 ..= 2]"), ("..=", "..=")]);
}

#[test]
fn generic_assembly_keeps_mnemonics_separate_from_operand_words() {
    let text = "asm machine copy<T> { mov x; mov /* gap */ . byte x, y; }";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = parsed.index(text, &tokens).unwrap();
    let names: Vec<_> = syntax
        .nodes
        .iter()
        .filter_map(|n| n.instruction_name())
        .map(|(name, span)| (name, &text[span.get_start_offset()..span.get_end_offset()]))
        .collect();
    assert_eq!(
        names,
        [
            ("mov".into(), "mov"),
            ("mov.byte".into(), "mov /* gap */ . byte")
        ]
    );
    assert!(syntax.nodes[0].binder_name().is_none());
}

#[test]
fn empty_and_trivia_only_files_have_an_empty_root_and_no_operator() {
    for text in ["", " /*none*/ \n"] {
        let tokens = lex(text.as_bytes()).unwrap();
        let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
        let syntax = parsed.index(text, &tokens).unwrap();
        assert_eq!(syntax.nodes.len(), 1);
        assert_eq!(syntax.span(0), omega_lexer::SourceSpan::new(0, 0));
        assert!(syntax.operator(0).is_none());
    }
}

#[test]
fn a_qualified_argument_and_its_head_identify_the_same_receiving_slot() {
    let text = "machine f(x:Pair<bool,u8 in Small>){}";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = parsed.index(text, &tokens).unwrap();
    let section = syntax
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::TypeArgs)
        .unwrap();
    for (index, &arg) in syntax.nodes[section].children.iter().enumerate() {
        let (head, domains) = syntax.argument_head(arg);
        assert_eq!(syntax.argument_position(head), Some((section, index)));
        assert_eq!(syntax.argument_position(arg), Some((section, index)));
        assert_eq!(domains.len(), index);
        for &domain in domains {
            assert_eq!(syntax.argument_position(domain), None);
        }
    }
}

#[test]
fn contextual_names_do_not_set_declaration_modifiers() {
    let text = "trait boundary{} public boundary trait S{} public satisfies S{} public satisfies satisfies S{}";
    let tokens = lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = tree.index(text, &tokens).unwrap();
    let modifiers: Vec<_> = syntax.nodes[0]
        .children
        .iter()
        .map(|&i| {
            let node = &syntax.nodes[i];
            (
                node.has_public_modifier(),
                node.has_boundary_modifier(),
                node.conformance_name().map(|leaf| leaf.text),
            )
        })
        .collect();
    assert_eq!(
        modifiers,
        [
            (false, false, None),
            (true, true, None),
            (false, false, Some("public")),
            (true, false, Some("satisfies"))
        ]
    );
}

#[test]
fn a_path_exposes_only_arguments_written_on_its_final_segment() {
    let text = "machine f(a:Box<u8>::Tag<2>, b:Box<u8>::Tag, c:Tag, d:Tag<3>) {}";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = parsed.index(text, &tokens).unwrap();
    let qualifiers: Vec<_> = syntax
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Param)
        .map(|node| {
            syntax.nodes[node.children[0]]
                .qualifier_span()
                .and_then(|span| span.slice(text.as_bytes()))
        })
        .collect();
    assert_eq!(qualifiers, [Some(b"Box".as_slice()), Some(b"Box".as_slice()), None, None]);
    let args: Vec<_> = syntax
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Param)
        .map(|param| {
            syntax
                .final_segment_arguments(param.children[0])
                .map(|section| syntax.span(section).slice(text.as_bytes()).unwrap())
        })
        .collect();
    assert_eq!(args, [Some(b"<2>".as_slice()), None, None, Some(b"<3>".as_slice())]);
}

#[test]
fn a_domain_head_and_its_alias_terms_have_distinct_argument_sections() {
    let text = "domain<const N:u8> Box<u8>::Tag<N> = Box<u8>::Other<N>;";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = parsed.index(text, &tokens).unwrap();
    let domain = syntax
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Domain)
        .unwrap();
    let sections: Vec<Vec<_>> = domain
        .children
        .iter()
        .copied()
        .filter(|&n| syntax.nodes[n].kind == NodeKind::PathType)
        .map(|n| {
            syntax
                .reference_sections(n)
                .iter()
                .map(|&s| syntax.span(s).slice(text.as_bytes()).unwrap())
                .collect()
        })
        .collect();
    assert_eq!(
        sections,
        [
            vec![b"<u8>".as_slice()],
            vec![b"<u8>".as_slice(), b"<N>".as_slice()]
        ]
    );
}

#[test]
fn register_clauses_preserve_register_names_and_the_empty_set() {
    let text = "asm machine f(x:u8) registers none=x clobbers r0,r1 {} asm machine g() clobbers none {}";
    let tokens = lex(text.as_bytes()).unwrap();
    let syntax = parse_file(text.as_bytes(), &tokens)
        .unwrap()
        .index(text, &tokens)
        .unwrap();
    let clauses: Vec<Vec<_>> = syntax
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Registers | NodeKind::Clobbers))
        .map(|node| node.register_names().map(|leaf| leaf.text).collect())
        .collect();
    assert_eq!(clauses, [vec!["none"], vec!["r0", "r1"], vec![]]);
    assert_eq!(syntax.nodes[0].register_names().count(), 0);
}

#[test]
fn range_endpoints_remain_on_the_written_side_of_the_operator() {
    let text = "machine f(a:@[u8], x:u64, y:u64){ _ = a[..]; _ = a[x..]; _ = a[..=y]; _ = a[(x) /* gap */ ..= (y)]; _ = a[x]; }";
    let tokens = lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = tree.index(text, &tokens).unwrap();
    let spelling = |n| {
        let span = syntax.span(n);
        &text[span.get_start_offset()..span.get_end_offset()]
    };
    let ranges: Vec<_> = (0..syntax.nodes.len())
        .filter_map(|n| syntax.range_bounds(n))
        .map(|bounds| (bounds.start.map(spelling), bounds.end.map(spelling), bounds.inclusive))
        .collect();
    assert_eq!(
        ranges,
        [
            (None, None, false),
            (Some("x"), None, false),
            (None, Some("y"), true),
            (Some("(x)"), Some("(y)"), true)
        ]
    );
}

#[test]
fn a_generic_argument_list_identifies_its_applied_path_prefix() {
    let text = "machine f(x:Outer<u8> /* gap */ ::Inner<bool>::Last<u32>) {root::Outer::<u8>::get::<bool>();}";
    let tokens = lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let syntax = tree.index(text, &tokens).unwrap();
    let prefixes: Vec<_> = syntax
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs))
        .map(|(n, _)| {
            syntax
                .application_prefix(n)
                .map(|(name, span)| (name, span.slice(text.as_bytes()).unwrap()))
        })
        .collect();
    assert_eq!(
        prefixes,
        [
            Some(("Outer".into(), b"Outer".as_slice())),
            Some(("Outer::Inner".into(), b"Outer<u8> /* gap */ ::Inner".as_slice())),
            None,
            Some(("root::Outer".into(), b"root::Outer".as_slice())),
            None
        ]
    );
    assert!(syntax.application_prefix(0).is_none());
}

#[test]
fn mathematical_bindings_separate_the_result_type_from_the_definition() {
    let source = "boundary let witness<T>(x:T):bool; let identity(x:u8):u8 = x;";
    let tokens = lex(source.as_bytes()).unwrap();
    let parsed = parse_file(source.as_bytes(), &tokens).unwrap();
    let tree = parsed.index(source, &tokens).unwrap();
    let text = |n| std::str::from_utf8(tree.span(n).slice(source.as_bytes()).unwrap()).unwrap();
    let bindings: Vec<_> = (0..tree.nodes.len())
        .filter_map(|n| tree.math_binding(n))
        .map(|b| (text(b.parameters), text(b.result), b.body.map(text)))
        .collect();
    assert_eq!(bindings, [("(x:T)", "bool", None), ("(x:u8)", "u8", Some("x"))]);
}

#[test]
fn machine_headers_and_callable_types_expose_each_named_annotation() {
    let source = "asm machine noop {} machine call(f:machine(x:u8)->bool,y:u32)->bool { f(1) }";
    let tokens = lex(source.as_bytes()).unwrap();
    let parsed = parse_file(source.as_bytes(), &tokens).unwrap();
    let tree = parsed.index(source, &tokens).unwrap();
    let text = |n| std::str::from_utf8(tree.span(n).slice(source.as_bytes()).unwrap()).unwrap();
    for (n, syntax) in tree.nodes.iter().enumerate() {
        let params: Vec<_> = tree
            .parameters(n)
            .map(|(name, ty)| (name.text, text(ty)))
            .collect();
        let result = tree.result_annotation(n).map(text);
        match syntax.kind {
            NodeKind::AsmMachine => {
                assert!(params.is_empty());
                assert_eq!(result, None);
            }
            NodeKind::Machine => {
                assert_eq!(params, [("f", "machine(x:u8)->bool"), ("y", "u32")]);
                assert_eq!(result, Some("bool"));
            }
            NodeKind::MachineType => {
                assert_eq!(params, [("x", "u8")]);
                assert_eq!(result, Some("bool"));
            }
            _ => {}
        }
    }
}
