//! Reconcile retained embedded calls against source operations and callee identity.
//!
//! Every retained row in the in-module and installed-provider call families is
//! collected and replayed against the callee's independently reconstructed
//! signature: the embedded plan, scalar and structural argument rows, result
//! identity, and result home are all re-derived from the abstract
//! declarations. The retained target plan, its
//! self-reported placements, and any producer-side digest are never admission
//! authority. A retained row bound to no source call operation, or a second
//! row under an operation key that already has one, is forged by construction.



mod canonical_roots;









