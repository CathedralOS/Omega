//! Scalar reads and storage extents used by the selected machines and boundaries.
use native_execution_reach::ExecutionReach;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{OperationKind, TerminalModule};

pub(super) struct ScalarDemand<'scope> {
    pub(super) emitted: Option<&'scope ExecutionReach>,
    pub(super) owned_sequence_bounds: &'scope [(semantic_vocabulary::StructuralTypeId, u64)],
}

impl ScalarDemand<'_> {
    fn contains_machine(&self, machine: MachineId) -> bool {
        self.emitted
            .is_none_or(|scope| scope.contains_machine(machine))
    }

    fn contains_boundary(&self, boundary: BoundaryMachineId) -> bool {
        self.emitted
            .is_none_or(|scope| scope.contains_boundary(boundary))
    }

    pub(super) fn scalar_application_demand(
        &self,
        module: &TerminalModule,
        layouts: &psi_target_layouts::AdmittedTargetLayouts,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, String> {
        let mut demanded = self.storage_extent_demand(module, layouts)?;
        for machine in module
            .machines
            .iter()
            .filter(|machine| self.contains_machine(machine.id))
        {
            for operation in machine.operations() {
                if let OperationKind::ReadScalarApplication { application } = operation.kind {
                    demanded.push(application);
                }
            }
        }
        let catalog = &module.scalars.scalar_applications;
        for (position, declaration) in catalog.declarations().iter().enumerate() {
            let terminal_psi::ScalarApplication::Evaluate { entry, .. } = declaration else {
                continue;
            };
            let terminal_psi::TargetMachineCallable::Machine(machine) = entry else {
                continue;
            };
            let machine = *machine;
            if !self.contains_machine(machine) {
                continue;
            }
            let application = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(semantic_vocabulary::ScalarApplicationId::new)
                .ok_or("native scalar application coordinate exceeds its catalog")?;
            if catalog.conditions.contains_subject(
                terminal_psi::ScalarConditionSubject::Application(application),
            ) {
                demanded.push(application);
            }
        }
        demanded.sort_unstable();
        demanded.dedup();
        Ok(demanded)
    }
}

mod storage {
    use super::ScalarDemand;
    use semantic_vocabulary::ScalarApplicationId;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{StructuralAccess, StructuralTypeShape, TerminalModule};

    fn transports_borrow_without_inline_storage(
        shape: &StructuralTypeShape,
        access: StructuralAccess,
    ) -> bool {
        use calling_conventions::terminal_layout::{
            borrowed_slice_element, opaque_borrow_transport,
        };
        opaque_borrow_transport(shape, access).is_some()
            || borrowed_slice_element(shape, access).is_some()
    }

    impl ScalarDemand<'_> {
        pub(super) fn storage_extent_demand(
            &self,
            module: &TerminalModule,
            layouts: &psi_target_layouts::AdmittedTargetLayouts,
        ) -> Result<Vec<ScalarApplicationId>, String> {
            let mut declarations = module
                .types
                .structural_types
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect::<BTreeMap<_, _>>();
            for resolved in layouts.resolved_records() {
                declarations.insert(resolved.id, resolved);
            }
            // Opaque borrows transport an address and direct slices transport a
            // descriptor. Neither requires inline storage for the formal.
            let parameter_storage = |formal: &terminal_psi::StructuralParameterDeclaration| {
                let fixed_transport = declarations.get(&formal.structural_type).is_some_and(|ty| {
                    transports_borrow_without_inline_storage(&ty.shape, formal.access)
                });
                (!fixed_transport).then_some(formal.structural_type)
            };
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
                        .filter_map(&parameter_storage),
                );
                roots.extend(
                    machine
                        .result
                        .structural()
                        .map(|result| result.structural_type),
                );
                for place in &machine.structural_places {
                    roots.extend(place.kind.produced_structural_type());
                }
                let borrowed_slices = machine
                    .structural_parameters
                    .iter()
                    .chain(machine.block_structural_parameters())
                    .filter(|formal| {
                        declarations.get(&formal.structural_type).is_some_and(|ty| {
                            calling_conventions::terminal_layout::borrowed_slice_element(
                                &ty.shape,
                                formal.access,
                            )
                            .is_some()
                        })
                    })
                    .map(|formal| formal.place)
                    .collect::<BTreeSet<_>>();
                let include_root = |place| !borrowed_slices.contains(&place);
                let needs_argument_storage = |argument: &terminal_psi::StructuralArgument| {
                    argument.access == terminal_psi::StructuralAccess::Owned
                        || !argument.path.is_empty()
                        || machine
                            .structural_parameter(argument.place)
                            .is_none_or(|formal| parameter_storage(formal).is_some())
                };
                for block in &machine.blocks {
                    for operation in &block.operations {
                        roots.extend(
                            operation
                                .result
                                .structural()
                                .map(|result| result.structural_type),
                        );
                        for (storage, _) in operation
                            .kind
                            .storage_uses_with_arguments(needs_argument_storage)
                        {
                            terminal_semantics::storage_operand_dependencies(
                                machine,
                                &declarations,
                                storage,
                                &mut roots,
                                include_root,
                            )
                            .map_err(|reason| {
                                format!("native storage demand for {:?}: {reason}", operation.id)
                            })?;
                        }
                    }
                    for projection in block
                        .terminator
                        .storage_uses_with_arguments(needs_argument_storage)
                    {
                        terminal_semantics::storage_operand_dependencies(
                            machine,
                            &declarations,
                            projection.into(),
                            &mut roots,
                            include_root,
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
                .interfaces
                .boundary_machines
                .iter()
                .filter(|boundary| self.contains_boundary(boundary.id))
            {
                roots.extend(
                    boundary
                        .structural_parameters
                        .iter()
                        .filter_map(&parameter_storage),
                );
                if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
                    roots.insert(result.structural_type);
                }
            }
            calling_conventions::terminal_layout::StructuralLayouts::with_owned_sequence_bounds(
                declarations.into_values(),
                self.owned_sequence_bounds,
            )
            .map_err(|error| format!("native storage declarations: {error:?}"))?
            .required_scalar_extents(roots)
            .map(|applications| applications.into_iter().collect())
            .map_err(|error| format!("native storage extent demand: {error:?}"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::StructuralTypeId;

        #[test]
        fn opaque_and_slice_borrows_leave_pointee_storage_out_of_formal_demand() {
            let slice = StructuralTypeShape::ElementView {
                element: StructuralTypeId::new(1).unwrap(),
                access: None,
            };
            for shape in [StructuralTypeShape::Opaque, slice] {
                for access in [
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::WriteOnlyBorrow,
                ] {
                    assert!(transports_borrow_without_inline_storage(&shape, access));
                }
                assert!(!transports_borrow_without_inline_storage(
                    &shape,
                    StructuralAccess::Owned
                ));
            }
        }
    }
}
