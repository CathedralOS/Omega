//! Physical roots of the already selected native work. Terminal owns operand
//! coordinates; the ABI geometry owner decides which extents those roots need.

use super::NativeExecutionScope;
use semantic_vocabulary::ScalarApplicationId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalModule;

impl NativeExecutionScope {
    pub(super) fn storage_extent_demand(
        &self,
        module: &TerminalModule,
        layouts: &terminal_installation::AdmittedTargetLayouts,
    ) -> Result<BTreeSet<ScalarApplicationId>, String> {
        let mut declarations = module
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();
        for resolved in layouts.resolved_records() {
            declarations.insert(resolved.id, resolved);
        }
        let mut roots = BTreeSet::new();
        for machine in module
            .machines
            .iter()
            .filter(|machine| self.contains_machine(machine.id))
        {
            roots.extend(
                machine
                    .structural_parameters
                    .iter()
                    .chain(machine.block_structural_parameters())
                    .map(|parameter| parameter.structural_type),
            );
            roots.extend(
                machine
                    .result
                    .structural()
                    .map(|result| result.structural_type),
            );
            for place in &machine.structural_places {
                roots.extend(place.kind.produced_structural_type());
                // An abandoned-construction parent is prefix-proof identity,
                // not an initialized aggregate home. Only this place stores.
            }
            for block in &machine.blocks {
                for operation in &block.operations {
                    roots.extend(
                        operation
                            .result
                            .structural()
                            .map(|result| result.structural_type),
                    );
                    for storage in operation.kind.storage_uses() {
                        terminal_semantics::storage_operand_dependencies(
                            machine,
                            &declarations,
                            storage,
                            &mut roots,
                        )
                        .map_err(|reason| {
                            format!("native storage demand for {:?}: {reason}", operation.id)
                        })?;
                    }
                }
                for projection in block.terminator.storage_uses() {
                    terminal_semantics::storage_operand_dependencies(
                        machine,
                        &declarations,
                        projection.into(),
                        &mut roots,
                    )
                    .map_err(|reason| {
                        format!(
                            "native terminator storage demand for {:?}: {reason}",
                            machine.id
                        )
                    })?;
                }
            }
        }
        for boundary in module
            .boundary_machines
            .iter()
            .filter(|boundary| self.contains_boundary(boundary.id))
        {
            roots.extend(
                boundary
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.structural_type),
            );
            if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
                roots.insert(result.structural_type);
            }
        }
        calling_conventions::terminal_layout::required_scalar_extents(roots, &declarations)
            .map_err(|error| format!("native storage extent demand: {error:?}"))
    }
}
