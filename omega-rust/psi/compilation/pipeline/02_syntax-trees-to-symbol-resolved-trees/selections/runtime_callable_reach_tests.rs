use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;

fn parse(text: &str, source: source::SourceId) -> syntax_trees::SyntaxTrees {
    let tokens = crate::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap()
}

fn check_rows(program: &SymbolResolvedTrees) {
    let contracts = program
        .tables
        .declarations
        .runtime_callable_contracts
        .iter()
        .map(|(_, contract)| &contract.signature)
        .collect::<Vec<_>>();
    assert_eq!(contracts.len(), 3);
    let service = program
        .traits
        .iter()
        .find(|declaration| declaration.name.as_str() == "Io")
        .unwrap()
        .symbol;
    let service_id = program.service_reaches.id_for_symbol(service).unwrap();
    for (position, signature) in contracts.iter().enumerate() {
        let authored = program
            .authored_service_reach_rows_for(signature.symbol)
            .collect::<Vec<_>>();
        let normalized = program
            .service_reach_rows
            .services(signature.service_reach_row);
        assert!(
            signature.blocks,
            "operational metadata remains independent of reach"
        );
        match position {
            0 => {
                assert_eq!(
                    authored.len(),
                    1,
                    "explicit reaches none retains writtenness"
                );
                assert_eq!(authored[0].owner, signature.symbol);
                assert!(!authored[0].keyword_source_spans.is_empty());
                assert!(authored[0].targets.is_empty());
                assert!(normalized.is_empty());
            }
            1 => {
                assert_eq!(authored.len(), 1);
                assert_eq!(authored[0].owner, signature.symbol);
                assert_eq!(authored[0].targets.len(), 1);
                assert_eq!(authored[0].targets[0].service, service);
                assert_eq!(normalized, &[service_id]);
            }
            2 => {
                assert!(
                    authored.is_empty(),
                    "omission must not become authored reaches none"
                );
                assert!(normalized.is_empty());
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn runtime_callable_reach_rows_keep_authored_and_omitted_ceilings_across_seeding() {
    let text = r#"
boundary trait Io { machine read() reaches none crashes none; }
machine hold(
    first: machine() -> u32 reaches none blocks crashes none,
    second: machine() -> u32 reaches Io blocks crashes none,
    third: machine() -> u32 blocks crashes none,
    value: u32
) {}
"#;
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("callable/base.omg".into(), text.into())
        .source_id;
    let sources = Arc::new(sources);
    let base = crate::resolve(crate::ResolutionRequest {
        syntax: parse(text, source),
        sources: Some(Arc::clone(&sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap();
    check_rows(&base);
    let original_rows = base.authored_service_reach_rows.clone();
    let original_contracts = base
        .tables
        .declarations
        .runtime_callable_contracts
        .iter()
        .map(|(handle, contract)| (handle, contract.signature.clone()))
        .collect::<Vec<_>>();
    let mut extended_sources = sources.as_ref().clone();
    let added = "machine extension() {}";
    let extension = extended_sources
        .add("callable/extension.omg".into(), added.into())
        .source_id;
    let resumed = crate::resolve_extension(crate::ExtensionRequest {
        base: Arc::new(base),
        base_declarations: Arc::new(parse(text, source)),
        syntax: parse(added, extension),
        sources: Arc::new(extended_sources),
        top_level_bindings: Vec::new(),
    })
    .unwrap();
    check_rows(resumed.trees());
    assert_eq!(resumed.trees().authored_service_reach_rows, original_rows);
    for (handle, signature) in original_contracts {
        assert_eq!(
            resumed
                .trees()
                .tables
                .declarations
                .runtime_callable_contracts
                .get(handle)
                .signature,
            signature
        );
    }
}
