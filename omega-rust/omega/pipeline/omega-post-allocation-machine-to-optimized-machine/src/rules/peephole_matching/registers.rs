use std::collections::BTreeSet;

use omega_register_model::{RegisterUnitId, ValidatedPhysicalRegisterModel};

use super::{
    InstructionPairMatchError, InstructionPairPattern, UnitSetPattern, model::ResolvedNamedUnitSet,
};

pub(super) fn resolve_named_sets(
    pattern: &InstructionPairPattern,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<ResolvedNamedUnitSet>, InstructionPairMatchError> {
    let mut names = BTreeSet::new();
    for unit_pattern in instruction_unit_patterns(pattern)
        .into_iter()
        .chain([pattern.live_through(), pattern.dead_after()])
    {
        names.extend(unit_pattern.0.iter().copied());
    }
    names
        .into_iter()
        .map(|name| {
            let view = physical
                .model()
                .view_named(name)
                .ok_or(InstructionPairMatchError::MissingArchitecturalView(name))?;
            Ok(ResolvedNamedUnitSet {
                name,
                units: view.units.clone(),
            })
        })
        .collect()
}

pub(super) fn units_for(
    pattern: UnitSetPattern,
    named: &[ResolvedNamedUnitSet],
) -> Vec<RegisterUnitId> {
    pattern
        .0
        .iter()
        .flat_map(|name| {
            named
                .iter()
                .find(|set| set.name == *name)
                .into_iter()
                .flat_map(|set| set.units.iter().copied())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn instruction_unit_patterns(pattern: &InstructionPairPattern) -> [UnitSetPattern; 6] {
    [
        pattern.first().implicit_uses,
        pattern.first().implicit_defs,
        pattern.first().implicit_clobbers,
        pattern.second().implicit_uses,
        pattern.second().implicit_defs,
        pattern.second().implicit_clobbers,
    ]
}
