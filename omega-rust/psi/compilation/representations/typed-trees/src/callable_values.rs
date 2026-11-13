//! Runtime machine values retain a structural contract independently of targets.
//!
//! The declaration coordinate is a lowering memo, not a semantic identity. Copies
//! and applications own new contract rows and clear that coordinate. A complete
//! normalized contract supplies type identity after recursive lowering finishes.

use arena::Handle;
use symbols::SymbolHandle;

pub type RuntimeCallableContractHandle = Handle<RuntimeCallableContract>;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct RuntimeCallableContract {
    pub signature: crate::signature::StateSignature,
    pub lexical_owner: SymbolHandle,
    pub resolved_source: Option<(u32, u32)>,
}

/// An exact named application awaiting ordinary checked contract refinement.
/// Selecting code constructs a value; it does not invoke the selected body.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct MachineValue {
    pub type_reference: crate::types::TypeReferenceHandle,
    pub selected: crate::expression::StaticMachineArgument,
    pub application: crate::static_applications::StaticMachineApplicationHandle,
    pub conformance: SymbolHandle,
    pub requirement: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct RuntimeCallBinding {
    pub callee: crate::expression::ExpressionHandle,
    pub contract: RuntimeCallableContractHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallExpression {
    pub callee: Box<crate::expression::Expression>,
    pub contract: RuntimeCallableContractHandle,
}

impl crate::expression::TableCallExpression {
    pub fn evaluated_callee(&self) -> crate::expression::ExpressionHandle {
        self.runtime_call
            .map_or(self.receiver, |runtime| runtime.callee)
    }
}

impl crate::TypedTrees {
    pub fn runtime_callable_contract(
        &self,
        contract: RuntimeCallableContractHandle,
    ) -> &RuntimeCallableContract {
        self.runtime_callable_contracts.get(contract)
    }
}

impl RuntimeCallBinding {
    pub fn signature<'program>(
        &self,
        program: &'program crate::TypedTrees,
    ) -> Result<&'program crate::signature::StateSignature, &'static str> {
        if !program.runtime_callable_contracts.is_valid(self.contract) {
            return Err("runtime call has no current callable contract");
        }
        Ok(&program
            .runtime_callable_contracts
            .get(self.contract)
            .signature)
    }
}

/// Source coordinates accelerate lowering; the pointed-to row remains authoritative.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct RuntimeCallableLowering {
    sources: Vec<RuntimeCallableContractHandle>,
}

impl RuntimeCallableLowering {
    pub fn from_contracts(
        contracts: &arena::Arena<RuntimeCallableContract>,
    ) -> Result<Self, &'static str> {
        let mut lowering = Self::default();
        for (handle, contract) in contracts.iter() {
            if let Some((index, _)) = contract.resolved_source {
                lowering.retain(index, handle)?;
            }
        }
        Ok(lowering)
    }

    pub fn retain(
        &mut self,
        index: u32,
        contract: RuntimeCallableContractHandle,
    ) -> Result<(), &'static str> {
        let index = index as usize;
        if self.sources.len() <= index {
            self.sources
                .resize(index + 1, RuntimeCallableContractHandle::invalid());
        }
        if self.sources[index].is_valid() {
            return Err("runtime callable contract has duplicate source ownership");
        }
        self.sources[index] = contract;
        Ok(())
    }

    pub fn get(&self, index: u32) -> Option<RuntimeCallableContractHandle> {
        self.sources
            .get(index as usize)
            .copied()
            .filter(|handle| handle.is_valid())
    }
}
