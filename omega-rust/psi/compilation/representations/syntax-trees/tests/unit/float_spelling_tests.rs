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
