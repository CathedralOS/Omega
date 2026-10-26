//! The fixed host entry used by emitted foreign ABI adapters. Arguments point
//! to complete aligned native value buffers; a borrowed argument buffer holds
//! the pointer supplied by its original-home owner. Geometry grants no custody.

#[repr(C)]
pub struct NativeForeignFrame {
    pub arguments: *const *const u8,
    pub result: *mut u8,
}

pub const NATIVE_FOREIGN_FRAME_ARGUMENTS_BYTE_OFFSET: u32 =
    std::mem::offset_of!(NativeForeignFrame, arguments) as u32;
pub const NATIVE_FOREIGN_FRAME_RESULT_BYTE_OFFSET: u32 =
    std::mem::offset_of!(NativeForeignFrame, result) as u32;

// The supported native host profiles are all 64-bit. Both emitter and loader
// consume this same actual C geometry rather than maintaining independent frames.
const _: () = assert!(std::mem::size_of::<NativeForeignFrame>() == 16);
const _: () = assert!(std::mem::align_of::<NativeForeignFrame>() == 8);
