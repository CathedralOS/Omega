//! Checked scalar expressions can decide a read-only atomic retry.
use super::super::build_purpose::judge_build;
use super::{execute_unit, expect_unproved_progress, reject_deadlock};
use crate::RouteVerdict;

fn one_cell(condition: &str, initial: &str) -> String {
    format!(
        "data Flag {{ word:AtomicU32; }}
        machine await_change(flag:@Flag, key:u32) {{
            let observed:u32=flag.word.load(Receive);
            transition {condition} {{true -> await_change(flag,key), _ -> done()}}
            machine done() {{}}
        }}
        machine build_entry() {{
            let mutable flag:Flag;
            flag.word.store({initial},Publish);
            await_change(@flag,2);
        }}"
    )
}

#[test]
fn a_read_only_arithmetic_predicate_cannot_hide_a_private_wait() {
    for test in [
        "(observed | key) == 3",
        "(observed ^ key) == 3",
        "(observed % 2) == 1",
    ] {
        reject_deadlock(&one_cell(test, "1"));
        execute_unit(&one_cell(test, "0"));
    }
}

#[test]
fn two_independent_read_operands_are_evaluated_in_the_same_retry() {
    let source = "data Pair { a:AtomicU32; b:AtomicU32; }
        machine await_change(pair:@Pair) {
            let left:u32=pair.a.load(Receive);
            let right:u32=pair.b.load(Receive);
            transition (left ^ right) == 0 {true -> await_change(pair), _ -> done()}
            machine done() {}
        }
        machine build_entry() {
            let mutable pair:Pair;
            pair.a.store(6,Publish);
            pair.b.store(6,Publish);
            await_change(@pair);
        }";
    reject_deadlock(source);
    execute_unit(&source.replace("b.store(6", "b.store(7"));
}

#[test]
fn an_unresolved_computed_wait_remains_an_obligation() {
    let source = one_cell("(observed | key) == 3", "input")
        .replace("machine build_entry()", "machine build_entry(input:u32)");
    expect_unproved_progress(&source);
}

#[test]
fn an_incidental_atomic_load_does_not_make_a_pure_loop_a_wait() {
    let source = one_cell("(key | 1) == 3", "1");
    assert!(matches!(
        judge_build(&source).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn forwarding_substitutes_both_the_borrowed_cell_and_the_calculation_operand() {
    let run = |initial| {
        let source = one_cell("(observed | key) == 3", initial)
            .replace("await_change(@flag,2);", "forward(2,@flag);");
        format!("{source} machine forward(key:u32, flag:@Flag) {{ await_change(flag,key); }}")
    };
    reject_deadlock(&run("1"));
    execute_unit(&run("0"));
}

#[test]
fn a_shared_cell_cannot_fix_a_computed_retry_across_iterations() {
    let source = one_cell("(observed | key) == 3", "1");
    let prefix = source.split_once("machine build_entry").unwrap().0;
    expect_unproved_progress(&format!(
        "{prefix}
        machine build_entry(flag:@Flag) {{
            flag.word.store(0,Publish);
            await_change(flag,2);
        }}"
    ));
}
