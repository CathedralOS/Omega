use super::*;
use omega_core::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentProjectionExpression,
    ContentScalarExpression,
};

fn checked(source: &str) -> omega_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn rejected(source: &str) -> Vec<omega_core::diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect_err("checked lowering should reject")
}

#[test]
fn checked_facts_retain_normalized_content_projection() {
    let source = r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }
        data Region [linear] { length: u64; }
        domain Region::Owned;

        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: region.length + 0x1 }
        }

        data Main {}
        machine Main::main(&mut self) {}
    "#;

    let checked = checked(source);
    let [plan] = checked.facts.qualifications.content.plans.as_slice() else {
        panic!("one normalized content projection should be retained");
    };
    assert!(plan.domain.is_valid());
    assert!(plan.machine.is_valid());
    assert_ne!(plan.fingerprint, 0);
    assert!(matches!(
        &plan.algebra,
        ContentAlgebraIdentity::CountedQuantity { unit }
            if unit == "named(name(ByteUnit))"
    ));
    let ContentProjectionExpression::CountedQuantity { magnitude } = &plan.expression else {
        panic!("quantity projection shape");
    };
    let ContentScalarExpression::Arithmetic {
        operator: ContentArithmeticOperator::Add,
        left,
        right,
    } = magnitude
    else {
        panic!("normalized addition");
    };
    assert!(
        matches!(
            left.as_ref(),
            ContentScalarExpression::SubjectField(path)
                if matches!(path.as_slice(), [field] if field.name == "length" && field.symbol.is_valid())
        ),
        "normalized subject path: {left:?}"
    );
    assert_eq!(
        right.as_ref(),
        &ContentScalarExpression::Natural("1".to_owned())
    );
}

#[test]
fn retained_content_custody_rejects_borrow_only_source() {
    let diagnostics = rejected(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained {
            Writer::submit;
        }
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        boundary trait Writer {
            machine submit(buffer: &Buffer in Buffer::Owned) -> PendingWrite
            ensures
                result in PendingWrite::Retained;
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("returns content-bearing custody `PendingWrite::Retained`")
                && diagnostic.message.contains("borrowed parameter `buffer`")
                && diagnostic.message.contains("consumed owned input")
        }),
        "borrow-only retained-custody diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn retained_content_custody_accepts_consumed_owned_source() {
    checked(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> {
            machine project(subject: &Self) -> A;
        }

        data Buffer [linear] {}
        domain Buffer::Owned;
        machine Owned::content(buffer: &Buffer) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        data PendingWrite [linear] {}
        domain PendingWrite::Retained {
            Writer::submit;
        }
        machine Retained::content(pending: &PendingWrite) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        {
            CountedQuantity { magnitude: 1 }
        }

        boundary trait Writer {
            machine submit(buffer: Buffer in Buffer::Owned) -> PendingWrite
            ensures
                result in PendingWrite::Retained;
        }

        data Main {}
        machine Main::main(&mut self) {}
        "#,
    );
}
