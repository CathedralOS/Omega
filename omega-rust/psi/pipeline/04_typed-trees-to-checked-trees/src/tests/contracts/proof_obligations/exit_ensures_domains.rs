use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn machine_entry_bounds_are_not_reintroduced_after_a_state_transition() {
    let source = r#"
        data Counter { length: u8 in Wrapping; }
        machine Counter::bump(&mut self)
        requires self.length < 4
        ensures self.length > 0
        {
            self.length = 255;
            transition { _ -> finish() }
            state finish(&mut self) {
                self.length = self.length + 1;
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("the entry bound cannot replace the later state's incoming value");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}

#[test]
fn rejects_exit_ensures_fixed_indexed_boolean_expression_from_domain_fact_after_mutating_call() {
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
            self.items[0].value > 0
        {
            self.mark_valid(&mut self.items[0]);
            self.break_valid(&mut self.items[0]);
            0
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("fixed indexed exit boolean ensures should fail after mutating call");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove ensures contract for exit from Main::main")
            && diagnostic.message.contains("self.items[0].value > 0")
    }));
}

#[test]
fn accepts_requires_from_local_boolean_alias_transfer() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::inspect(flag: bool)
        requires
            flag
        {
        }

        machine Main::main(&mut self)
        requires
            self.value > 0
        {
            let flag: bool = self.value > 0;
            self.inspect(flag);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("boolean requires should be provable from a transferred local alias fact");
}

/// A snapshot parameter lets `ensures` name the entry value: the write
/// `self.count = self.count + 1` stores a source expression whose `self.count`
/// read transports the live `self.count == before` fact, so the post-write
/// equality is provable through the write.
#[test]
fn snapshot_parameter_equality_transports_through_the_write() {
    let source = r#"
        data Counter { count: u32; cap: u32; }
        machine Counter::bump(&mut self, before: u32)
        requires
            self.count == before
            self.count < self.cap
        ensures
            self.count == before + 1
        {
            self.count = self.count + 1;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}
