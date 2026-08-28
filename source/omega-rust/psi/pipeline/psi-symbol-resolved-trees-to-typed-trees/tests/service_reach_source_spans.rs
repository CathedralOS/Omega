use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn lower(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn span_text<'a>(source: &'a str, span: &psi_source::SourceSpan) -> &'a str {
    &source[span.span.start..span.span.end]
}

#[test]
fn retains_exact_reach_targets_and_explicit_empty_clause_occurrences() {
    let source = r#"
boundary trait Parent { machine ping(); }
boundary trait Child: Parent { machine ping(); }

pub boundary trait Gateway {
    machine enter()
    reaches <= Child;
}

pub machine dispatch()
reaches Child + Child
{ }

machine strict()
reaches
{ }

machine inferred() { }

pub machine apply<machine Work>()
where machine Work()
    reaches Child;
{ }
"#;
    let typed = lower(source);
    let child = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Child")
        .expect("Child trait");

    let dispatch = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "dispatch")
        .expect("dispatch machine");
    let dispatch_reaches = typed
        .authored_service_reach_rows_for(dispatch.symbol)
        .collect::<Vec<_>>();
    let [dispatch_reach] = dispatch_reaches.as_slice() else {
        panic!("one dispatch reach row")
    };
    assert!(!dispatch_reach.installation_bound);
    assert_eq!(dispatch_reach.keyword_source_spans.len(), 1);
    assert_eq!(dispatch_reach.targets.len(), 2);
    assert!(
        dispatch_reach
            .targets
            .iter()
            .all(|target| target.service == child.symbol)
    );
    assert_eq!(
        dispatch_reach
            .targets
            .iter()
            .map(|target| span_text(source, &target.source_span))
            .collect::<Vec<_>>(),
        ["Child", "Child"],
    );

    let strict = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "strict")
        .expect("strict machine");
    let strict_reaches = typed
        .authored_service_reach_rows_for(strict.symbol)
        .collect::<Vec<_>>();
    let [strict_reach] = strict_reaches.as_slice() else {
        panic!("one explicit-empty reach row")
    };
    assert!(strict_reach.targets.is_empty());
    assert_eq!(strict_reach.keyword_source_spans.len(), 1);
    assert_eq!(
        span_text(source, &strict_reach.keyword_source_spans[0]),
        "reaches"
    );

    let inferred = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inferred")
        .expect("inferred machine");
    assert_eq!(
        typed
            .authored_service_reach_rows_for(inferred.symbol)
            .count(),
        0
    );

    let gateway = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Gateway")
        .expect("Gateway trait");
    let [enter] = typed.trait_machine_signatures(gateway) else {
        panic!("one Gateway requirement")
    };
    let gateway_reaches = typed
        .authored_service_reach_rows_for(enter.symbol)
        .collect::<Vec<_>>();
    let [gateway_reach] = gateway_reaches.as_slice() else {
        panic!("one Gateway reach row")
    };
    assert!(gateway_reach.installation_bound);
    assert_eq!(gateway_reach.targets[0].service, child.symbol);

    let apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let [work] = typed.machine_type_parameters(apply) else {
        panic!("one Work parameter")
    };
    let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &work.kind else {
        panic!("Work is a machine parameter")
    };
    let psi_typed_trees::data::MachineParameterContract::Structural(signature) = contract else {
        panic!("Work has a structural contract")
    };
    let work_reaches = typed
        .authored_service_reach_rows_for(signature.symbol)
        .collect::<Vec<_>>();
    let [work_reach] = work_reaches.as_slice() else {
        panic!("one structural reach row")
    };
    assert_eq!(work_reach.targets[0].service, child.symbol);
    assert_eq!(
        span_text(source, &work_reach.targets[0].source_span),
        "Child"
    );
}
