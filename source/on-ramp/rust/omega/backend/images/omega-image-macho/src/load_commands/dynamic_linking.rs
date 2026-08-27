use crate::bytes::{write_u32, write_u64};
use crate::constants::{
    MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_LOAD_DYLINKER_COMMAND_SIZE,
    MACHO_MAIN_COMMAND_SIZE,
};
use crate::layout::align_to;

pub(crate) fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xe);
    write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
    write_u32(bytes, 12);
    bytes.extend(b"/usr/lib/dyld\0");
    bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
}

pub(crate) fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
    write_u32(bytes, 0x80000028);
    write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
    write_u64(
        bytes,
        u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
    );
    write_u64(bytes, 0);
}

pub(crate) fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 1);
    write_u32(bytes, 3);
    write_u32(bytes, 0);
}

/// A dylib this image links against, in `LC_LOAD_DYLIB` order (index + 1 is the
/// bind-info dylib ordinal). `path` is the absolute install name; the version
/// fields are what the executable claims/requires (dyld checks the loaded dylib's
/// current_version >= our `compatibility_version`, so keep the latter low).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MachoDylib {
    pub(crate) path: &'static str,
    pub(crate) timestamp: u32,
    pub(crate) current_version: u32,
    pub(crate) compatibility_version: u32,
}

impl MachoDylib {
    /// libSystem — ALWAYS ordinal 1. Exact fields preserved so images that import
    /// only libc/libm produce byte-identical load commands to before multi-dylib.
    pub(crate) const LIBSYSTEM: MachoDylib = MachoDylib {
        path: "/usr/lib/libSystem.B.dylib",
        timestamp: 2,
        current_version: 1351 << 16,
        compatibility_version: 1 << 16,
    };
    /// The Objective-C runtime. compat_version 1.0.0 so the dyld version check
    /// passes against any installed libobjc.
    pub(crate) const LIBOBJC: MachoDylib = MachoDylib {
        path: "/usr/lib/libobjc.A.dylib",
        timestamp: 2,
        current_version: 1 << 16,
        compatibility_version: 1 << 16,
    };
    /// Foundation — loaded for its side effect of REGISTERING its classes
    /// (`NSString`, `NSNumber`, …) with the runtime, so `objc_getClass` can find
    /// them. libobjc alone provides only `NSObject` + the runtime. No symbol is
    /// imported from it; it is loaded purely for class registration. compat_version
    /// 1.0.0 so the dyld check passes against any installed Foundation.
    pub(crate) const FOUNDATION: MachoDylib = MachoDylib {
        path: "/System/Library/Frameworks/Foundation.framework/Versions/C/Foundation",
        timestamp: 2,
        current_version: 1 << 16,
        compatibility_version: 1 << 16,
    };
    /// AppKit — registers the windowing classes (`NSApplication`, `NSWindow`,
    /// `NSImageView`, …). Transitively pulls Foundation + CoreGraphics, but we load
    /// them explicitly too. Loaded for class registration; no symbol imported.
    pub(crate) const APPKIT: MachoDylib = MachoDylib {
        path: "/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit",
        timestamp: 2,
        current_version: 1 << 16,
        compatibility_version: 1 << 16,
    };
    /// CoreGraphics — the `CGImage`/`CGColorSpace`/`CGContext` C API for the blit.
    pub(crate) const COREGRAPHICS: MachoDylib = MachoDylib {
        path: "/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics",
        timestamp: 2,
        current_version: 1 << 16,
        compatibility_version: 1 << 16,
    };

    /// The `LC_LOAD_DYLIB` command size: 24-byte header + NUL-terminated install
    /// name, padded up to an 8-byte multiple. (libSystem: 24 + 27 -> 56, matching
    /// the historical `MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE`.)
    pub(crate) fn command_size(&self) -> usize {
        align_to(24 + self.path.len() + 1, 8)
    }
}

/// Emit one `LC_LOAD_DYLIB` load command for `dylib`.
pub(crate) fn write_macho_load_dylib_command(bytes: &mut Vec<u8>, dylib: &MachoDylib) {
    let start = bytes.len();
    let command_size = dylib.command_size();
    write_u32(bytes, 0xc); // LC_LOAD_DYLIB
    write_u32(bytes, command_size as u32);
    write_u32(bytes, 24); // dylib.name str offset (immediately after the header)
    write_u32(bytes, dylib.timestamp);
    write_u32(bytes, dylib.current_version);
    write_u32(bytes, dylib.compatibility_version);
    bytes.extend(dylib.path.as_bytes());
    bytes.push(0);
    bytes.resize(start + command_size, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libsystem_command_size_matches_historical_constant() {
        // The pre-multi-dylib hardcoded size was 56; keeping it byte-identical is
        // what makes images with no second dylib unchanged.
        assert_eq!(MachoDylib::LIBSYSTEM.command_size(), 56);
    }

    #[test]
    fn dylib_command_is_self_describing_and_8_byte_aligned() {
        for dylib in [MachoDylib::LIBSYSTEM, MachoDylib::LIBOBJC] {
            let mut bytes = Vec::new();
            write_macho_load_dylib_command(&mut bytes, &dylib);
            // Emitted bytes == the declared command_size, 8-byte aligned, and the
            // cmdsize field (bytes[4..8]) agrees.
            assert_eq!(bytes.len(), dylib.command_size());
            assert_eq!(bytes.len() % 8, 0);
            assert_eq!(
                u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize,
                dylib.command_size()
            );
            // LC_LOAD_DYLIB and the install name is present + NUL-terminated.
            assert_eq!(
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                0xc
            );
            assert!(bytes[24..].starts_with(dylib.path.as_bytes()));
            assert_eq!(bytes[24 + dylib.path.len()], 0);
        }
    }
}
