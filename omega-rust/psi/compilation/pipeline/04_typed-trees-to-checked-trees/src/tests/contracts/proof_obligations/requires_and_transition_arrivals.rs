use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn rejects_exit_ensures_dynamic_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
    let source = r#"
        data Item {
            value: i32;
            tag: i32;
        }

        domain Item::Valid
        requires
            self.value > 0;

        data Main {
            items: [Item; 2];
            index: u64;
        }

        machine Main::mark_valid(&mut self, item: &mut Item)
        ensures
            item in Item::Valid
        {
            item.value = 12;
        }

        machine Main::break_valid(&mut self, item: &mut Item) {
            item.value = 0;
        }

        machine Main::main(&mut self) -> i32
        ensures
            self.items[self.index].value > 0
        {
            self.mark_valid(&mut self.items[self.index]);
            self.break_valid(&mut self.items[self.index]);
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("dynamic indexed exit boolean ensures should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main")
            && diagnostic
                .message
                .contains("self.items[self.index].value > 0")
    }));
}

#[test]
fn rejects_unproven_requires_domain_union() {
    let source = r#"
        data Password [copy] {
            length: i32;
            score: i32;
        }

        domain Password::Valid
        requires
            self.length > 0;

        domain Password::Secure
        requires
            self.score >= 8;

        data Main {
            password: Password;
        }

        machine Main::accept(password: Password)
        requires
            password in Password::Valid | Password::Secure
        {
        }

        machine Main::main(&mut self) {
            self.accept(self.password);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("requires union should fail when neither domain branch is proven");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from Main::main")
            && diagnostic
                .message
                .contains("password.length > 0 || password.score >= 8")
    }));
}

/// A named call nested inside an enclosing call's operand list writes its
/// `&mut` operand before the enclosing call binds: `x` was captured as
/// `sink`'s first operand before `Ns::bump` rewrote its storage, so the
/// `x >= 20` fact `bump` publishes describes a different storage version than
/// the operand `sink` bound. Facts riding the rewritten source place must not
/// survive the enclosing invocation to prove `admit`'s `requires`. An
/// ordinary call already enters this operand-write timeline; a named call
/// mints no borrow-call row for `invoke` to find, so its write set must join
/// the timeline explicitly.
#[test]
fn named_call_operand_write_retires_facts_riding_the_rewritten_operand_source() {
    for (declaration, call) in [
        (
            "machine bump(value: &mut i32) -> i32
             ensures
                 value >= 20
             {
                 value = 20;
                 value
             }",
            "bump(&mut x)",
        ),
        (
            "boundary operator Ns::bump(value: &mut i32) -> i32
             ensures
                 value >= 20;",
            "Ns::bump(&mut x)",
        ),
    ] {
        let source = format!(
            r#"
                {declaration}

                machine sink(first: i32, second: i32) {{
                }}

                machine admit(value: i32)
                requires
                    value >= 20
                {{
                }}

                machine drive(mut x: i32) {{
                    sink(x, {call});
                    admit(x);
                }}
            "#
        );

        let Err(diagnostics) =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
        else {
            panic!(
                "a nested `&mut` operand write — spelled or named — must retire the ensures fact riding the rewritten operand source: {call}"
            );
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove requires contract for call admit")
            }),
            "{call}: {diagnostics:#?}"
        );
    }
}

#[test]
fn broader_declared_local_range_does_not_prove_call_requires() {
    let source = r#"
        machine accept(value: i64)
        requires
            value >= 0 && value <= 255
        {
        }

        machine main(source: i64 [-1..=255]) {
            let not_nonnegative: i64 [-1..=255] = source;
            accept(not_nonnegative);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a range containing -1 must not establish nonnegativity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove requires contract for call accept from main")
            && diagnostic.message.contains("value >= 0")
    }));
}

#[test]
fn rejects_named_back_edge_after_state_arrival_fact_is_invalidated() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(context: &mut Main) {
            transition context.value > 0 {
                true -> positive(context)
                false -> done(context)
            }

            state positive(context: &mut Main)
            requires
                context.value > 0
            {
                context.value = 0;
                transition {
                    _ -> positive(context)
                }
            }

            state done(context: &mut Main) {
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a named back-edge must re-establish an invalidated arrival invariant");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("cannot prove")
            && diagnostic.message.contains("context.value > 0")
    }));
}
