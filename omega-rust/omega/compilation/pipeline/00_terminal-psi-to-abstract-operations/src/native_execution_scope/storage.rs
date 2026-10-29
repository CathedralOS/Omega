//! Physical roots of the already selected native work. Terminal owns operand
//! coordinates; the ABI geometry owner decides which extents those roots need.

use super::NativeExecutionScope;
use semantic_vocabulary::{ScalarApplicationId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralStorageUse, StructuralTypeDeclaration, TerminalMachine, TerminalModule,
};

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
                        retain_storage_roots(machine, &declarations, storage, &mut roots).map_err(
                            |reason| {
                                format!("native storage demand for {:?}: {reason}", operation.id)
                            },
                        )?;
                    }
                }
                for projection in block.terminator.storage_uses() {
                    retain_storage_roots(machine, &declarations, projection.into(), &mut roots)
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

fn retain_storage_roots(
    machine: &TerminalMachine,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    storage: StructuralStorageUse<'_>,
    roots: &mut BTreeSet<StructuralTypeId>,
) -> Result<(), String> {
    let place_type = |place| {
        machine
            .structural_place_type(place)
            .ok_or_else(|| format!("storage operand {place:?} has no declared carrier"))
    };
    match storage {
        StructuralStorageUse::Place(place) => {
            roots.insert(place_type(place)?);
        }
        StructuralStorageUse::Type(structural_type) => {
            roots.insert(structural_type);
        }
        StructuralStorageUse::Projection(projection) => {
            terminal_semantics::projection_storage_dependencies(
                declarations.values().copied(),
                place_type(projection.root)?,
                projection.path,
                roots,
            )
            .ok_or_else(|| format!("malformed storage projection {projection:?}"))?;
        }
        StructuralStorageUse::Canonical { root, path } => {
            terminal_semantics::canonical_storage_dependencies(
                declarations.values().copied(),
                place_type(root)?,
                path,
                roots,
            )
            .ok_or_else(|| {
                format!("malformed declaration-ID storage projection at {root:?}: {path:?}")
            })?;
        }
        StructuralStorageUse::ObservedField { root, path } => {
            let carrier = place_type(root)?;
            roots.insert(carrier);
            // This operation's checked scalar-read contract observes the
            // referent of an initial reference result, not the pointer carrier.
            let observed = match declarations.get(&carrier).map(|row| &row.shape) {
                Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => *referent,
                _ => carrier,
            };
            terminal_semantics::canonical_storage_dependencies(
                declarations.values().copied(),
                observed,
                path,
                roots,
            )
            .ok_or_else(|| {
                format!("malformed scalar-field carrier projection at {root:?}: {path:?}")
            })?;
        }
        StructuralStorageUse::Element { descriptor, path } => {
            let descriptor_type = terminal_semantics::projection_storage_dependencies(
                declarations.values().copied(),
                place_type(descriptor.root)?,
                descriptor.path,
                roots,
            )
            .flatten()
            .ok_or_else(|| format!("element access has no descriptor at {descriptor:?}"))?;
            let Some(terminal_psi::StructuralTypeShape::ElementView { element, .. }) =
                declarations.get(&descriptor_type).map(|row| &row.shape)
            else {
                return Err(format!(
                    "element access carrier {descriptor_type:?} is not a view"
                ));
            };
            terminal_semantics::projection_storage_dependencies(
                declarations.values().copied(),
                *element,
                path,
                roots,
            )
            .ok_or_else(|| {
                format!("malformed element storage projection below {descriptor_type:?}: {path:?}")
            })?;
        }
    }
    Ok(())
}
