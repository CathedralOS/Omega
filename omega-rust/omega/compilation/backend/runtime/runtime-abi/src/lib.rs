/*
We define the concrete target runtime ABI geometries (`FatDescriptorLayout`,
`NativeForeignFrame`, `ScalarReferenceLayout`, `NativeBorrowedStorageLayout`,
and `static_interrupts`) alongside the native host mechanism leaves consumed by
Terminal execution and emitted foreign adapters.

Every two-word descriptor carrier—slice (`ptr, len`), text window (`ptr, len`),
and borrowed dynamic-trait value (`instance, table`)—shares one target-derived
shape computed by `fat_descriptor_layout`: two pointer-sized words aligned to
`target.pointer_alignment`. Similarly, `NativeForeignFrame` fixes the 16-byte
`{ arguments: *const *const u8, result: *mut u8 }` C frame shared by emitted
foreign ABI adapters and the host loader.
*/

/*
Why keep `fat_descriptor_layout` and `target_carrier_layout` agnostic to which
carrier a descriptor represents? The second word's semantic meaning (element
count, byte count, or conformance-table pointer) belongs to Checked/Terminal
Psi lowering, where applying a slice operation to a dynamic trait value is
rejected by type checking rather than runtime layout inspection. Likewise, host
mechanism leaves (`host_numeric.rs`, `network_address.rs`, `descriptor_io.rs`,
`capture_host_x86_flags`) operate strictly on primitive C ABI values and bounded
host snapshots so Psi never imports native ABI execution types.
*/

/*
Start in this file for descriptor geometry (`fat_descriptor_layout`,
`target_carrier_layout`), the foreign-adapter frame (`NativeForeignFrame`), and
x86 flags observation (`capture_host_x86_flags`). The domain modules cover:
- `borrowed_storage.rs`: native scalar, discriminator, and opaque-region
  coordinates inside borrowed structural storage.
- `scalar_reference.rs`: two-word internal scalar-loan descriptors and the
  20-value `ScalarStorageCodec` encoding.
- `descriptor_io.rs`: normalized libc/CRT/kernel `read` and `write` entry
  identities.
- `host_numeric.rs`: C `abs`, `lldiv`, `strtol`, and macOS
  `CGAffineTransformMake` leaves.
- `network_address.rs` and `windows_provider.rs`: IPv4 classful
  `inet_makeaddr`/`inet_lnaof` evaluation and the companion `omega_host.dll` PE
  image for Windows x64.
- `static_interrupts.rs`: x86-64 static-interrupt token offsets and arrival
  frame geometry.
*/

mod borrowed_storage;
mod descriptor_io;
mod host_numeric;
mod network_address;
mod scalar_reference;
pub mod static_interrupts;
mod windows_provider;

pub use borrowed_storage::{
    NativeBorrowedStorageDiscriminator, NativeBorrowedStorageField, NativeBorrowedStorageLayout,
    NativeStorageCaseTag, NativeStorageRepetition,
};
pub use descriptor_io::{
    DescriptorAbi, DescriptorOperation, descriptor_import, descriptor_import_identity,
    descriptor_system_call,
};
pub use host_numeric::{
    HostAffineTransform, HostIntegerDivision, host_affine_transform, host_integer_absolute,
    host_integer_division, parse_host_long,
};
pub use network_address::HostInternetAddress;
pub use scalar_reference::{ScalarReferenceLayout, ScalarStorageCodec};
pub use target::{CASE_TAG_BYTE_ALIGNMENT, CASE_TAG_BYTE_SIZE};
pub use windows_provider::{
    WINDOWS_INTERNET_PROVIDER_LIBRARY, image_imports_windows_internet_provider,
    windows_internet_provider_dll,
};

use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FatDescriptorLayout {
    pub byte_size: usize,
    pub alignment: usize,
}

pub const fn fat_descriptor_layout(target: NativeTarget) -> FatDescriptorLayout {
    FatDescriptorLayout {
        byte_size: target.pointer_size.saturating_mul(2),
        alignment: target.pointer_alignment,
    }
}

pub const fn target_carrier_layout(
    target: NativeTarget,
) -> semantic_vocabulary::TargetCarrierLayout {
    let descriptor = fat_descriptor_layout(target);
    semantic_vocabulary::TargetCarrierLayout {
        pointer_byte_size: target.pointer_size as u64,
        pointer_byte_alignment: target.pointer_alignment as u64,
        descriptor_byte_size: descriptor.byte_size as u64,
        descriptor_byte_alignment: descriptor.alignment as u64,
        case_tag_byte_size: CASE_TAG_BYTE_SIZE,
        case_tag_byte_alignment: CASE_TAG_BYTE_ALIGNMENT,
    }
}

#[repr(C)]
pub struct NativeForeignFrame {
    pub arguments: *const *const u8, // Points to complete aligned native argument buffers.
    pub result: *mut u8,
}

pub const NATIVE_FOREIGN_FRAME_ARGUMENTS_BYTE_OFFSET: u32 =
    std::mem::offset_of!(NativeForeignFrame, arguments) as u32;
pub const NATIVE_FOREIGN_FRAME_RESULT_BYTE_OFFSET: u32 =
    std::mem::offset_of!(NativeForeignFrame, result) as u32;

const _: () = assert!(std::mem::size_of::<NativeForeignFrame>() == 16);
const _: () = assert!(std::mem::align_of::<NativeForeignFrame>() == 8);

pub fn capture_host_x86_flags() -> Option<u64> {
    #[cfg(target_arch = "x86_64")]
    {
        let image: u64;
        unsafe {
            core::arch::asm!("pushfq", "pop {}", out(reg) image, options(preserves_flags));
        }
        Some(image)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::{Architecture, ObjectFormat};

    #[test]
    fn descriptor_geometry_tracks_the_target_pointer_words() {
        for pointer_size in [4, 8] {
            let target = NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::Coff,
                pointer_size,
                pointer_alignment: pointer_size,
            };
            let layout = fat_descriptor_layout(target);
            assert_eq!(layout.byte_size, pointer_size * 2);
            assert_eq!(layout.alignment, pointer_size);
        }
    }
}
