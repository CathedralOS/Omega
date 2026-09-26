//! Optimizer module role: executable entrance. Section placement from current fragment data.
mod fixed_frame;
mod relocation_free;

pub(super) use fixed_frame::place as fixed_frame;
pub(super) use relocation_free::place as relocation_free;
