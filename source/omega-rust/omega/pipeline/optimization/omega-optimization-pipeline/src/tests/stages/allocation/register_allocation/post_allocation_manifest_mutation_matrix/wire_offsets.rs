pub(super) struct WireOffsets {
    pub stage: usize,
    pub architecture: usize,
    pub object_format: usize,
    pub completion: usize,
    pub first_transformation: Option<usize>,
    pub spills: usize,
    pub frame: usize,
    pub emission: usize,
    pub publication: usize,
}

pub(super) fn locate(encoded: &[u8]) -> WireOffsets {
    let stage = 44;
    let architecture = stage + 1 + 32;
    let object_format = architecture + 1;
    let completion = object_format + 1 + 8 + 8 + 32;
    let mut cursor = completion + 1;
    match encoded[completion] {
        0 => {}
        1 => cursor += 32,
        tag => panic!("fixture has unknown completion tag {tag}"),
    }
    let transformation_count = read_length(encoded, &mut cursor);
    let first_transformation = (transformation_count != 0).then_some(cursor);
    cursor += transformation_count * 33;
    cursor += 32 * 6;
    let spills = cursor;
    let frame = spills + 1;
    let emission = frame + 1;
    let publication = emission + 1;
    assert_eq!(encoded.len() - (publication + 1), 6 * 8);
    WireOffsets {
        stage,
        architecture,
        object_format,
        completion,
        first_transformation,
        spills,
        frame,
        emission,
        publication,
    }
}

fn read_length(encoded: &[u8], cursor: &mut usize) -> usize {
    let end = cursor.checked_add(8).expect("fixture offset fits usize");
    let length = u64::from_le_bytes(encoded[*cursor..end].try_into().unwrap());
    *cursor = end;
    usize::try_from(length).expect("fixture length fits usize")
}
