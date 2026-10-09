//! Replay-owned edge substitutions from independently validated selected input.
//!
//! Transport validation establishes the exact parameter/argument relationships
//! before this preparation. Keep those relationships for all fixed-point sweeps
//! and successor publication instead of resolving them through the full register
//! roster again. No producer flow, liveness result, or retained scratch is read.
