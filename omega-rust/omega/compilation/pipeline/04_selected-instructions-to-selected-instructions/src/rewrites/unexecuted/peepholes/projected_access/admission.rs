//! Producer admission of a projected-access pair.
//!
//! The descriptor table drives the admission: the consumer instruction's
//! semantic kind selects the declared consumers, the pointer operand's
//! in-block producer completes the pair, and the rule's axes — operand
//! shape, displacement bound, unit flow, effect surface — state every gate
//! the concrete records must satisfy. What no catalog declaration can
//! attest is checked on the function itself: the pointer's last definition
//! before the consumer, the base register's freedom from intervening
//! redefinition, the operand classes against the roster, and the
//! whole-function work all charge into the caller's budget. The replay in
//! `replay` re-derives all of it without the table.
