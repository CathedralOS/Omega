use super::*;

fn pair(lower: u32, higher: u32) -> VirtualInterference {
    VirtualInterference {
        lower: VirtualRegisterId(lower),
        higher: VirtualRegisterId(higher),
    }
}

fn original_bytes(rows: &[VirtualInterference]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(rows.len() as u64).to_le_bytes());
    for row in rows {
        bytes.extend_from_slice(&row.lower.0.to_le_bytes());
        bytes.extend_from_slice(&row.higher.0.to_le_bytes());
    }
    bytes
}

fn check(rows: &[VirtualInterference]) {
    let compact: VirtualInterferences = rows.iter().copied().collect();
    assert_eq!(compact.len(), rows.len());
    assert_eq!(compact.is_empty(), rows.is_empty());
    assert_eq!(compact.iter().collect::<Vec<_>>(), rows);
    assert_eq!(format!("{compact:?}"), format!("{rows:?}"));
    assert_eq!(format!("{compact:#?}"), format!("{rows:#?}"));
    assert_eq!(compact, compact.clone());
    assert_eq!(compact, VirtualInterferences::from(rows.to_vec()));
    for index in 0..=rows.len() {
        assert_eq!(compact.get(index), rows.get(index).copied());
    }
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&(compact.len() as u64).to_le_bytes());
    for row in &compact {
        encoded.extend_from_slice(&row.lower.0.to_le_bytes());
        encoded.extend_from_slice(&row.higher.0.to_le_bytes());
    }
    assert_eq!(encoded, original_bytes(rows));
    for lower in [0, 1, 2, u32::MAX] {
        for higher in [0, 1, 2, u32::MAX] {
            let target = pair(lower, higher);
            // Compare the real pinned slice API even on arbitrary unsorted
            // raw input. No sortedness, validity or dedup authority is added.
            assert_eq!(compact.binary_search(&target), rows.binary_search(&target));
        }
    }
    let mut changed = compact.clone();
    changed.push(pair(7, 8));
    assert_ne!(changed, compact);
    changed.clear();
    assert!(changed.is_empty());
    changed.push(pair(u32::MAX, 0));
    assert_eq!(changed.iter().collect::<Vec<_>>(), vec![pair(u32::MAX, 0)]);
    let mut iter = compact.iter();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(iter.len(), rows.len() - index);
        assert_eq!(iter.next(), Some(*row));
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.len(), 0);
}

#[test]
fn compact_edges_match_original_rows_search_debug_identity_and_construction() {
    let alphabet = [pair(0, 0), pair(0, 1), pair(1, 0), pair(1, 1)];
    for len in 0..=5 {
        for mut code in 0..4usize.pow(len) {
            let rows = (0..len)
                .map(|_| {
                    let row = alphabet[code % alphabet.len()];
                    code /= alphabet.len();
                    row
                })
                .collect::<Vec<_>>();
            check(&rows);
        }
    }
    check(&[
        pair(u32::MAX, 0),
        pair(0, u32::MAX),
        pair(u32::MAX, u32::MAX),
    ]);
    check(&[pair(4, 3), pair(4, 3), pair(2, 8), pair(4, 9), pair(4, 0)]);

    // Layout is not equality authority: split adjacent same-lower runs still
    // represent the original Vec sequence and have its exact Debug output.
    let split = VirtualInterferences {
        lower_ids: vec![VirtualRegisterId(4), VirtualRegisterId(4)],
        run_ends: vec![1, 3],
        higher_ids: vec![
            VirtualRegisterId(9),
            VirtualRegisterId(2),
            VirtualRegisterId(2),
        ],
    };
    let merged: VirtualInterferences = vec![pair(4, 9), pair(4, 2), pair(4, 2)].into();
    assert_eq!(split, merged);
    assert_eq!(format!("{split:?}"), format!("{merged:?}"));

    let mut old = Vec::new();
    let mut compact = VirtualInterferences::new();
    for (lower, higher) in [
        (4, vec![9, 1, 1]),
        (4, vec![8, 2]),
        (0, vec![]),
        (2, vec![3, 0]),
    ] {
        let first = old.len();
        for higher in higher {
            let row = pair(lower, higher);
            old.push(row);
            compact.push(row);
        }
        old[first..].sort_unstable();
        compact.sort_lower_tail(first);
        assert_eq!(compact.iter().collect::<Vec<_>>(), old);
    }
    old.sort_unstable();
    old.dedup();
    compact.sort_unstable_and_dedup();
    assert_eq!(compact.iter().collect::<Vec<_>>(), old);

    // A canonical dense graph is constructed directly in the compact owner;
    // the producer need not retain an 8-byte full-pair Vec alongside it.
    let dense: VirtualInterferences = (0..128)
        .flat_map(|lower| (lower + 1..128).map(move |higher| pair(lower, higher)))
        .collect();
    assert_eq!(dense.len(), 128 * 127 / 2);
    assert_eq!(dense.lower_ids.len(), 127);
    assert_eq!(std::mem::size_of::<VirtualInterference>(), 8);
    let payload = dense.higher_ids.len() * std::mem::size_of::<VirtualRegisterId>()
        + dense.lower_ids.len() * std::mem::size_of::<VirtualRegisterId>()
        + dense.run_ends.len() * std::mem::size_of::<usize>();
    assert!(payload < dense.len() * std::mem::size_of::<VirtualInterference>());
}
