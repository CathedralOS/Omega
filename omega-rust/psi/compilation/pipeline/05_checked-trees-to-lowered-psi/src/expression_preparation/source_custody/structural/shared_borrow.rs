//! Replay custody for a `Reference` selection value carrying a `SharedBorrow`
//! argument. The checked plan recorded the authored `&place` expression's
//! exact root symbol and member/fixed-index path; replay rebuilds that place
//! and requires shared access on the authored borrow, the result type and the
//! argument, the exact path, the projected leaf's identity equal to the
//! result's record referent, and an established structural root place the
//! pipeline can carry -- linear-tolerant, because observing a place moves
//! nothing. There is no owned receipt or transfer to consume: a shared-borrow
//! join is provenance pass-through, so unlike `validate_projection` this leaf
//! leaves owned custody records untouched.
