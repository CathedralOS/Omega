//! Replay symbolic descriptor compatibility against the current unit's calls.
use crate::OptimizationUnitValidationError;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorCompatibilityRequirement,
};

pub(crate) fn validate_compatibility(
    unit: &PsiOptimizationUnit,
    requirements: &BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
) -> Result<(), OptimizationUnitValidationError> {
    if requirements.values().all(Vec::is_empty) {
        return Ok(());
    }
    let error = |machine, position: terminal_semantics::borrowed_storage::Position| {
        let function = unit.function(machine).expect("validated function");
        let place = requirements
            .get(&machine)
            .and_then(|rows| {
                rows.iter().find_map(|row| match &row.demand {
                    DescriptorCallOrigin::Argument { source, .. } => Some(*source),
                    _ => None,
                })
            })
            .or_else(|| function.structural_places.first().map(|place| place.id))
            .expect("derived storage requirement");
        OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
            machine,
            block: position.0,
            node: position.1,
            place,
        }
    };
    let graph = crate::descriptor_results::optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        &unit.dynamic_dispatch,
        |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
    )
    .ok_or_else(|| {
        let (&machine, rows) = requirements
            .iter()
            .find(|(_, rows)| !rows.is_empty())
            .expect("nonempty requirements");
        error(machine, rows[0].position)
    })?;
    let mut analysis = DescriptorAnalysis::new(&unit.structural_types, &graph);
    let residuals = analysis
        .compatibility_requirements(requirements)
        .map_err(|failure| error(failure.machine, failure.position))?;
    let mut external = BTreeSet::from([unit.entry]);
    external.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    for machine in external {
        if let Some(requirement) = residuals.get(&machine).and_then(|rows| rows.first()) {
            return Err(error(machine, requirement.position));
        }
    }
    Ok(())
}
