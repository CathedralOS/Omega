//! Explicit interference and directional early-clobber compatibility.

use selected_instructions::VirtualRegisterId;

use selected_instructions::VirtualInterference;

pub(super) fn registers_interfere(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let (lower, higher) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    interference
        .binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}
