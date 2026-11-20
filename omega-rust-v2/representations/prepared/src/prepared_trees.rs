use crate::{PreparationDependencies, PreparationResourceReceipt, PreparedValue, TargetObservation};
use num_bigint::BigInt;
use omega_resolved::DeclarationId;
use omega_typed::{SpecializedIndexValue, TypeError, TypedTrees};
use std::collections::HashMap;
use std::sync::Arc;

/// Typed trees whose demanded closed indices have been evaluated and materialized.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedTrees<'text> {
    typed: TypedTrees<'text>,
    constants: HashMap<DeclarationId, PreparedValue>,
    open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    specialized_indices: Vec<SpecializedIndexValue>,
    target_observations: Vec<TargetObservation>,
    resource_receipt: Option<PreparationResourceReceipt>,
    dependencies: PreparationDependencies,
}

impl<'text> PreparedTrees<'text> {
    /// Freeze the results from a completed preparation round with the typed
    /// package they describe.
    pub fn from_preparation(
        typed: TypedTrees<'text>,
        constants: HashMap<DeclarationId, PreparedValue>,
        open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
        specialized_indices: Vec<SpecializedIndexValue>,
        target_observations: Vec<TargetObservation>,
        resource_receipt: Option<PreparationResourceReceipt>,
        dependencies: PreparationDependencies,
    ) -> Self {
        Self {
            typed,
            constants,
            open_target_constants,
            specialized_indices,
            target_observations,
            resource_receipt,
            dependencies,
        }
    }

    pub fn typed(&self) -> &TypedTrees<'text> {
        &self.typed
    }

    /// The frozen value of a local integer declaration.
    pub fn integer_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&BigInt> {
        match self.constant(declaration)? {
            PreparedValue::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&PreparedValue> {
        self.constants.get(declaration)
    }

    pub fn dependencies(&self) -> &PreparationDependencies {
        &self.dependencies
    }

    pub fn resource_receipt(&self) -> Option<&PreparationResourceReceipt> {
        self.resource_receipt.as_ref()
    }

    /// Identify the live target dependency of an open constant. The original
    /// initializer remains a typed recipe in the typed stage.
    pub fn open_target_constant(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&TypeError> {
        self.open_target_constants.get(declaration).map(Arc::as_ref)
    }

    /// Target inputs used anywhere in this prepared package, including inputs
    /// that selected an overload during an earlier typing pass.
    pub fn target_observations(&self) -> &[TargetObservation] {
        &self.target_observations
    }

    /// Each answer identifies both the source operation and the owner arguments
    /// that closed it. One generic field can have a different value at each use.
    pub fn specialized_indices(&self) -> &[SpecializedIndexValue] {
        &self.specialized_indices
    }

    /// Extract the typed stage, dropping preparation results owned by this stage.
    pub fn into_typed(self) -> TypedTrees<'text> {
        self.typed
    }
}
