#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    NatDescending,
    SliceLength,
}

impl RankingOrder {
    pub(super) fn from_path(order: &[omega_typed_trees::name::Identifier]) -> Option<Self> {
        if order.is_empty() || path_matches(order, &["Nat", "Descending"]) {
            return Some(Self::NatDescending);
        }
        if path_matches(order, &["Slice", "Length"]) {
            return Some(Self::SliceLength);
        }

        None
    }
}

fn path_matches(order: &[omega_typed_trees::name::Identifier], expected: &[&str]) -> bool {
    order.len() == expected.len()
        && order
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}
