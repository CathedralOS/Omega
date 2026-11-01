//! Shared source-authorized semantic reflection plans.
//!
//! Typed preparation and compile-time queries consume the same exact member
//! correspondence. The checking owner is independent of typed preparation;
//! compile-time evaluation depends on preparation and cannot own that shared
//! prerequisite without a dependency cycle.
pub use validation::reflection::*;
