//! Optimizer module role: stage group. Physical-pipeline coordination tests by exact route.
//!
//! Phase routing and allocation recovery precede ISA-specific post-allocation
//! realization. Cross-rule rejection remains separate from successful route
//! composition, and realization corruption stays with the owning ISA family.

mod allocation_recovery;
