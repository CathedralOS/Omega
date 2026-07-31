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
