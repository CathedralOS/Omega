//! Target-resolved storage extent of an internal scalar-reference descriptor.
//! The second lane is a u64 storage codec, independent of the address width.
//! It supplies neither readable access nor a loan or ownership claim.
pub fn scalar_reference_storage_layout(
    pointer_size: u64,
    pointer_alignment: u64,
) -> Option<(u64, u64)> {
    if pointer_size == 0
        || !pointer_alignment.is_power_of_two()
        || pointer_size % pointer_alignment != 0
    {
        return None;
    }
    let codec_offset = pointer_size.checked_add(7)? / 8 * 8;
    let alignment = pointer_alignment.max(8);
    let end = codec_offset.checked_add(8)?;
    Some((
        end.checked_add(alignment - 1)? / alignment * alignment,
        alignment,
    ))
}
