/// The canonical libc/libm umbrella every ordinary darwin import resolves through.
pub const DARWIN_LIBSYSTEM_PATH: &str = "/usr/lib/libSystem.B.dylib";
/// The Objective-C runtime dylib (`objc_msgSend`, `objc_getClass`,
/// `sel_registerName`, …) — a SEPARATE `LC_LOAD_DYLIB` from libSystem.
pub const DARWIN_LIBOBJC_PATH: &str = "/usr/lib/libobjc.A.dylib";
/// CoreGraphics — the `CG*` C API (`CGRectGetMaxX`, `CGImageCreate`,
/// `CGColorSpaceCreateDeviceRGB`, …). A directly-CALLED framework (unlike
/// Foundation/AppKit which load only for objc class registration).
pub const DARWIN_COREGRAPHICS_PATH: &str =
    "/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics";

/// The absolute dylib path a darwin import symbol binds against (its
/// `LC_LOAD_DYLIB`). The Mach-O backend derives each import's dylib ordinal from
/// this, so a program that calls into the Objective-C runtime emits a second load
/// command for libobjc. Everything else (libc/libm) is the libSystem umbrella.
/// Mirrors `windows_import_library`; extends as CoreGraphics/AppKit/Foundation
/// symbols are added. Symbols are the Mach-O `_`-prefixed spellings.
pub fn darwin_import_library(symbol: &str) -> &'static str {
    if symbol.starts_with("_objc_")
        || symbol.starts_with("_sel_")
        || symbol.starts_with("_class_")
        || symbol.starts_with("_object_")
        || symbol.starts_with("_method_")
        || symbol.starts_with("_ivar_")
        || symbol.starts_with("_protocol_")
    {
        return DARWIN_LIBOBJC_PATH;
    }
    // The CoreGraphics C API is `CG`-prefixed (`_CGRectGetMaxX`, `_CGImageCreate`).
    if symbol.starts_with("_CG") {
        return DARWIN_COREGRAPHICS_PATH;
    }
    DARWIN_LIBSYSTEM_PATH
}
