//! Prepare exact immutable body views and close dependencies before exposing
//! their normal-return loan graphs. Active or conditional bodies stay unavailable.
use super::*;
use std::borrow::Cow;

/// Reconstruct permissions from the exact current operation view. The caller
/// owns any admitted body selection used by `operation_view`; this result does
/// not select providers or admit control cycles. Unsupported bodies retain an
/// explicit unavailable row while independent bodies can still be checked.
pub fn reconstruct_reference_output_catalog(
    unit: &optimization_unit::PsiOptimizationUnit,
    admitted_cycle_machines: &[MachineId],
    operation_view: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    )
        -> Option<Cow<'node, abstract_operations::AbstractOperation>>,
) -> Result<ReferenceOutputCatalog, crate::OptimizationUnitValidationError> {
    if let Some(pair) = unit
        .functions
        .windows(2)
        .find(|rows| rows[0].machine >= rows[1].machine)
    {
        return Err(crate::OptimizationUnitValidationError::UnorderedMachine(
            pair[1].machine,
        ));
    }
    let mut view = unit.clone();
    let mut unavailable = Vec::new();
    for (source, viewed) in unit.functions.iter().zip(&mut view.functions) {
        for (source_block, viewed_block) in source.blocks.iter().zip(&mut viewed.blocks) {
            for (source_node, viewed_node) in source_block.nodes.iter().zip(&mut viewed_block.nodes)
            {
                match operation_view(source, source_node) {
                    Some(operation)
                        if operation.psi_operation() == source_node.operation.psi_operation() =>
                    {
                        viewed_node.operation = operation.into_owned();
                    }
                    _ => unavailable.push(source.machine),
                }
            }
        }
        viewed.facts = crate::unit_validation::function_structure::reconstruct_fact_index(viewed);
    }
    unavailable.sort_unstable();
    unavailable.dedup();
    view.identity = optimization_unit::recompute_psi_optimization_unit_identity(&view);
    let catalogs = crate::unit_validation::prepare_unit_catalogs(&view)?;
    prepare_reference_output_catalog(
        &catalogs.function_catalogs(),
        admitted_cycle_machines,
        &unavailable,
    )
}

/// Reuse the exact catalogs already prepared by the ordinary unit owner. The
/// external normalization route prepares its own view once before entering.
pub(crate) fn prepare_reference_output_catalog(
    function_catalogs: &crate::unit_validation::function_structure::FunctionCatalogs<'_, '_>,
    admitted_cycle_machines: &[MachineId],
    unavailable: &[MachineId],
) -> Result<ReferenceOutputCatalog, crate::OptimizationUnitValidationError> {
    let view = function_catalogs.unit;
    let functions = function_catalogs.functions;
    let boundaries = function_catalogs.boundary_machines;
    let types = function_catalogs.structural_types;
    let mut accesses = crate::current_ownership::CallAccesses::new(
        &functions,
        boundaries.values().copied(),
        &view.dynamic_dispatch,
        &types,
        &view.structural_types,
    );
    let outputs = Rc::new(RefCell::new(ReferenceOutputCatalog::new(
        view.functions.iter().map(|function| function.machine),
    )));
    accesses
        .references
        .use_output_preparation(Rc::clone(&outputs));
    let mut cycles = crate::unit_validation::function_structure::ControlCyclePolicy::default();
    for &machine in admitted_cycle_machines {
        cycles.admit(machine);
    }
    let graph = crate::descriptor_results::optimized_graph(&view);
    let borrowed_graph = graph.as_ref().and_then(|_| {
        crate::descriptor_results::optimized_borrowed_storage_catalog_graph(
            view.functions.iter(),
            boundaries.values().copied(),
            &view.structural_types,
            &view.dynamic_dispatch,
        )
    });
    let mut memberships = graph.as_ref().map(|graph| {
        crate::unit_validation::function_structure::ReferenceMembershipValidation::new(
            &view.structural_types,
            graph,
            borrowed_graph.as_deref(),
        )
    });
    let prepared = view
        .functions
        .iter()
        .map(|function| {
            crate::unit_validation::function_structure::prepare_function(
                function_catalogs,
                function,
                &cycles,
                &mut accesses,
                memberships.as_mut(),
            )
        })
        .collect::<Vec<_>>();
    let machine_order = functions.keys().copied().collect::<Vec<_>>();
    let dependencies = machine_order
        .iter()
        .map(|machine| {
            let mut dependencies = functions[machine]
                .block_nodes()
                .filter_map(|node| match node.operation {
                    abstract_operations::AbstractOperation::CallStructural {
                        callee: abstract_operations::AbstractCallTarget::Machine(machine),
                        ..
                    } => machine_order.binary_search(&machine).ok(),
                    _ => None,
                })
                .collect::<Vec<_>>();
            dependencies.sort_unstable();
            dependencies.dedup();
            dependencies
        })
        .collect::<Vec<_>>();
    #[allow(clippy::too_many_arguments)]
    fn visit(
        position: usize,
        order: &[MachineId],
        dependencies: &[Vec<usize>],
        unavailable: &[MachineId],
        prepared: &[Result<
            crate::unit_validation::function_structure::FunctionControlFlow<'_>,
            crate::OptimizationUnitValidationError,
        >],
        catalogs: &crate::unit_validation::function_structure::FunctionCatalogs<'_, '_>,
        accesses: &mut crate::current_ownership::CallAccesses<'_>,
        outputs: &ReferenceOutputPreparation,
    ) {
        let machine = order[position];
        if !outputs.borrow_mut().begin(machine) {
            return;
        }
        for &dependency in &dependencies[position] {
            visit(
                dependency,
                order,
                dependencies,
                unavailable,
                prepared,
                catalogs,
                accesses,
                outputs,
            );
        }
        let result = if unavailable.binary_search(&machine).is_ok() {
            Err("the exact current call operation view is unavailable".to_owned())
        } else if has_unresolved_reference_writer(catalogs, catalogs.functions[&machine]) {
            Err("a boundary call has no independently completed reference-output body".to_owned())
        } else {
            let function = catalogs.functions[&machine];
            match &prepared[position] {
                Err(error) => Err(format!("reference output prerequisites: {error}")),
                Ok(control) => control
                    .replay_ownership(catalogs, function, accesses, &mut Vec::new())
                    .map(|completion| {
                        completion.into_reference_outputs(function, &catalogs.unit.structural_types)
                    })
                    .map_err(|error| format!("reference output completion: {error}")),
            }
        };
        outputs.borrow_mut().finish(machine, result);
    }
    for position in 0..machine_order.len() {
        visit(
            position,
            &machine_order,
            &dependencies,
            unavailable,
            &prepared,
            function_catalogs,
            &mut accesses,
            &outputs,
        );
    }
    let completed = outputs.borrow().clone();
    Ok(completed)
}

fn has_unresolved_reference_writer(
    catalogs: &crate::unit_validation::function_structure::FunctionCatalogs<'_, '_>,
    function: &PsiOptimizationFunction,
) -> bool {
    let types = catalogs.structural_types;
    let declarations = &catalogs.unit.structural_types;
    function.block_nodes().any(|node| {
        let abstract_operations::AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } = &node.operation
        else {
            return false;
        };
        structural_arguments.iter().any(|argument| {
            crate::unit_validation::operation_contracts::structural_source_contract(
                function,
                argument.place,
            )
            .and_then(|source| {
                crate::unit_validation::references::access_path_type(
                    types,
                    source.structural_type,
                    &argument.path,
                )
            })
            .and_then(|selected| {
                terminal_semantics::descriptor_results::DescriptorSlotKind::References
                    .writable_slots(declarations, argument.place, selected, argument.access)
            })
            .is_none_or(|slots| !slots.is_empty())
        })
    })
}

impl ReferenceOutputCatalog {
    pub fn body(&self, machine: MachineId) -> Option<Arc<ReferenceBodyOutputs>> {
        match &self.rows.get(self.position(machine)?)?.1 {
            ReferenceOutputState::Complete(body) => Some(Arc::clone(body)),
            _ => None,
        }
    }

    pub(crate) fn normally_completed(&self, machine: MachineId) -> bool {
        self.position(machine)
            .is_some_and(|position| self.normal_completion[position])
    }

    pub fn unavailable_reason(&self, machine: MachineId) -> Option<&str> {
        match &self.rows.get(self.position(machine)?)?.1 {
            ReferenceOutputState::Unavailable(reason) => Some(reason),
            ReferenceOutputState::Conditional(_) => {
                Some("reference output custody still has caller compatibility requirements")
            }
            _ => None,
        }
    }

    pub fn compatibility_requirements(
        &self,
        machine: MachineId,
    ) -> Option<&[terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement]> {
        match &self.rows.get(self.position(machine)?)?.1 {
            ReferenceOutputState::Complete(body) | ReferenceOutputState::Conditional(body) => {
                Some(body.compatibility_requirements())
            }
            _ => None,
        }
    }

    fn position(&self, machine: MachineId) -> Option<usize> {
        self.rows
            .binary_search_by_key(&machine, |(candidate, _)| *candidate)
            .ok()
    }

    pub(crate) fn new(machines: impl Iterator<Item = MachineId>) -> Self {
        let mut rows = machines
            .map(|machine| (machine, ReferenceOutputState::Pending))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(machine, _)| *machine);
        rows.dedup_by_key(|(machine, _)| *machine);
        Self {
            normal_completion: vec![false; rows.len()],
            rows,
        }
    }

    pub(crate) fn begin(&mut self, machine: MachineId) -> bool {
        let Some(position) = self.position(machine) else {
            return false;
        };
        if !matches!(self.rows[position].1, ReferenceOutputState::Pending) {
            return false;
        }
        self.rows[position].1 = ReferenceOutputState::Active;
        true
    }

    pub(crate) fn finish(
        &mut self,
        machine: MachineId,
        body: Result<ReferenceBodyCompletion, String>,
    ) {
        let Some(position) = self.position(machine) else {
            return;
        };
        let body = body.and_then(|completion| {
            self.normal_completion[position] = completion.unconditional;
            completion.outputs
        });
        self.rows[position].1 = match body {
            Ok(body) if body.compatibility_requirements().is_empty() => {
                ReferenceOutputState::Complete(Arc::new(body))
            }
            Ok(body) => ReferenceOutputState::Conditional(Arc::new(body)),
            Err(reason) => ReferenceOutputState::Unavailable(reason),
        };
    }
}
