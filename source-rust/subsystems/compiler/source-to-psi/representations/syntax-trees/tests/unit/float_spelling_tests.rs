use crate::SyntaxEquivalent;
use crate::float_spelling::*;
use source_text::{SourceId, Span};
use std::sync::Arc;

#[test]
fn equality_uses_exact_spelling_across_owners_and_generated_provenance() {
    let mut left = SyntaxTextOwner::default();
    let mut right = SyntaxTextOwner::default();
    let source = left.retain(Arc::from("1.25f32"));
    let other = right.retain(Arc::from("xx1.25f32"));
    let span = SourceSpan {
        source_id: SourceId(7),
        span: Span { start: 0, end: 7 },
    };
    let authored = FloatSpelling::source_backed(source, span);
    let shifted = FloatSpelling::source_backed(
        other,
        SourceSpan {
            source_id: SourceId(8),
            span: Span { start: 2, end: 9 },
        },
    );
    assert!(authored.same_syntax(&shifted, &left, &right));
    let generated = FloatSpelling::new("2.5f32", span);
    assert_eq!(
        generated.as_str(&left),
        "2.5f32",
        "provenance does not replace generated spelling"
    );
    assert!(!authored.same_syntax(&generated, &left, &right));
    assert!(authored.same_syntax(&FloatSpelling::generated("1.25f32"), &left, &right));
}

#[test]
fn generated_infinity_spelling_keeps_its_sign_and_landing() {
    use numeric_literals::FloatFormat;

    let owner = SyntaxTextOwner::default();
    for (text, expected, landing) in [
        ("+inff64", f64::INFINITY, Some(FloatFormat::F64)),
        ("+inf", f64::INFINITY, None),
        ("-inff32", f64::NEG_INFINITY, Some(FloatFormat::F32)),
        ("-inf", f64::NEG_INFINITY, None),
    ] {
        let literal = FloatSpelling::generated(text)
            .literal(&owner)
            .unwrap_or_else(|| panic!("cannot read generated spelling {text}"));
        assert_eq!(literal.landing(), landing, "{text}");
        assert_eq!(literal.landed_f64().to_bits(), expected.to_bits(), "{text}");
    }
}

#[test]
fn negating_a_materialized_float_keeps_the_nan_payload() {
    use numeric_literals::{FloatFormat, FloatLiteral};
    let literal = FloatLiteral::from_bits(FloatFormat::F32, 0x7f80_1234).unwrap();
    let owner = SyntaxTextOwner::default();
    let spelling = FloatSpelling::materialized(literal, SourceSpan::default());
    let negated = spelling.negated(&owner, SourceSpan::default());
    assert_eq!(
        negated.literal(&owner).unwrap().presented_bits(),
        Some((FloatFormat::F32, 0xff80_1234))
    );
}

#[test]
#[should_panic(expected = "does not belong to this forest")]
fn another_owner_cannot_resolve_a_float_source_handle() {
    let mut owner = SyntaxTextOwner::default();
    let source = owner.retain(Arc::from("1.0"));
    let spelling = FloatSpelling::source_backed(
        source,
        SourceSpan {
            source_id: SourceId(1),
            span: Span { start: 0, end: 3 },
        },
    );
    spelling.as_str(&SyntaxTextOwner::default());
}
