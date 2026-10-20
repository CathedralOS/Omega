//! Runtime ABI geometry and native host mechanism leaves.
//! Descriptor carriers and calling primitives retain their concrete ABI here.
//!
//! Start at `runtime_abi.rs`: `RuntimeAbiPlan` carries the target's pointer
//! size, and `FatDescriptorAbi`/`DynamicTraitDescriptorAbi` own the
//! descriptor layouts built from it.
//!
//! @Cleanup: `layout` is the only consumer; terminal emission still re-derives
//! the same geometry by hand. See the @Cleanup notes in `runtime_abi.rs`.

mod borrowed_storage;
mod descriptor_io;
mod network_address;
mod processor_flags;
mod runtime_abi;
mod scalar_reference;

pub use borrowed_storage::{
    NativeBorrowedStorageDiscriminator, NativeBorrowedStorageField, NativeBorrowedStorageLayout,
    NativeStorageCaseTag, NativeStorageRepetition,
};
pub use descriptor_io::{
    DescriptorAbi, DescriptorOperation, descriptor_import, descriptor_system_call,
};
pub use network_address::HostInternetAddress;
pub use processor_flags::capture_host_x86_flags;
pub use runtime_abi::build_runtime_abi_plan;
pub use scalar_reference::ScalarStorageCodec;
