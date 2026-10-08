//! Runtime ABI geometry: the two-word descriptor carriers every backend
//! consumer shares.
//!
//! Start at `runtime_abi.rs`: `RuntimeAbiPlan` carries the target's pointer
//! size, and `FatDescriptorAbi`/`DynamicTraitDescriptorAbi` own the
//! descriptor layouts built from it.
//!
//! @Cleanup: `layout` is the only consumer; terminal emission still re-derives
//! the same geometry by hand. See the @Cleanup notes in `runtime_abi.rs`.

mod borrowed_storage;
mod runtime_abi;
mod scalar_reference;

pub use borrowed_storage::{
    NativeBorrowedStorageField, NativeBorrowedStorageLayout, NativeStorageRepetition,
};

pub use scalar_reference::{ScalarReferenceDescriptorAbi, ScalarStorageCodec};

pub use runtime_abi::{
    DynamicTraitDescriptorAbi, FatDescriptorAbi, FatDescriptorKind, RuntimeAbiPlan, SubsliceLayout,
    build_runtime_abi_plan,
};
