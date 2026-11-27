//! A contract clause may call a machine that reads a shared borrowed scalar,
//! record or slice formal (`wiki/drafts/designs/borrowed_proof_inputs.md`). The clause lends
//! one of its own borrowed formals, so the callee's `$value` reads the same
//! place at the same point as the caller writing `$value` itself, and a write
//! to that place in between invalidates the fact (`wiki/spec/terminal-psi/
//! fact_validity.md`, "Borrowed read dependencies").

#[path = "support/source_handoff.rs"]
mod source;

use source::{Program, Verdict};

const POSITIVE: &str = "machine positive(value: @i32) -> bool { $value > 0i32 }
machine needs(value: @i32) requires positive(value) {}
";

fn judged(caller: &str) -> Verdict {
    Program::of(&format!("{POSITIVE}{caller}")).judged(|_| {})
}

#[test]
fn a_clause_call_reads_the_referent_its_caller_lends() {
    let verdict = judged(
        "machine forward(value: @i32) requires $value > 5i32 { needs(value); }
machine chained(value: @i32) requires positive(value) { needs(value); }
",
    );
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_weaker_premise_about_the_referent_does_not_prove_the_call() {
    let verdict =
        judged("machine forward(value: @i32) requires $value > -1i32 { needs(value); }\n");
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

#[test]
fn a_write_to_the_referent_invalidates_the_callers_fact() {
    let verdict = judged(
        "machine stale(value: @mutable i32) requires positive(value) { $value = 0i32; needs(value); }\n",
    );
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

#[test]
fn an_unwritten_mutable_referent_keeps_the_callers_fact() {
    let verdict =
        judged("machine kept(value: @mutable i32) requires positive(value) { needs(value); }\n");
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

const RECORD: &str = "data P { x: i32; y: i32; }
machine x_positive(point: @P) -> bool { point.x > 0i32 }
machine needs_x(point: @P) requires x_positive(point) {}
";

#[test]
fn a_clause_call_reads_a_field_of_the_record_its_caller_lends() {
    let verdict = Program::of(&format!(
        "{RECORD}machine forward(point: @P) requires point.x > 3i32 {{ needs_x(point); }}\n"
    ))
    .judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_fact_about_another_field_does_not_prove_the_call() {
    let verdict = Program::of(&format!(
        "{RECORD}machine forward(point: @P) requires point.y > 3i32 {{ needs_x(point); }}\n"
    ))
    .judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

fn accepted(program: &str) {
    let verdict = Program::of(program).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

fn unproved(program: &str) {
    let verdict = Program::of(program).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains("unproved: [\"CallRequirement"),
        "{verdict:?}"
    );
}

const PAIR: &str = "machine pair(items: @[i32]) -> bool { items.length == 2 }
machine needs(items: @[i32]) requires pair(items) {}
";

#[test]
fn a_clause_call_reads_the_length_of_the_slice_its_caller_lends() {
    accepted(&format!(
        "{PAIR}machine forward(items: @[i32]) requires items.length == 2 {{ needs(items); }}
machine chained(items: @[i32]) requires pair(items) {{ needs(items); }}
machine lent(items: @mutable [i32]) requires items.length == 2 {{ needs(items); }}
"
    ));
}

#[test]
fn another_length_does_not_prove_the_slice_call() {
    unproved(&format!(
        "{PAIR}machine forward(items: @[i32]) requires items.length == 3 {{ needs(items); }}\n"
    ));
}

const HELD: &str = "data Held<'a> { slot: @'a i32; }
machine pair<'a>(items: @[Held<'a>]) -> bool { items.length == 2 }
machine needs<'a>(items: @[Held<'a>]) requires pair(items) {}
";

#[test]
fn a_length_call_over_reference_bearing_elements_reads_no_referent() {
    accepted(&format!(
        "{HELD}machine forward<'a>(items: @[Held<'a>]) requires items.length == 2 {{ needs(items); }}\n"
    ));
}

#[test]
fn another_length_over_reference_bearing_elements_does_not_prove_the_call() {
    unproved(&format!(
        "{HELD}machine forward<'a>(items: @[Held<'a>]) requires items.length == 1 {{ needs(items); }}\n"
    ));
}

const NESTED: &str = "data Inner { v: i32; u: i32; }
data Outer { inner: Inner; w: i32; }
machine deep(o: @Outer) -> bool { o.inner.v > 0i32 }
machine needs(o: @Outer) requires deep(o) {}
";

#[test]
fn a_clause_call_reads_a_nested_field_path() {
    accepted(&format!(
        "{NESTED}machine forward(o: @Outer) requires o.inner.v > 3i32 {{ needs(o); }}\n"
    ));
}

#[test]
fn a_fact_about_a_sibling_nested_field_does_not_prove_the_call() {
    unproved(&format!(
        "{NESTED}machine forward(o: @Outer) requires o.inner.u > 3i32 {{ needs(o); }}\n"
    ));
}

const ELEMENT: &str = "data Window { v: [i32; 2]; }
machine first(a: @Window) -> bool { a.v[0] > 0i32 }
machine needs(a: @Window) requires first(a) {}
";

#[test]
fn a_clause_call_reads_an_array_element_of_a_lent_record() {
    accepted(&format!(
        "{ELEMENT}machine forward(a: @Window) requires a.v[0] > 3i32 {{ needs(a); }}
machine kept(a: @mutable Window) requires a.v[0] > 3i32 {{ a.v[1] = 0i32; needs(a); }}
"
    ));
}

#[test]
fn another_element_does_not_prove_the_element_call() {
    unproved(&format!(
        "{ELEMENT}machine forward(a: @Window) requires a.v[1] > 3i32 {{ needs(a); }}\n"
    ));
}

#[test]
fn a_write_to_the_read_element_invalidates_the_callers_fact() {
    unproved(&format!(
        "{ELEMENT}machine stale(a: @mutable Window) requires a.v[0] > 3i32 {{ a.v[0] = 0i32; needs(a); }}\n"
    ));
}

const HELPER: &str = "machine positive(value: @i32) -> bool { $value > 0i32 }
machine through(value: @i32) -> bool { positive(value) }
machine needs_through(value: @i32) requires through(value) {}
";

#[test]
fn a_clause_call_reads_through_a_helper_that_lends_onward() {
    accepted(&format!(
        "{HELPER}machine forward(value: @i32) requires $value > 3i32 {{ needs_through(value); }}\n"
    ));
}

#[test]
fn a_weaker_premise_does_not_prove_a_helper_call() {
    unproved(&format!(
        "{HELPER}machine forward(value: @i32) requires $value > -1i32 {{ needs_through(value); }}\n"
    ));
}

const GUARDED: &str = "machine big(value: @i32) -> bool requires $value > 0i32 { $value > 10i32 }
";

fn lowering_refusal(program: &str) -> String {
    match Program::try_of(&[("product/main.omg", program)]) {
        Ok(_) => panic!("lowering accepted {program}"),
        Err(refusal) => format!("{refusal:?}"),
    }
}

#[test]
fn a_clause_call_relies_on_the_requires_clauses_before_it() {
    accepted(&format!(
        "{GUARDED}machine needs(value: @i32) requires $value > 0i32, big(value) {{}}
machine split(value: @i32) requires $value > 0i32 requires big(value) {{}}
machine forward(value: @i32) requires $value > 20i32 {{ needs(value); split(value); }}
"
    ));
}

#[test]
fn a_clause_call_proves_nothing_its_callee_promises_beyond_the_premise() {
    unproved(&format!(
        "{GUARDED}machine needs(value: @i32) requires $value > 0i32, big(value) {{}}
machine forward(value: @i32) requires $value > 5i32 {{ needs(value); }}
"
    ));
}

#[test]
fn a_clause_call_cannot_rely_on_a_later_or_missing_clause() {
    for needs in [
        "machine needs(value: @i32) requires big(value) {}\n",
        "machine needs(value: @i32) requires big(value), $value > 0i32 {}\n",
        "machine needs(value: @i32) requires $value > -1i32, big(value) {}\n",
    ] {
        let refusal = lowering_refusal(&format!("{GUARDED}{needs}"));
        assert!(
            refusal.contains("called machine in a clause has an unproved precondition"),
            "{refusal}"
        );
    }
}
