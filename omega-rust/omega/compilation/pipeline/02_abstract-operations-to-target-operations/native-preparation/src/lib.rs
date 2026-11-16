#![forbid(unsafe_code)]

/*
We finish `Omega 02`: turn the target operations it lowered into the execution
plan instruction selection consumes.

Stage 02 already holds everything this needs: the target operations, the
accepted obligation and ownership-frontier facts of the optimized program, the
admitted boundary origins and interrupt installations, and the sealed module.
`prepare_execution_plan` joins them once into a `PreparedExecutionPlan`
(`legalized-operations`), so `Omega 03` reads that plan and none of its sources.

The same crate holds stage 02's validator. `verify_target_operation_plan`
derives the plan a second way, from the abstract program and the optimization
unit, and compares it with the plan preparation published. Ordinary
compilation never calls it; it runs on an explicit validator request and in
tests.
*/

/*
- `legalization.rs`, `legalization_instruction.rs`, `legalization_control.rs`: preparation of the plan from retained target rows.
- `legalization_scalar_graph.rs`, `legalization_reference_custody.rs`, `legalization_target.rs`, `legalization_source.rs`, `legalization_descriptor_inputs.rs`, `legalization_function_inputs.rs`: shared derivation and the independent validator.
- `installed_interrupts.rs`, `interrupt_installation.rs`: the join of the target program to its admitted interrupt installations.
*/

pub(crate) mod native_geometry {
    pub(crate) use legalized_operations::native_geometry::*;

    /// The validator reads the abstract program and the optimization unit
    /// with the same prepared layouts and retained types as the plan.
    pub(crate) type NativePlan<'a> = NativeInput<'a, abstract_operations::AbstractOperationPlan>;
    pub(crate) type NativeUnit<'a> = NativeInput<'a, optimization_unit::PsiOptimizationUnit>;
}
pub(crate) use legalized_operations::structural_inputs;

pub(crate) mod installed_interrupts;
pub(crate) mod interrupt_installation;
mod legalization;
mod legalization_control;
mod legalization_descriptor_inputs;
mod legalization_function_inputs;
mod legalization_instruction;
mod legalization_reference_custody;
mod legalization_scalar_graph;
mod legalization_source;
mod legalization_target;

pub(crate) use legalization::LegalizationError;
pub use legalization::{
    LegalizationError as TargetOperationAuditError, PreparationEvidence, prepare_execution_plan,
    verify_execution_plan, verify_target_operation_plan,
    verify_target_operation_plan_with_closed_module,
};
pub use legalized_operations::native_geometry::retained_types;

#[cfg(test)]
mod sum_layout_selection;
