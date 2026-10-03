//! Discharge exact body-derived incoming descriptor relations at every call.
use crate::validation::ModuleError;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorCompatibilityRequirement,
};

pub(in crate::validation) fn validate_compatibility(
    module: &TerminalModule,
    requirements: &BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
) -> Result<(), ModuleError> {
    if requirements.values().all(Vec::is_empty) {
        return Ok(());
    }
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let mut analysis = DescriptorAnalysis::new(&module.structural_types, &graph);
    let error = |machine, position: terminal_semantics::borrowed_storage::Position| {
        let owner = module
            .machines
            .iter()
            .find(|owner| owner.id == machine)
            .expect("validated machine");
        let operation = owner
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .and_then(|block| block.operations.get(position.1 as usize))
            .map(|operation| operation.id)
            .unwrap_or_else(|| {
                owner
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .next()
                    .expect("derived requirement has an operation")
                    .id
            });
        let place = requirements
            .get(&machine)
            .and_then(|rows| {
                rows.iter().find_map(|row| match &row.demand {
                    DescriptorCallOrigin::Argument { source, .. } => Some(*source),
                    _ => None,
                })
            })
            .or_else(|| owner.structural_places.first().map(|place| place.id))
            .expect("derived storage requirement");
        ModuleError::BorrowedDescriptorLoanDisturbed { operation, place }
    };
    let residuals = analysis
        .compatibility_requirements(requirements)
        .map_err(|failure| error(failure.machine, failure.position))?;
    // These roots may be invoked without an ordinary call represented in the
    // graph. No input declaration alone discharges their relational obligation.
    let mut external = BTreeSet::from([module.entry]);
    external.extend(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    external.extend(
        module
            .closed_conformance_applications
            .iter()
            .flat_map(|application| &application.realization_callables)
            .map(|callable| callable.machine),
    );
    external.extend(
        module
            .dynamic_dispatch
            .direct_dispatches
            .iter()
            .map(|dispatch| dispatch.realization),
    );
    external.extend(
        module
            .dynamic_dispatch
            .indirect_dispatches
            .iter()
            .map(|dispatch| dispatch.realization),
    );
    external.extend(
        module
            .dynamic_dispatch
            .stored_dispatches
            .iter()
            .map(|dispatch| dispatch.realization),
    );
    for machine in external {
        if let Some(requirement) = residuals.get(&machine).and_then(|rows| rows.first()) {
            return Err(error(machine, requirement.position));
        }
    }
    Ok(())
}
