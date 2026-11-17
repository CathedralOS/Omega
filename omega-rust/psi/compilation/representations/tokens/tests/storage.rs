use std::sync::Arc;
use tokens::*;

#[test]
fn recycled_records_do_not_overwrite_escaped_literals() {
    let mut scratch = TokenScratch::default();
    let mut first = TokenStream::with_scratch("", 16, &mut scratch);
    first.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(b"first"));
    let literal = first.literal_bytes(&first[0]);
    let records = first.as_ptr();
    first.recycle(&mut scratch);
    let mut second = TokenStream::with_scratch("", 8, &mut scratch);
    assert!(second.is_empty());
    assert_eq!(second.as_ptr(), records);
    second.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(b"second"));
    assert_eq!(second.text(&second[0]).as_bytes(), b"second");
    assert_eq!(literal.as_ref(), b"first");
    second.recycle(&mut scratch);
    drop(scratch);
    assert_eq!(literal.as_ref(), b"first");
}

#[test]
fn retained_literals_share_storage_and_survive_token_mutation() {
    let mut stream = TokenStream::default();
    stream.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(&[0, 255]));
    stream.push_decoded(Span::default(), |_| {});
    let first = stream.literal_bytes(&stream[0]);
    let empty = stream.literal_bytes(&stream[1]);
    assert_eq!(first.as_ptr(), stream.text(&stream[0]).as_bytes().as_ptr());
    let original = first.as_ptr();
    stream.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(b"more"));
    assert_eq!(first.as_ptr(), original);
    assert_eq!(first.as_ref(), &[0, 255]);
    drop(stream);
    assert_eq!(first.as_ref(), &[0, 255]);
    assert!(empty.is_empty());
}

#[test]
fn retained_stream_moves_records_and_owns_one_source_reference() {
    let source: Arc<str> = Arc::from("alpha beta");
    let mut stream = TokenStream::with_capacity(&source, 2);
    stream.push_source(TokenKind::Identifier, Span::new(0, 5));
    stream.push_source(TokenKind::Identifier, Span::new(6, 10));
    let records = stream.as_ptr();
    let retained = stream.retain_source(source.clone());
    assert_eq!(retained.as_ptr(), records);
    assert_eq!(Arc::strong_count(&source), 2);
    let cloned = retained.clone();
    assert_eq!(Arc::strong_count(&source), 3);
    drop(source);
    drop(retained);
    assert_eq!(cloned.text(&cloned[0]), "alpha");
    assert_eq!(cloned.text(&cloned[1]), "beta");
}

#[test]
fn decoded_ranges_survive_growth_clone_and_empty_payloads() {
    let mut stream = TokenStream::default();
    for count in 0..100 {
        stream.push_decoded(Span::default(), |bytes| {
            bytes.extend(std::iter::repeat_n(255, count));
        });
    }
    let cloned = stream.clone();
    assert_eq!(cloned, stream);
    drop(stream);
    for (count, token) in cloned.iter().enumerate() {
        assert_eq!(cloned.text(token).as_bytes(), vec![255; count]);
    }
    assert_eq!(cloned.text(&cloned[0]).as_str(), "");
    assert_eq!(cloned.text(&cloned[1]).try_as_str(), None);
}

#[test]
#[should_panic(expected = "retained source must be the exact text")]
fn cannot_retain_an_unrelated_equal_length_source() {
    let stream = TokenStream::with_capacity("alpha", 0);
    let _ = stream.retain_source(Arc::from("other"));
}

#[test]
fn stream_equality_observes_spelling_and_spans_not_source_ownership() {
    let source: Arc<str> = Arc::from("alpha");
    let mut borrowed = TokenStream::with_capacity(&source, 1);
    borrowed.push_source(TokenKind::Identifier, Span::new(0, 5));
    let retained = borrowed.clone().retain_source(source.clone());
    assert_eq!(borrowed, retained);
    let mut other = TokenStream::with_capacity("bravo", 1);
    other.push_source(TokenKind::Identifier, Span::new(0, 5));
    assert_ne!(borrowed, other);
    assert!(!format!("{:?}", borrowed.text(&borrowed[0])).contains("bravo"));
}
