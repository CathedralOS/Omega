use abstract_operations::AbstractOperation;
use optimization_unit::{OptimizationNode, PsiOptimizationFunction};
use optimization_unit_semantics::descriptor_results::{
    optimized_catalog_graph, optimized_reference_catalog_graph_with_boundary_preservation,
};
use std::borrow::Cow;
use std::cell::OnceCell;
use target_operations::TargetOperationPlan;
use terminal_semantics::descriptor_results::DescriptorMachine;

// Keep each catalog's immutable input graph for all function checks in this
// audit. The view and reference catalogs use different boundary rules; custody
// observations and effects are still computed separately for every function.
pub(crate) struct DescriptorInputs<'a> {
    native: &'a TargetOperationPlan,
    plan: &'a crate::native_geometry::NativePlan<'a>,
    unit: &'a crate::native_geometry::NativeUnit<'a>,
    views: OnceCell<Option<Vec<DescriptorMachine>>>,
    references: OnceCell<Option<Vec<DescriptorMachine>>>,
}

impl<'a> DescriptorInputs<'a> {
    pub(crate) fn new(
        native: &'a TargetOperationPlan,
        plan: &'a crate::native_geometry::NativePlan<'a>,
        unit: &'a crate::native_geometry::NativeUnit<'a>,
    ) -> Self {
        Self {
            native,
            plan,
            unit,
            views: OnceCell::new(),
            references: OnceCell::new(),
        }
    }

    pub(crate) fn native(&self) -> &'a TargetOperationPlan {
        self.native
    }
    pub(crate) fn plan(&self) -> &'a crate::native_geometry::NativePlan<'a> {
        self.plan
    }
    pub(crate) fn unit(&self) -> &'a crate::native_geometry::NativeUnit<'a> {
        self.unit
    }

    pub(crate) fn views(&self) -> Option<&[DescriptorMachine]> {
        self.views
            .get_or_init(|| {
                optimized_catalog_graph(
                    &self.unit.functions,
                    &self.unit.boundary_machines,
                    self.plan.geometry(),
                    &self.unit.dynamic_dispatch,
                    |caller, node| self.operation(caller, node),
                )
            })
            .as_deref()
    }

    pub(crate) fn references(&self) -> Option<&[DescriptorMachine]> {
        self.references
            .get_or_init(|| {
                optimized_reference_catalog_graph_with_boundary_preservation(
                    &self.unit.functions,
                    &self.unit.boundary_machines,
                    self.plan.geometry(),
                    &self.unit.dynamic_dispatch,
                    |caller, node| self.operation(caller, node),
                    |caller, node| self.preserves_boundary(caller, node),
                )
            })
            .as_deref()
    }

    fn operation<'node>(
        &self,
        caller: &PsiOptimizationFunction,
        node: &'node OptimizationNode,
    ) -> Option<Cow<'node, AbstractOperation>> {
        crate::legalization::scalar_graph_input::call_origin::selected_boundary_operation(
            node,
            caller,
            self.native,
            self.plan,
        )
        .ok()
        .map(|selected| match selected {
            Some((operation, _)) => Cow::Owned(operation),
            None => Cow::Borrowed(&node.operation),
        })
    }

    fn preserves_boundary(
        &self,
        caller: &PsiOptimizationFunction,
        node: &OptimizationNode,
    ) -> bool {
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } = &node.operation
        else {
            return false;
        };
        if !completion_claim_sources.is_empty() || !completion_receipts.is_empty() {
            return false;
        }
        let Ok(Some(target_operations::TargetUnitOperation::NormalizedForeignCall {
            boundary: selected_boundary,
            binding,
            ..
        })) = crate::legalization::scalar_graph_input::normalized_foreign::row(
            self.native,
            caller.machine,
            *psi_operation,
        )
        else {
            return false;
        };
        let mut declarations = self
            .unit
            .boundary_machines
            .iter()
            .filter(|declaration| declaration.id == *boundary);
        let Some(declaration) = declarations.next() else {
            return false;
        };
        *selected_boundary == *boundary
            && declarations.next().is_none()
            && arguments.len() == declaration.scalar_parameters.len()
            && structural_arguments.len() == declaration.structural_parameters.len()
            && structural_arguments
                .iter()
                .zip(&declaration.structural_parameters)
                .all(|(actual, formal)| actual.access == formal.access)
            && target_operations::descriptor_input_guard(
                declaration,
                binding,
                self.plan.geometry(),
                |buffer| {
                    self.native
                        .borrowed_storage_layouts
                        .iter()
                        .find(|layout| layout.structural_type == buffer)
                },
            )
            .is_ok_and(|guard| guard.is_some())
    }
}
