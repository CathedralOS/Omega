//! Replay-owned general candidates for one immutable function and register environment.
//!
//! Register-order replay revisits a location for every live value. Keep only visited
//! class/location rows for this invocation; no row survives into another function or
//! admission. Derivation remains independent of the producer. Fixed-view checks stay
//! at each original register occurrence, including views excluded from general
//! allocation, so preparation cannot change their error order or authority.
