//! The control-flow-integrity gate between installation and physical
//! invocation
//! (wiki/spec/build/executable_installation.md#control-flow-integrity).
//!
//! An `InstalledCode` proves a realization exists; nothing in that state may
//! yet be called. An entry becomes reachable only through this gate: the
//! receiver names the exact entry and the contract identity the reference
//! must satisfy, and the provider reports that requirement compatibility and
//! instruction-fetch visibility were established over that entry. Only an
//! established bind produces a sealed `InstalledEntryReference`, which
//! retains the satisfier — the exact installed occurrence and its admitted
//! entry — together with the contract identity.
//!
//! The reference borrows the installed realization rather than copying
//! evidence: retirement consumes `InstalledCode`, so Rust's borrow rules keep
//! the realization unretirable for exactly as long as an entry remains
//! possible. Component boundaries still use bindings, not exported local
//! descriptors — this carrier is sealed custody evidence with no serialized
//! form and no callable address. Missing or mismatched evidence rejects and
//! returns every supplied input.

