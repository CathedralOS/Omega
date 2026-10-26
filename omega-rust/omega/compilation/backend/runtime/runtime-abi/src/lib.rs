//! Runtime ABI geometry and native host mechanism leaves.
//! Descriptor import identities and calling primitives retain their concrete ABI
//! here.
//!
//! Start at `runtime_abi.rs`: `fat_descriptor_layout` answers the target's
//! two-word descriptor geometry, the only descriptor fact `layout` reads.

mod affine_transform;
mod borrowed_storage;
mod descriptor_io;
mod integer_absolute;
mod integer_division;
mod integer_parsing;
mod native_foreign_frame;
mod network_address;
pub use native_foreign_frame::{
    NATIVE_FOREIGN_FRAME_ARGUMENTS_BYTE_OFFSET, NATIVE_FOREIGN_FRAME_RESULT_BYTE_OFFSET,
    NativeForeignFrame,
};
mod processor_flags;
mod runtime_abi;
mod scalar_reference;
mod windows_provider;

pub use affine_transform::{HostAffineTransform, host_affine_transform};
pub use borrowed_storage::{
    NativeBorrowedStorageDiscriminator, NativeBorrowedStorageField, NativeBorrowedStorageLayout,
    NativeStorageCaseTag, NativeStorageRepetition,
};
pub use descriptor_io::{
    DescriptorAbi, DescriptorOperation, descriptor_import, descriptor_import_identity,
    descriptor_system_call,
};
pub use integer_absolute::host_integer_absolute;
pub use integer_division::{HostIntegerDivision, host_integer_division};
pub use integer_parsing::parse_host_long;
pub use network_address::HostInternetAddress;
pub use processor_flags::capture_host_x86_flags;
pub use runtime_abi::{FatDescriptorLayout, fat_descriptor_layout};
pub use scalar_reference::{ScalarReferenceLayout, ScalarStorageCodec};
pub use windows_provider::{
    WINDOWS_INTERNET_PROVIDER_LIBRARY, image_imports_windows_internet_provider,
    windows_internet_provider_dll,
};
