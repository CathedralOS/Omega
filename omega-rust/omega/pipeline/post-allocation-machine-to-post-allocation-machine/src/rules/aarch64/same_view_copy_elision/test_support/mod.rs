//! Optimizer module role: stage group. Shared exact-rule fixtures.

pub(crate) mod fixture;
mod right_operand;

pub(crate) use right_operand::{
    compare_i64_right_operand_fixture, two_pair_compare_i64_right_operand_fixture,
};

fn sorted_units(
    units: impl IntoIterator<Item = register_model::RegisterUnitId>,
) -> Vec<register_model::RegisterUnitId> {
    let mut units = units.into_iter().collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

fn constraint() -> register_model::RegisterConstraintKey {
    register_model::RegisterConstraintKey {
        family: register_model::RegisterConstraintFamily::Instruction,
        variant: 0,
    }
}
