//! Independent scalar-function and block liveness replay.
//! Exact selected-edge validation precedes replay-local transport preparation;
//! the full-scan fixed point and result reconstruction remain independent of the
//! producer. Reuse the validated relationships, not a producer's analysis.
