//! Immutable source contract for Core's explicit representation constructor.
//!
//! Stage-local declaration selection must authenticate source custody as well
//! as these bytes before assigning compiler meaning to the constructor.

pub const CONSTRUCTOR_NAME: &str = "LayoutStorage";
pub const SOURCE_FILE_NAME: &str = "layout.omg";
pub const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/layout.omg"
));
