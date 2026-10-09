//! Proposal of the in-block commuting run relocation: admit the window the
//! named run and destination bound, then rotate the run onto the
//! destination's position — the crossed positions shifting one run-width
//! toward the run's vacated span — and rewrite the window's roster rows
//! into the new execution order. Validation independently re-derives the
//! transform and restores the source by content.
