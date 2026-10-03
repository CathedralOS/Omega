//! Internal states are transition destinations, never callables
//! (`wiki/spec/language/state_contracts.md`: "Ordinary calls invoke machines,
//! not internal states"). A statement that still spells a sibling state as a
//! call is rejected by name, with the transition spelling to migrate to,
//! instead of the generic unresolved-call diagnostic.

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
}

const COUNTER: &str = "data Counter { total: u64 in Wrapping; }
    machine bump(counter: &mut Counter) {
        transition {
            _ -> add(counter)
        }
        state add(counter: &mut Counter) {
            counter.total = counter.total + 1;
            STEP
        }
        state finish(counter: &mut Counter) {
            counter.total = counter.total + 1;
        }
    }";

#[test]
fn internal_state_called_as_a_statement_names_the_state() {
    let diagnostics = check(&COUNTER.replace("STEP", "finish(counter);"))
        .expect_err("a sibling state is not a callable machine");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            let message = diagnostic.to_string();
            message.contains("calls its internal state `finish` as an ordinary call")
                && message.contains("transition { _ -> finish(..) }")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn internal_state_entered_by_transition_checks() {
    check(&COUNTER.replace("STEP", "transition {\n _ -> finish(counter)\n }"))
        .expect("a transition enters the internal state");
}
