//! Optimizer module role: stage group. Physical-pipeline coordination tests by exact route.
//!
//! Phase routing and allocation recovery precede ISA-specific post-allocation
//! realization. Cross-rule rejection remains separate from successful route
//! composition, and realization corruption stays with the owning ISA family.

mod aarch64_cbnz;
mod aarch64_movn;
mod allocation_recovery;
mod composition_rejections;
mod phase_routing;
mod x86_mov_after_active_resident;
mod x86_mov_r64_sign_extended_after_active_resident;
mod x86_xor_zero;
