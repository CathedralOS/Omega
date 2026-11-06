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
    let functions = view
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    if functions.len() != view.functions.len() {
        return Err(
            crate::OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None },
        );
    }
    let boundaries = view
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect::<BTreeMap<_, _>>();
    if boundaries.len() != view.boundary_machines.len() {
        return Err(
            crate::OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None },
        );
    }
    let services = crate::unit_validation::services::index_service_catalog(&view)?;
    let (types, domains) =
        crate::unit_validation::structural_catalog::index_structural_catalogs(&view)?;
    let mut accesses = crate::current_ownership::CallAccesses::new(
        &functions,
        boundaries.values().copied(),
        &view.dynamic_dispatch,
        &types,
        &view.structural_types.access_views(),
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
    let mut memberships = graph.as_ref().map(|graph| {
        crate::unit_validation::function_structure::ReferenceMembershipValidation::new(
            &view.structural_types,
            graph,
        )
    });
    let prepared = view
        .functions
        .iter()
        .map(|function| {
            crate::unit_validation::function_structure::prepare_function(
                &view,
                function,
                &functions,
                &boundaries,
                &services,
                &types,
                &domains,
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
        unit: &optimization_unit::PsiOptimizationUnit,
        order: &[MachineId],
        dependencies: &[Vec<usize>],
        unavailable: &[MachineId],
        prepared: &[Result<
            crate::unit_validation::function_structure::FunctionControlFlow<'_>,
            crate::OptimizationUnitValidationError,
        >],
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &BTreeMap<
            semantic_vocabulary::BoundaryMachineId,
            &terminal_psi::BoundaryMachineDeclaration,
        >,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
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
                unit,
                order,
                dependencies,
                unavailable,
                prepared,
                functions,
                boundaries,
                types,
                accesses,
                outputs,
            );
        }
        let result = if unavailable.binary_search(&machine).is_ok() {
            Err("the exact current call operation view is unavailable".to_owned())
        } else if has_unresolved_reference_writer(functions[&machine], types) {
            Err("a boundary call has no independently completed reference-output body".to_owned())
        } else {
            let function = functions[&machine];
            match &prepared[position] {
                Err(error) => Err(format!("reference output prerequisites: {error}")),
                Ok(control) => crate::current_ownership::reconstruct_reference_outputs(
                    unit.psi,
                    &unit.identities,
                    &unit.dynamic_dispatch,
                    function,
                    control.blocks(),
                    control.predecessors(),
                    functions,
                    boundaries,
                    types,
                    accesses,
                )
                .map_err(|error| format!("reference output completion: {error}")),
            }
        };
        outputs.borrow_mut().finish(machine, result);
    }
    for position in 0..machine_order.len() {
        visit(
            position,
            &view,
            &machine_order,
            &dependencies,
            &unavailable,
            &prepared,
            &functions,
            &boundaries,
            &types,
            &mut accesses,
            &outputs,
        );
    }
    let completed = outputs.borrow().clone();
    Ok(completed)
}

fn has_unresolved_reference_writer(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    let declarations = types.values().copied().cloned().collect::<Vec<_>>();
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
                    .writable_slots(&declarations, argument.place, selected, argument.access)
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
        Self { rows }
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
        body: Result<ReferenceBodyOutputs, String>,
    ) {
        let Some(position) = self.position(machine) else {
            return;
        };
        self.rows[position].1 = match body {
            Ok(body) if body.compatibility_requirements().is_empty() => {
                ReferenceOutputState::Complete(Arc::new(body))
            }
            Ok(body) => ReferenceOutputState::Conditional(Arc::new(body)),
            Err(reason) => ReferenceOutputState::Unavailable(reason),
        };
    }
}
