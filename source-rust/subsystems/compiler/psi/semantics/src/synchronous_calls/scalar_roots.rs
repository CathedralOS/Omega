use super::{SynchronousCallGraph, scalar_conditions};
use semantic_vocabulary::MachineId;
use terminal_psi::{ScalarApplication, TargetMachineCallable};

impl SynchronousCallGraph<'_> {
    /// Find compiler evaluation roots in the selected Product closure. Layout
    /// observations and ordinary storage use the same by-value type graph, and
    /// proof-only observations use the same condition selector as evaluation.
    pub fn scalar_evaluation_roots(
        &self,
        roots: &[MachineId],
        target: Option<&str>,
    ) -> Result<Vec<TargetMachineCallable>, String> {
        let module = self.module;
        let selected = &self.boundary_calls;
        use std::collections::{BTreeSet, VecDeque};
        use terminal_psi::{ArrayExtent, OperationKind, StructuralTypeShape};

        if module.scalar_applications.declarations().is_empty() {
            return Ok(Vec::new());
        }
        let scope = &module.scalar_applications.proof_scope;
        let conditions = &module.scalar_applications.conditions;
        let mut condition_demand = scalar_conditions::ScalarConditionDemand::new(conditions)?;
        let mut proof_entries = Vec::new();
        let mut proof_providers = Vec::new();
        if condition_demand.has_invocation_conditions() {
            proof_providers = scalar_conditions::proof_provider_edges(scope, selected)?;
            for &root in roots {
                proof_entries.push(
                    scope
                        .callable_declaration(TargetMachineCallable::Machine(root))
                        .ok_or("scalar demand root has no checked proof declaration")?,
                );
            }
        }
        let mut pending = VecDeque::new();
        let mut discovered = BTreeSet::new();
        let enqueue = |pending: &mut VecDeque<_>, discovered: &mut BTreeSet<_>, application| {
            if discovered.insert(application) {
                pending.push_back(application);
            }
        };
        condition_demand.include_module(|application| {
            enqueue(&mut pending, &mut discovered, application);
        })?;
        let mut selected_proof_machines = BTreeSet::new();
        let applications = scope.applications();
        let mut proof_walk = applications.invocation_machines_walk(target, &proof_providers);
        let mut callable_roots = roots
            .iter()
            .copied()
            .map(TargetMachineCallable::Machine)
            .collect::<Vec<_>>();
        let mut evaluated = BTreeSet::new();
        let mut visited_callables = BTreeSet::new();
        let storage =
            crate::storage::type_graph::StorageTypeGraph::new(module.structural_types.iter())
                .map_err(|error| format!("scalar demand storage declarations: {error:?}"))?;
        let mut storage = storage.reachability();
        loop {
            if !proof_entries.is_empty() {
                let machines = proof_walk(&proof_entries);
                let declarations =
                    scalar_conditions::invocation_declarations(scope, &proof_entries, &machines);
                proof_entries.clear();
                condition_demand.include_invocation(&machines, &declarations, |application| {
                    enqueue(&mut pending, &mut discovered, application);
                })?;
                selected_proof_machines.extend(machines);
            }
            if !callable_roots.is_empty() {
                let mut storage_roots = Vec::new();
                for machine in self
                    .visit_callables(std::mem::take(&mut callable_roots), &mut visited_callables)?
                    .machines
                {
                    storage_roots.extend(
                        machine
                            .structural_parameters
                            .iter()
                            .map(|parameter| parameter.structural_type),
                    );
                    storage_roots.extend(
                        machine
                            .block_structural_parameters()
                            .map(|parameter| parameter.structural_type),
                    );
                    storage_roots.extend(
                        machine
                            .structural_places
                            .iter()
                            .filter_map(|place| place.kind.produced_structural_type()),
                    );
                    storage_roots.extend(
                        machine
                            .result
                            .structural()
                            .map(|result| result.structural_type),
                    );
                    for operation in machine.operations() {
                        if let OperationKind::ReadScalarApplication { application } = operation.kind
                        {
                            enqueue(&mut pending, &mut discovered, application);
                        }
                        storage_roots.extend(
                            operation
                                .result
                                .structural()
                                .map(|result| result.structural_type),
                        );
                    }
                }
                storage
                    .visit(storage_roots, |declaration| {
                        if let StructuralTypeShape::FixedArray {
                            length: ArrayExtent::Application(application),
                            ..
                        } = declaration.shape
                        {
                            enqueue(&mut pending, &mut discovered, application);
                        }
                    })
                    .map_err(|error| format!("scalar demand storage closure: {error:?}"))?;
            }
            let mut applications = std::mem::take(&mut pending);
            if applications.is_empty() {
                break;
            }
            while let Some(application) = applications.pop_front() {
                condition_demand.include_application(application, |dependency| {
                    enqueue(&mut pending, &mut discovered, dependency);
                })?;
                match module.scalar_applications.declaration(application).copied() {
                    Some(ScalarApplication::LayoutObservation { subject, .. }) => storage
                        .visit([subject], |declaration| {
                            if let StructuralTypeShape::FixedArray {
                                length: ArrayExtent::Application(application),
                                ..
                            } = declaration.shape
                            {
                                enqueue(&mut pending, &mut discovered, application);
                            }
                        })
                        .map_err(|error| format!("scalar demand storage closure: {error:?}"))?,
                    Some(ScalarApplication::Evaluate { entry, .. }) => {
                        if evaluated.insert(entry) {
                            callable_roots.push(entry);
                            let machine = match entry {
                                TargetMachineCallable::Machine(machine) => Some(machine),
                                TargetMachineCallable::Boundary(boundary) => selected
                                    .binary_search_by_key(&boundary, |(boundary, _)| *boundary)
                                    .ok()
                                    .map(|index| selected[index].1),
                            };
                            if condition_demand.has_invocation_conditions()
                                && let Some(machine) = machine
                            {
                                let declaration = scope
                                    .callable_declaration(TargetMachineCallable::Machine(machine))
                                    .ok_or(
                                        "scalar evaluation entry has no checked proof declaration",
                                    )?;
                                if !selected_proof_machines.contains(&declaration) {
                                    proof_entries.push(declaration);
                                }
                            }
                        }
                    }
                    _ => return Err("scalar demand names no retained declaration".into()),
                }
            }
        }
        Ok(evaluated.into_iter().collect())
    }
}
