//! Runtime ABI geometry: the two-word descriptor carriers every backend
//! consumer shares.
//!
//! Start at `runtime_abi.rs`: `RuntimeAbiPlan` carries the target's pointer
//! size, and `FatDescriptorAbi`/`DynamicTraitDescriptorAbi` own the
//! descriptor layouts built from it.
//!
//! @Cleanup: `layout` is the only consumer; terminal emission still re-derives
//! the same geometry by hand. See the @Cleanup notes in `runtime_abi.rs`.

mod runtime_abi;

pub use runtime_abi::*;
