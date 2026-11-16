//! `view as &[T]` re-views the complete window of an existing shared view as
//! another element representation.
//!
//! byte_views.md, "Collection view establishment", `WholeRepresentation`:
//! source and target elements have total, padding-free representations, the
//! target elements exactly tile each source element, and the shared result
//! has `source_length * source_stride / target_stride` elements. A separate
//! obligation proves that count fits `u64`, and the region supplies no
//! mutable access. So a program may read an `&[i32]` as its bytes, learns
//! the byte view's exact length, and still owes the count bound; a target
//! that does not tile the source, a padded element, and a mutable re-view
//! are refused.

use super::*;

/// `accepted`, `rejected` with what the program could not establish, or
/// `not supported: ` and the refusal's text.
fn verdict(body: &str) -> String {
    match super::entryless::judged(body, false) {
        RouteVerdict::Accepted(_) => "accepted".to_owned(),
        RouteVerdict::Rejected { .. } => "rejected".to_owned(),
        RouteVerdict::FallBack(refusal) => format!("not supported: {}", refusal.detail),
        other => panic!("{other:?}"),
    }
}

/// The program is refused for `detail`, in whichever machine forms the view.
fn refused(body: &str, detail: &str) {
    let verdict = verdict(body);
    assert!(
        verdict.starts_with(&format!("not supported: {detail}")),
        "{verdict}"
    );
}

#[test]
fn a_shared_view_is_read_as_the_elements_that_tile_it() {
    // An `i32` holds four bytes, so sixteen words are sixty-four bytes.
    let bytes = |bound: u64| {
        format!(
            "machine bytes_of(words: &[i32]) requires words.len <= 16 {{
    let bytes: &[u8] = words as &[u8];
    take(bytes);
}}
machine take(bytes: &[u8]) requires bytes.len <= {bound} {{ }}"
        )
    };
    assert_eq!(verdict(&bytes(64)), "accepted");
    // The re-view passes straight to a slice parameter as well.
    assert_eq!(
        verdict(
            "machine bytes_of(words: &[i32]) requires words.len <= 16 { take(words as &[u8]); }
machine take(bytes: &[u8]) requires bytes.len <= 64 { }"
        ),
        "accepted"
    );
    // Sixty-three bytes would be a view shorter than its window.
    assert_eq!(verdict(&bytes(63)), "rejected");
    // A padding-free record is tiled by its leaves as well.
    assert_eq!(
        verdict(
            "data Pair { low: u16; high: u16; }
machine halves(pairs: &[Pair]) requires pairs.len <= 8 {
    let halves: &[u16] = pairs as &[u16];
    take(halves);
}
machine take(halves: &[u16]) requires halves.len <= 16 { }"
        ),
        "accepted"
    );
}

#[test]
fn the_converted_count_must_fit_u64() {
    let program = |requires: &str| {
        format!(
            "machine bytes_of(words: &[i32]) -> u64 {requires} {{
    let bytes: &[u8] = words as &[u8];
    bytes.len
}}"
        )
    };
    // A bounded word count has a byte count that fits.
    assert_eq!(verdict(&program("requires words.len <= 16")), "accepted");
    // Nothing bounds the word count, so four bytes per word may overflow.
    assert_eq!(verdict(&program("")), "rejected");
}

#[test]
fn a_re_view_that_does_not_tile_or_writes_is_refused() {
    const UNTILED: &str =
        "a whole-representation view whose target elements do not exactly tile its source elements";
    // One `u16` cannot hold a whole `u32`.
    refused(
        "machine widen(halves: &[u16]) requires halves.len <= 8 {
    let words: &[u32] = halves as &[u32];
}",
        UNTILED,
    );
    // Padding bytes have no valid byte value to read.
    refused(
        "data Padded { tag: u8; value: u32; }
machine bytes_of(rows: &[Padded]) requires rows.len <= 8 {
    let bytes: &[u8] = rows as &[u8];
}",
        UNTILED,
    );
    // A mutable re-view would also owe that every written element leaves
    // the source valid; nothing establishes that, so even bytes re-viewed
    // as bytes are refused when the view writes.
    refused(
        "machine scribble(bytes: &mut [u8]) {
    let same: &mut [u8] = bytes as &mut [u8];
}",
        "expression exclusive storage view without establishment and handback evidence",
    );
}

/// A window of a mutable byte parameter, handed to a shared parameter, reads
/// those bytes as the parameter's own measurement does (the standard
/// library's POSIX write path does this with `buffer[0..count]`).
#[test]
fn a_window_of_a_mutable_byte_parameter_is_lent_for_reading() {
    let program = |requires: &str| {
        format!(
            "machine send(bytes: &[u8]) -> u64 {{ bytes.len }}
machine write(buffer: &mut [u8], count: u64) -> u64 {requires} {{
    send(buffer[0..count])
}}"
        )
    };
    assert_eq!(
        verdict(&program("requires count <= buffer.len")),
        "accepted"
    );
    // Without the bound the window may end past the bytes it views.
    assert_eq!(verdict(&program("")), "rejected");
}
