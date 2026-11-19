use arena::{Handle, HandleSpan};

#[test]
fn adjacent_spans_preserve_coordinates_and_ignore_empty_inputs() {
    let mut span = HandleSpan::<u8>::empty();
    span.extend_contiguous(HandleSpan::empty());
    assert!(span.is_empty());
    span.extend_contiguous(HandleSpan::from_parts(Handle::from_parts(4, 9), 2));
    span.extend_contiguous(HandleSpan::empty());
    span.extend_contiguous(HandleSpan::from_parts(Handle::from_parts(6, 9), 3));
    span.push_contiguous(Handle::from_parts(9, 9));
    assert_eq!(span.start(), Handle::from_parts(4, 9));
    assert_eq!(span.count(), 6);
}

#[test]
#[should_panic(expected = "handle span push must be contiguous")]
fn appending_a_span_refuses_a_gap() {
    let mut span = HandleSpan::<u8>::from_parts(Handle::from_parts(4, 9), 2);
    span.extend_contiguous(HandleSpan::from_parts(Handle::from_parts(7, 9), 3));
}

#[test]
#[should_panic(expected = "handle span push must keep one generation")]
fn appending_a_span_refuses_another_generation() {
    let mut span = HandleSpan::<u8>::from_parts(Handle::from_parts(4, 9), 2);
    span.extend_contiguous(HandleSpan::from_parts(Handle::from_parts(6, 10), 3));
}

#[test]
#[should_panic(expected = "handle span count overflow")]
fn appending_a_span_refuses_count_overflow() {
    let mut span = HandleSpan::<u8>::from_parts(Handle::from_parts(1, 9), 1);
    span.extend_contiguous(HandleSpan::from_parts(Handle::from_parts(2, 9), u32::MAX));
}
