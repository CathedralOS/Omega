//! Explicit frame geometry and encoding admission; no physical construction.
#![forbid(unsafe_code)]
pub mod frame_layout;
pub mod frame_protocol;
fn count_where<T>(rows: &[T], predicate: impl Fn(&T) -> bool) -> usize {
    rows.iter().filter(|row| predicate(row)).count()
}
