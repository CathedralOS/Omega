//! Calls and possible code values discovered from the program’s execution roots.
//! The checker, interpreter and native compiler share these results after analysis.

use crate::types::descriptor_locations::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition,
};
use semantic_vocabulary::{
    BoundaryMachineId, CallableContractId, MachineDescriptorId, MachineId, OperationId,
    StructuralTypeId, ValueId,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallableParameter {
    Scalar(usize),
    Structural {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallableFormalInput {
    pub machine: MachineId,
    pub parameter: CallableParameter,
    pub contract: CallableContractId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallableValuePossibilities {
    pub descriptors: Vec<MachineDescriptorId>,
    pub inputs: Vec<CallableFormalInput>,
    pub unresolved_ceilings: Vec<CallableContractId>,
}

impl CallableValuePossibilities {
    pub fn is_resolved(&self) -> bool {
        !self.descriptors.is_empty()
            && self.inputs.is_empty()
            && self.unresolved_ceilings.is_empty()
    }

    pub fn is_bottom(&self) -> bool {
        self.descriptors.is_empty() && self.inputs.is_empty() && self.unresolved_ceilings.is_empty()
    }

    pub fn join(&mut self, other: Self) -> bool {
        let before = (
            self.descriptors.len(),
            self.inputs.len(),
            self.unresolved_ceilings.len(),
        );
        join_ordered(&mut self.descriptors, other.descriptors);
        join_ordered(&mut self.inputs, other.inputs);
        join_ordered(&mut self.unresolved_ceilings, other.unresolved_ceilings);
        before
            != (
                self.descriptors.len(),
                self.inputs.len(),
                self.unresolved_ceilings.len(),
            )
    }

    pub fn ceiling(contract: CallableContractId) -> Self {
        Self {
            unresolved_ceilings: vec![contract],
            ..Self::default()
        }
    }
}

fn join_ordered<T: Ord>(values: &mut Vec<T>, mut incoming: Vec<T>) {
    values.append(&mut incoming);
    values.sort_unstable();
    values.dedup();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallableInvocationTarget {
    Machine(MachineId),
    Value(ValueId, CallableContractId),
    Opaque,
}

pub use semantic_vocabulary::CallableInvocationSite;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableInvocation {
    pub site: CallableInvocationSite,
    pub operation: Option<OperationId>,
    pub boundary: Option<BoundaryMachineId>,
    pub position: DescriptorPosition,
    pub successor: Option<usize>,
    pub target: CallableInvocationTarget,
    pub scalar_arguments: Vec<ValueId>,
    pub structural_arguments: Vec<DescriptorLocation>,
    pub structural_actual_types: Vec<(StructuralTypeId, crate::StructuralAccess)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReachedCallableTarget {
    Declaration(MachineId),
    Callable(CallableValuePossibilities),
    /// The current operation has opaque code, not an absent call edge.
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableCallableInvocation {
    pub caller: MachineId,
    pub invocation: CallableInvocation,
    pub target: ReachedCallableTarget,
    pub selected_machines: Vec<MachineId>,
}

impl ReachableCallableInvocation {
    pub fn targets_are_resolved(&self) -> bool {
        !self.selected_machines.is_empty()
            && match &self.target {
                ReachedCallableTarget::Declaration(_) => true,
                ReachedCallableTarget::Callable(values) => values.is_resolved(),
                ReachedCallableTarget::Opaque => false,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableCallableInvocations {
    pub roots: Vec<MachineId>,
    pub reached_machines: Vec<MachineId>,
    pub invocations: Vec<ReachableCallableInvocation>,
    pub inputs: Vec<(CallableFormalInput, CallableValuePossibilities)>,
}

impl ReachableCallableInvocations {
    pub fn site(
        &self,
        caller: MachineId,
        site: CallableInvocationSite,
    ) -> Option<&ReachableCallableInvocation> {
        self.invocations
            .binary_search_by_key(&(caller, site), |row| (row.caller, row.invocation.site))
            .ok()
            .map(|position| &self.invocations[position])
    }

    pub fn operation(
        &self,
        caller: MachineId,
        operation: OperationId,
    ) -> Result<Option<&ReachableCallableInvocation>, &'static str> {
        let mut matches = self
            .invocations
            .iter()
            .filter(|row| row.caller == caller && row.invocation.operation == Some(operation));
        let first = matches.next();
        if matches.next().is_some() {
            return Err("reached callable invocation has an ambiguous operation association");
        }
        Ok(first)
    }
}
