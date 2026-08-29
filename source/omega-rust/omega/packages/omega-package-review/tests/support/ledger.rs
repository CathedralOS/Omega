pub(crate) const LEDGER_MAGIC: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER\0";

pub(crate) fn read_ledger_u64(bytes: &[u8], position: &mut usize) -> usize {
    let end = *position + 8;
    let value = u64::from_le_bytes(bytes[*position..end].try_into().unwrap());
    *position = end;
    usize::try_from(value).unwrap()
}

pub(crate) fn ledger_target_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let mut position = LEDGER_MAGIC.len() + 4 * std::mem::size_of::<u16>() + 32;
    let length = read_ledger_u64(bytes, &mut position);
    position..position + length
}

pub(crate) fn ledger_closure_package_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let target = ledger_target_range(bytes);
    let mut position = target.end + 32 + 1;
    let count = read_ledger_u64(bytes, &mut position);
    position..position + count * 32
}

pub(crate) fn ledger_row_frames(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let packages = ledger_closure_package_range(bytes);
    let mut position = packages.end;
    let dependencies = read_ledger_u64(bytes, &mut position);
    for _ in 0..dependencies {
        position += 32;
        let alias_length = read_ledger_u64(bytes, &mut position);
        position += alias_length + 32;
    }
    let rows = read_ledger_u64(bytes, &mut position);
    (0..rows)
        .map(|_| {
            let start = position;
            let length = read_ledger_u64(bytes, &mut position);
            position += length;
            start..position
        })
        .collect()
}
