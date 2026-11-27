//! A contract clause may call a machine that reads a shared borrowed scalar
//! formal (`wiki/drafts/designs/borrowed_proof_inputs.md`). The clause lends
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
