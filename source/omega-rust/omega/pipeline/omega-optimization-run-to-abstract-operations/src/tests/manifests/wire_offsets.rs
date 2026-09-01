pub(super) struct WireOffsets {
    pub stage: usize,
    pub physical: usize,
    pub vocabulary: usize,
    pub fuel: usize,
    pub selections_payload: usize,
    pub psi_selections_payload: usize,
    pub budget: usize,
    pub decision_payload: usize,
    pub first_pass_payload: usize,
    pub ledger_payload: usize,
    pub bundle_payload: usize,
}

pub(super) fn locate(encoded: &[u8]) -> WireOffsets {
    let stage = 44;
    let physical = stage + 1;
    let vocabulary = physical + 1;
    let fuel = vocabulary + 2 + 32;
    let mut cursor = fuel + 4 + 32 * 3;

    let selections_payload = skip_length_prefixed(encoded, &mut cursor);
    let psi_selections_payload = skip_length_prefixed(encoded, &mut cursor);
    let budget = cursor;
    cursor += 40;
    cursor += 40;
    let decision_payload = skip_length_prefixed(encoded, &mut cursor);
    let pass_count = read_length(encoded, &mut cursor);
    assert!(
        pass_count > 0,
        "fixture must retain at least one pass manifest"
    );
    let first_pass_payload = skip_length_prefixed(encoded, &mut cursor);
    for _ in 1..pass_count {
        skip_length_prefixed(encoded, &mut cursor);
    }
    let ledger_payload = skip_length_prefixed(encoded, &mut cursor);
    let bundle_payload = skip_length_prefixed(encoded, &mut cursor);
    assert_eq!(
        encoded.len() - cursor,
        14 * 8,
        "two seven-axis statistic rows"
    );

    WireOffsets {
        stage,
        physical,
        vocabulary,
        fuel,
        selections_payload,
        psi_selections_payload,
        budget,
        decision_payload,
        first_pass_payload,
        ledger_payload,
        bundle_payload,
    }
}

fn skip_length_prefixed(encoded: &[u8], cursor: &mut usize) -> usize {
    let length = read_length(encoded, cursor);
    let payload = *cursor;
    *cursor = cursor
        .checked_add(length)
        .expect("fixture offset fits usize");
    assert!(*cursor <= encoded.len(), "fixture payload is complete");
    payload
}

fn read_length(encoded: &[u8], cursor: &mut usize) -> usize {
    let end = cursor.checked_add(8).expect("fixture offset fits usize");
    let length = u64::from_le_bytes(encoded[*cursor..end].try_into().unwrap());
    *cursor = end;
    usize::try_from(length).expect("fixture length fits usize")
}
