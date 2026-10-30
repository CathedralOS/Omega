//! Exactly-one selection over a roster scan.
//!
//! A caller that selects a declaration, field, case or row by identity must
//! refuse a roster that carries two entries for it: silently taking the first
//! would let a duplicated entry decide a meaning the caller never asked for.
//! Every such site states the same decision — one matching entry is the answer,
//! none or several is `None` — so the scan is written once here and the call
//! sites state only which entries they consider.
//!
//! This is selection, not validation: it reports that the roster is not
//! single-valued but never says what a duplicated entry would mean.

/// The one entry of `rows` that `keep` accepts, or `None` when it accepts none
/// or several.
pub(crate) fn exactly_one<'row, Row>(
    rows: impl Iterator<Item = &'row Row>,
    keep: impl Fn(&Row) -> bool,
) -> Option<&'row Row> {
    let mut matching = rows.filter(|row| keep(row));
    let row = matching.next()?;
    matching.next().is_none().then_some(row)
}
