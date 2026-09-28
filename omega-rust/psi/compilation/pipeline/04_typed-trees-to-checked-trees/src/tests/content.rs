//! Fixtures shared by the content tests: checked and rejected programs and
//! the retained content sources.

mod content_projections_and_old;
mod content_reshuffles_and_partitions;
mod retained_content_custody;
mod structural_conservation;

use crate::tests::front_end::checked_program_result;

fn rejected(source: &str) -> Vec<diagnostics::Diagnostic> {
    checked_program_result(source).expect_err("checked lowering should reject")
}

fn retained_borrow_program(signature: &str) -> String {
    [
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        data PendingRead<'storage> [linear] {}
        domain PendingRead::Retained
        established by Reader::submit;
        machine Retained::content(pending: &PendingRead) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: 1 } }

        boundary trait Reader {
        "#,
        signature,
        r#"
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    ]
    .join("\n")
}

fn retained_self_content_source(body: &str) -> String {
    r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }

        data Store { region: Region in Owned; }
        machine Store::retain(&mut self)
        ensures
            Owned::content(old(&self.region)) == Owned::content(&self.region)
        { RETAIN_BODY }
    "#
    .replace("RETAIN_BODY", body)
}
