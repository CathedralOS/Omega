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
fn retains_exact_machine_and_requirement_invocation_name_spans() {
    let source = r#"
boundary trait Host {
    machine ping()
    reaches Host
    invokes Host;
}

pub machine dispatch(host: &mut Host)
reaches Host
invokes host;
invokes Host;
{ }
"#;
    let typed = lower(source);

    let dispatch = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "dispatch")
        .expect("dispatch machine");
    let invocations = typed.machine_invokes(dispatch);
    assert_eq!(
        invocations
            .iter()
            .map(|invocation| span_text(source, &invocation.source_span))
            .collect::<Vec<_>>(),
        ["host", "Host"],
    );

    let host = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Host")
        .expect("Host trait");
    let entry = typed
        .machine_states(dispatch)
        .first()
        .expect("dispatch entry");
    let host_parameter = typed
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.name.as_str() == "host")
        .expect("host parameter");
    assert_eq!(
        invocations[0].target,
        psi_typed_trees::signature::AuthoredInvocationTarget::Parameter {
            ordinal: 0,
            symbol: host_parameter.symbol,
        }
    );
    assert_eq!(
        invocations[1].target,
        psi_typed_trees::signature::AuthoredInvocationTarget::Service(host.symbol)
    );
    let [ping] = typed.trait_machine_signatures(host) else {
        panic!("one Host requirement")
    };
    assert_eq!(
        typed
            .state_signature_invokes(ping)
            .iter()
            .map(|invocation| span_text(source, &invocation.source_span))
            .collect::<Vec<_>>(),
        ["Host"],
    );
    assert_eq!(
        typed.state_signature_invokes(ping)[0].target,
        psi_typed_trees::signature::AuthoredInvocationTarget::Service(host.symbol)
    );
}
