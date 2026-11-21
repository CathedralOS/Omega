use crate::{PreparationDependencies, PreparationResourceReceipt, PreparedFloat, PreparedValue, TargetObservation};
use num_bigint::BigInt;
use omega_resolved::{DeclarationId, TreeNodeId};
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
    /// Sorted by node, then format.
    landed_floats: Vec<(TreeNodeId, PreparedFloat)>,
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
        mut landed_floats: Vec<(TreeNodeId, PreparedFloat)>,
    ) -> Self {
        landed_floats.sort_unstable_by_key(|(node, value)| (*node, value.format() == omega_target_contract::FloatFormat::Binary64));
        landed_floats.dedup();
        Self {
            typed,
            constants,
            open_target_constants,
            specialized_indices,
            target_observations,
            resource_receipt,
            dependencies,
            landed_floats,
        }
    }

    /// The bits of the anonymous numeric expression that typing landed at
    /// `node` in `format`, rounded once to nearest, ties to even. `None` when
    /// no such landing exists or two landings there disagree.
    pub fn landed_float(
        &self,
        node: TreeNodeId,
        format: omega_target_contract::FloatFormat,
    ) -> Option<PreparedFloat> {
        let start = self.landed_floats.partition_point(|(at, _)| *at < node);
        let mut matches = self.landed_floats[start..]
            .iter()
            .take_while(|(at, _)| *at == node)
            .filter(|(_, value)| value.format() == format)
            .map(|(_, value)| *value);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
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
