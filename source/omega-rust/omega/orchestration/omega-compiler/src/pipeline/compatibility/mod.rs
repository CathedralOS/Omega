//! Test-only compatibility path for pre-Terminal StateGraph fixtures.
//!
//! Production compilation cannot enter this module. It remains only while
//! old executable canaries are ported to the checked-Psi-to-Terminal route.

mod harness;
mod stages;

pub(crate) use harness::StateGraphHarness;
