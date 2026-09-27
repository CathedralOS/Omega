//! A structural case-field store rooted at an owned local: the checked plan's
//! destination is `Local`, and the lowering opens the window pair on the
//! local's live home place — the same extraction and restoration contract a
//! borrowed parameter would carry.
