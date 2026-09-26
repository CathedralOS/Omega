use super::{rejected, retained_self_content_source};
use crate::tests::front_end::checked_program;

#[test]
fn scalar_and_content_guarantees_are_checked_independently() {
    let source = retained_self_content_source("7")
        .replace("retain(&mut self)", "retain(&mut self) -> u64")
        .replace(
            "== Owned::content(&self.region)",
            "== Owned::content(&self.region); result == 7",
        );
    let accepted = checked_program(&source);
    assert_eq!(
        accepted
            .facts
            .qualifications
            .content
            .conservation_plans
            .len(),
        1
    );
    for invalid in [
        source.replace("result == 7", "result == 8"),
        source.replace("{ 7 }", "{ self.region.length = 5; 7 }"),
    ] {
        let diagnostics = rejected(&invalid);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn signed_runtime_embedding_cannot_enter_a_nat_content_algebra() {
    let diagnostics = rejected(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: Nat; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { delta: i64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: embed(region.delta) as Nat }
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("outside the closed projection fragment")
    }));
}

#[test]
fn old_rejects_result_local_computed_and_retired_entry_operands() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }

        trait InvalidOldOperands {
            machine result_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&result)) == Owned::content(&whole);
            machine local_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&scratch)) == Owned::content(&whole);
            machine computed_operand(whole: Region in Owned)
            ensures
                Owned::content(old(&whole.length + 1)) == Owned::content(&whole);
            machine retired_entry_operand(whole: Region in Owned)
            ensures
                Owned::content(entry(&whole)) == Owned::content(&whole);
        }
        "#,
    );
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("`old(result)` is invalid"), "{rendered}");
    assert!(
        rendered.contains("projection root `scratch` is not a callable parameter"),
        "{rendered}"
    );
    assert!(
        rendered.contains("is not a parameter, `self`, `result`, or structural subplace"),
        "{rendered}"
    );
    assert!(rendered.contains("entry(&whole)"), "{rendered}");
}
