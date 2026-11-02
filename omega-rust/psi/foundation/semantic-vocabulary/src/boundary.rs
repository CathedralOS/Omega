//! Coordinates at the program's boundary: `callback_identities` for boundary
//! parameter telescopes, `foreign_locator` for native locators before target
//! validation, and `program_entry` for the entry requirements a target's slot
//! catalog supplies.

pub(crate) mod callback_identities;
pub(crate) mod foreign_locator;
pub(crate) mod program_entry;
