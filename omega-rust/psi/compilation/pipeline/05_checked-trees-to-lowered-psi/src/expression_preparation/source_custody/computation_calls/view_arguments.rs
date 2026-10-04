//! Readable view operands retain their declared owner and exact call
//! occurrence. A shared or mutable view actual loans an existing caller place whose carrier is
//! itself a view, and a `&[u8]` actual may instead spell a literal payload.
//! The borrowed view has no members, so validation stops at the carrier's
//! element and the authored place's identity; nothing here depends on which
//! element the view carries, so byte and element views share this one check.
