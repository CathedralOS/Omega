//! Replay custody for an element-view selection value: a `BorrowedSliceView`
//! carrying a `SharedBorrow` argument or a `MutableSliceView` carrying a
//! `MutableBorrow` one. The checked plan recorded the authored
//! `place.as_slice()`, `place.as_mut_slice()`, or whole-collection borrow's exact collection root
//! symbol and member path; replay rebuilds that collection place and requires
//! the view's access on the argument, the exact path, and the projected
//! leaf's identity equal to the argument's recorded collection identity. Like
//! `shared_borrow`, a view join is provenance pass-through under either
//! access: it leaves owned custody records untouched — loan exclusivity is
//! the checked planner's `Mutable`-kind roster, not a replay concern. The two
//! accesses replay one walk and differ only in the planned access they
//! require and the view family their refusals name.
//! Retained range operations use the shared range source validator, including
//! array-backed byte views and both pure and computed endpoint operands.
