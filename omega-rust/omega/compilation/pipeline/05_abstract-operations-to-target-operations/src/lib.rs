//! Lower optimized abstract operations, prepare their target instructions and publish the facts needed by emission.
#![forbid(unsafe_code)]

use verified_optimization_input::publication::{
    SemanticEmissionFacts, target_replay::BodyPublication,
};

mod emission_facts;
mod preparation;
mod target_lowering;
pub use target_lowering::{LoweringError, OptimizedTargetLoweringRequest, lower_native_body};
pub mod replay {
    pub use crate::preparation::{
        ExecutionPlanReplayInputs, TargetOperationAuditError, verify_execution_plan,
        verify_target_operation_plan_with_closed_module,
    };
    pub use crate::target_lowering::capture_runtime_borrowed_storage_layouts;
}
/// Only target lowering issues the instructions and obligations for a body together.
///
/// ```compile_fail,E0616
/// fn replace_references(plan: &mut abstract_operations_to_target_operations::NativeBodyPlan) {
///     plan.private_references.clear();
/// }
/// ```
pub struct NativeBodyPlan {
    selection: legalized_operations::PreparedInstructionSelection,
    facts: BodyPublication<SemanticEmissionFacts>,
    private_references: Vec<function_identity::MachineFunctionIdentity>,
}

impl NativeBodyPlan {
    pub fn semantic_facts(
        &self,
    ) -> &verified_optimization_input::publication::SemanticEmissionFacts {
        &self.facts.facts
    }

    pub fn original_for_audit(
        &self,
    ) -> Option<&verified_optimization_input::publication::target_replay::RetainedTargetOperations>
    {
        self.facts.original_for_audit()
    }

    pub fn release_original_for_audit(&mut self) {
        self.facts.release_original();
    }

    pub fn private_references(&self) -> &[function_identity::MachineFunctionIdentity] {
        &self.private_references
    }

    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        &self.selection.selections
    }

    pub fn into_selection(
        self,
    ) -> (
        legalized_operations::PreparedInstructionSelection,
        BodyPublication<SemanticEmissionFacts>,
        Vec<function_identity::MachineFunctionIdentity>,
    ) {
        (self.selection, self.facts, self.private_references)
    }

    pub fn execution_plan(&self) -> &legalized_operations::PreparedExecutionPlan {
        &self.selection.plan
    }
}

pub fn runtime_storage(
    input: &verified_optimization_input::OptimizedAbstractBody,
    target: target::NativeTarget,
) -> Result<runtime_abi::NativeBorrowedStorageLayouts, LoweringError> {
    let (program, facts) = input.parts();
    target_lowering::capture_runtime_borrowed_storage_layouts(program, facts, target)
}
