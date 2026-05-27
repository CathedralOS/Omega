#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    NatDescending,
    SliceLength,
    CustomNatDescending,
}

impl RankingOrder {
    pub(super) fn from_path(
        program: &omega_typed_trees::TypedTrees,
        order: &[omega_typed_trees::name::Identifier],
    ) -> Option<Self> {
        if order.is_empty() || path_matches(order, &["Nat", "Descending"]) {
            return Some(Self::NatDescending);
        }
        if path_matches(order, &["Slice", "Length"]) {
            return Some(Self::SliceLength);
        }
        if custom_nat_descending_order_is_declared(program, order) {
            return Some(Self::CustomNatDescending);
        }

        None
    }
}

fn custom_nat_descending_order_is_declared(
    program: &omega_typed_trees::TypedTrees,
    order: &[omega_typed_trees::name::Identifier],
) -> bool {
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain).iter()),
        )
        .any(|operator| {
            path_matches_path(program.operator_path_members(operator.name), order)
                && program.operator_parameters(operator).len() == 1
                && program
                    .type_reference_table
                    .display_name(operator.return_type)
                    == "usize"
        })
}

fn path_matches_path(
    actual: &[omega_typed_trees::name::Identifier],
    expected: &[omega_typed_trees::name::Identifier],
) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual == expected)
}

fn path_matches(order: &[omega_typed_trees::name::Identifier], expected: &[&str]) -> bool {
    order.len() == expected.len()
        && order
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}
