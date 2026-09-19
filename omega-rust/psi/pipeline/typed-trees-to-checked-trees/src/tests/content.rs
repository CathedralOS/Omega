//! Fixtures shared by the content tests: checked and rejected programs and
//! the retained content sources.

mod content_projections_and_old;
mod content_reshuffles_and_partitions;
mod retained_content_custody;
mod structural_conservation;

use crate::lower_typed_trees;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn rejected(source: &str) -> Vec<diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect_err("checked lowering should reject")
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
