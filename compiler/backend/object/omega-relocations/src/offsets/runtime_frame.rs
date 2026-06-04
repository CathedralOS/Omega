pub(super) fn runtime_frame_index_setup_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match element_byte_size {
        0 => 60 + add_constant_width(field_byte_offset),
        _ => 60 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset),
    }
}

fn scale_index_width(element_byte_size: usize) -> usize {
    let highest_bit = usize::BITS - element_byte_size.leading_zeros();
    let doubles = highest_bit.saturating_sub(1) as usize;
    let additions = element_byte_size.count_ones() as usize;
    8 + (doubles + additions) * 4
}

pub(super) fn add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        unsigned_immediate_width(value as u64) + 4
    }
}

fn unsigned_immediate_width(value: u64) -> usize {
    if value == 0 {
        return 4;
    }

    let mut width = 0;
    for halfword_index in 0..4 {
        if ((value >> (halfword_index * 16)) & 0xffff) != 0 {
            width += 4;
        }
    }
    width
}
