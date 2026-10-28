//! Canonical retained-row order. A roster is in canonical form only when the
//! key its producer orders by strictly increases; equal keys would be two rows
//! claiming one slot, and a decreasing pair would be a roster the producer
//! re-sorted. This is the one spelling of that rule for every replay pass.

/// Whether `rows` are strictly increasing under `key`.
pub(crate) fn strictly_increasing<T, K: Ord>(rows: &[T], key: impl Fn(&T) -> K) -> bool {
    rows.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}
