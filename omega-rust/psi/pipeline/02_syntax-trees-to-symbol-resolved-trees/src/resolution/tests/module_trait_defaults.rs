use crate::resolution::ResolutionRequest;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn resolve(texts: &[&str]) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (ordinal, text) in texts.iter().enumerate() {
        let source = sources
            .add(
                PathBuf::from(format!("module_default_{ordinal}.omg")),
                (*text).to_owned(),
            )
            .source_id;
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize module default selection");
        parse_syntax_trees_into_with_id(&mut syntax, source, &tokens)
            .expect("parse module default selection");
    }
    crate::resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
}

#[test]
fn module_trait_default_keeps_the_exact_selected_trait_and_carrier() {
    let resolved = resolve(&[
        "module first; trait Service { machine run(&mut self) { } } data Worker {} first_membership: Worker satisfies Service;",
        "module second; trait Service { machine stop(&mut self) { } } data Worker {}",
    ])
    .expect("same-spelled module declarations select their exact owners");
    let machine = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "first::Worker::run")
        .expect("the selected module's synthesized default");
    assert_eq!(
        resolved
            .symbols
            .display_path(machine.attached_data_symbol, "::"),
        "first::Worker",
        "the synthesized machine attaches to the exact carrier declaration"
    );
    let [requirement] = resolved.machine_trait_conformances(machine.satisfies) else {
        panic!("the synthesized default retains one requirement edge");
    };
    assert_eq!(
        resolved.symbols.display_path(requirement.symbol, "::"),
        "first::Service",
        "the generated requirement edge rejoins the exact trait declaration"
    );
}

#[test]
fn module_closed_conformance_rows_retain_the_exact_declaring_trait() {
    let resolved = resolve(&[
        "module first; trait Service { machine run(&mut self) { } } data Worker {} membership: Worker satisfies Service {}",
        "module second; trait Service { machine stop(&mut self) { } }",
    ])
    .expect("the closed row joins its exact declaring trait");
    let conformance = resolved
        .conformances
        .iter()
        .next()
        .expect("one conformance");
    let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
        &conformance.implementation
    else {
        panic!("closed implementation retained");
    };
    let [row] = rows.as_slice() else {
        panic!("one synthesized default row");
    };
    assert_eq!(
        resolved.symbols.display_path(row.declaring_trait, "::"),
        "first::Service",
        "the qualified declaring-trait row selects the same-module trait"
    );
    assert!(row.realization_machine.is_valid());
}

#[test]
fn module_conformance_does_not_borrow_a_same_spelled_foreign_template() {
    // `second::Service` carries a default `stop`; `first::Service` requires
    // `run` with no body. The first-module conformance must not inherit
    // `stop` from the same-leaf sibling.
    let resolved = resolve(&[
        "module first; trait Service { machine run(&mut self); } data Worker {} machine Worker::run(&mut self) { } membership: Worker satisfies Service;",
        "module second; trait Service { machine stop(&mut self) { } }",
    ])
    .expect("the conformance uses only its own module's template");
    assert!(
        !resolved
            .machines
            .iter()
            .any(|machine| machine.name.as_str() == "first::Worker::stop"),
        "no same-leaf foreign default attaches to first::Worker"
    );
}
