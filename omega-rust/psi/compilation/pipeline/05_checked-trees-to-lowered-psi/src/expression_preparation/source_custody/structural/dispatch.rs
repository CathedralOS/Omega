//! A structural selection replays against its authored `match`: the subject
//! keeps its scalar carrier, the retained arms cover the authored arms in
//! order, each value pattern keeps its operand role and selected equality,
//! and each arm's value is queued for replay under that arm.
