//! The uninstall join over the retirement and quarantine transitions
//! (wiki/spec/build/executable_installation.md#visibility-and-retirement).
//!
//! One decision point consumes installed code toward its two endings: a
//! complete drain retires the realization and returns the placement for
//! reuse, and a drain that cannot complete — quiescence unproven, residual
//! authority outstanding, or an opaque holder — routes to the trapping
//! quarantine that reserves the range. A provider that reports an incomplete
//! drain supplies both receipts; a caller that omits the quarantine receipt
//! asks for retirement only and keeps every input when it fails.
//!
//! Both legs validate the complete installed evidence independently, so a
//! mismatched retirement receipt never reaches quarantine on its word, and a
//! quarantine receipt naming a different realization rejects the same way it
//! does at the direct crossing. Any refusal returns every supplied input:
//! nothing about the mapping's custody is lost to a failed transition.

