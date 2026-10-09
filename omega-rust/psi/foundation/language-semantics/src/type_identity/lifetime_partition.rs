/// Canonical public identity of an exact requirement lifetime application.
/// Raw ordinals are private checking custody; only the equality partition in
/// target-trait parameter order crosses provider and package boundaries.
pub fn normalize_requirement_lifetime_partition(raw: &[u32]) -> Vec<u32> {
    let mut representatives = Vec::new();
    raw.iter()
        .map(|ordinal| {
            if let Some(index) = representatives
                .iter()
                .position(|representative| representative == ordinal)
            {
                u32::try_from(index).expect("lifetime partition index exceeds u32")
            } else {
                representatives.push(*ordinal);
                u32::try_from(representatives.len() - 1)
                    .expect("lifetime partition index exceeds u32")
            }
        })
        .collect()
}
