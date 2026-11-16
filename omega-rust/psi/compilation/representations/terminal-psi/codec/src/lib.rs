#![forbid(unsafe_code)]

/*
We own the sealed Terminal Psi module (`ClosedTerminalModule`), the canonical artifact envelope
(`CanonicalTerminalArtifact`) with the execution-code views that borrow or retain it, and entry
receipts. The canonical wire encoding, fingerprints, structural-foundation validation and the
companion sections live in `terminal-wire`; this crate re-exports all of it so existing users name
one crate.

A native form that needs only a canonical vocabulary encoder (a proposition order key, a crash
route or catalog encoding) depends on `terminal-wire` directly, so it cannot name the sealed
module.
*/

/*
Why enforce a single canonical byte encoding and run structural-foundation validation inside the
decoder rather than trusting deserialized tables? In Omega's separate-compilation
architecture, a `SemanticFingerprint` must identify the exact semantic
program across hosts, caches, and package boundaries. Re-encoding every decoded module and
rejecting any byte stream that deviates from canonical table order, violates structural-foundation
closure, or carries trailing bytes gives a single byte representation of portable
semantics. The digest commits to those bytes; it does not establish their truth
or authorize execution.
*/

/*
The lowered-form checker's accepted closure publishes canonical Psi artifacts
through this codec. The interpreter, native lowering and inspection paths borrow
the same immutable closed module or decode external bytes at ingress. Receiving
host authorization of an exact producer and artifact remains separate from
decoding, structural validation and carried proof data.
*/

pub use terminal_wire::*;

mod canonical_artifact;
mod closed_module;
pub use closed_module::ClosedTerminalModule;
pub mod entry_receipt;
pub(crate) use canonical_artifact::execution_code;
#[cfg(test)]
mod qualified_cleanup_tests;

pub(crate) use optimization::{codec_bytes, cursor};

pub use canonical_artifact::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactEnvelopeError,
    CanonicalTerminalArtifactError, TerminalArtifactView,
};
pub use execution_code::{
    InvocationTerminalCode, RetainedTerminalCode, TerminalExecutionIdentity, TerminalExecutionView,
    TerminalProgramProduct, TerminalPublication,
};
