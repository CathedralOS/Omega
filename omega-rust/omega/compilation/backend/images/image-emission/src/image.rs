//! The image lane: `image_output` emits an executable image over a sealed
//! object through its direct writers or the `dynamic_elf` lane, and
//! `final_image_validation` replays either for retained-artifact readers.

pub(crate) mod dynamic_elf;
pub(crate) mod final_image_validation;
pub(crate) mod image_output;
