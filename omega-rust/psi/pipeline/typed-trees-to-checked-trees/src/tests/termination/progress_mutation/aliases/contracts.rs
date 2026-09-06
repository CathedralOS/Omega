use super::*;

fn checks(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    lower_typed_trees(typed)
}

fn reject_requires(source: &str) {
    let diagnostics =
        checks(source).expect_err("reference identity cannot establish a missing domain fact");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call waiting")),
        "{diagnostics:#?}"
    );
}

#[test]
fn an_alias_requires_an_existing_fact_for_its_actual_referent() {
    for access in ["", "mut "] {
        let body = format!(
            "let borrowed: &{access}Context = &{access}context;
             transition {{ _ -> waiting(borrowed) }}
             state waiting(selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             {{ wait_context(selected) }}"
        );
        assert_subjects(
            &checks(&fixture_source(&body, true, false, "")).unwrap(),
            &["context"],
        );
        reject_requires(&fixture_source(&body, false, false, ""));
    }
}

#[test]
fn rebinding_a_shared_alias_cannot_keep_its_old_referents_fact() {
    let body = "let mut borrowed: &Context = &context;
        borrowed = &replacement;
        transition { _ -> waiting(borrowed) }
        state waiting(selected: &Context) -> u64
        requires selected.scheduler in WeakFair
        { wait_context(selected) }";
    let qualified = fixture_source(body, true, false, "");
    assert_subjects(&checks(&qualified).unwrap(), &["replacement"]);
    let missing = qualified.replace("requires replacement.scheduler in WeakFair", "");
    reject_requires(&missing);
}

#[test]
fn a_reference_does_not_restore_a_fact_invalidated_by_a_store() {
    let body = "let borrowed: &mut Context = &mut context;
        borrowed.scheduler = replacement.scheduler;
        transition { _ -> waiting(borrowed) }
        state waiting(selected: &Context) -> u64
        requires selected.scheduler in WeakFair
        { wait_context(selected) }";
    let qualified = fixture_source(body, true, false, "");
    assert_subjects(&checks(&qualified).unwrap(), &["replacement"]);
    reject_requires(&qualified.replace(
        "borrowed.scheduler = replacement.scheduler;",
        "borrowed.scheduler = SchedulerHandle {};",
    ));
}

#[test]
fn earlier_argument_effects_must_preserve_a_reference_domain_fact() {
    for field in ["counter", "scheduler"] {
        let body = "let borrowed: &mut Context = &mut context;
            transition { _ -> waiting(change(borrowed), borrowed) }
            state waiting(ignored: u64, selected: &Context) -> u64
            requires selected.scheduler in WeakFair
            { wait_context(selected) }";
        let assignment = if field == "counter" {
            "context.counter = 1;"
        } else {
            "context.scheduler = SchedulerHandle {};"
        };
        let extra = format!("machine change(context: &mut Context) -> u64 {{ {assignment} 0 }}");
        let source = fixture_source(body, true, false, &extra);
        if field == "counter" {
            assert_subjects(&checks(&source).unwrap(), &["context"]);
        } else {
            reject_requires(&source);
        }
    }
}
