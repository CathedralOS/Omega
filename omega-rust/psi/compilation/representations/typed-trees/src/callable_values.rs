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

/// Scalar storage preserves callable contracts independently of numeric carriers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ScalarValueType {
    Primitive(crate::types::PrimitiveType),
    Callable(RuntimeCallableContractHandle),
}

impl ScalarValueType {
    pub fn from_type_reference(
        program: &crate::TypedTrees,
        reference: crate::types::TypeReferenceHandle,
    ) -> Option<Self> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if let crate::types::TypeReferenceNode::Callable { contract } =
            program.type_reference_table.type_reference(reference)
        {
            return program
                .runtime_callable_contracts
                .is_valid(*contract)
                .then_some(Self::Callable(*contract));
        }
        program
            .primitive_type_reference(reference)
            .map(Self::Primitive)
    }

    pub fn same_type(
        self,
        program: &crate::TypedTrees,
        other: Self,
    ) -> Result<bool, crate::runtime_callable_identity::RuntimeCallableIdentityError> {
        match (self, other) {
            (Self::Primitive(left), Self::Primitive(right)) => Ok(left == right),
            (Self::Callable(left), Self::Callable(right)) => {
                if !program.runtime_callable_contracts.is_valid(left)
                    || !program.runtime_callable_contracts.is_valid(right)
                {
                    return Err(crate::runtime_callable_identity::RuntimeCallableIdentityError::MissingContract);
                }
                if left == right {
                    return Ok(true);
                }
                Ok(program.runtime_callable_contract_identity(left, &[])?
                    == program.runtime_callable_contract_identity(right, &[])?)
            }
            _ => Ok(false),
        }
    }

    pub const fn primitive(self) -> Option<crate::types::PrimitiveType> {
        match self {
            Self::Primitive(primitive) => Some(primitive),
            Self::Callable(_) => None,
        }
    }
}

impl From<crate::types::PrimitiveType> for ScalarValueType {
    fn from(primitive: crate::types::PrimitiveType) -> Self {
        Self::Primitive(primitive)
    }
}

impl PartialEq<crate::types::PrimitiveType> for ScalarValueType {
    fn eq(&self, other: &crate::types::PrimitiveType) -> bool {
        self.primitive() == Some(*other)
    }
}
impl PartialEq<ScalarValueType> for crate::types::PrimitiveType {
    fn eq(&self, other: &ScalarValueType) -> bool {
        other == self
    }
}

impl crate::TypedTrees {
    pub fn declared_callable_contract(
        &self,
        contract: RuntimeCallableContractHandle,
    ) -> Result<&RuntimeCallableContract, &'static str> {
        if !self.runtime_callable_contracts.is_valid(contract) {
            return Err("callable type query names a stale signature row");
        }
        let row = self.runtime_callable_contracts.get(contract);
        if !self.symbols.symbols().nodes().is_valid(row.lexical_owner)
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(row.signature.symbol)
        {
            return Err("callable type query has no complete declaration namespace");
        }
        Ok(row)
    }

    /// Read the type owned by a retained callable declaration. `None` means
    /// another source query owns this expression. A known Unit result keeps
    /// its ordinary invalid-handle sentinel; it is not an absent declaration.
    pub fn declared_callable_expression_type_reference(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Result<Option<crate::types::TypeReferenceHandle>, &'static str> {
        use crate::expression::ExpressionNode;
        use crate::types::TypeReferenceNode;
        if !self.expression_table.expression_is_valid(expression) {
            return Err("callable type query names a stale expression");
        }
        let reference = match self.expression_table.expression(expression) {
            ExpressionNode::MachineValue(value) => {
                let mut current = value.type_reference;
                let mut seen = Vec::new();
                let contract = loop {
                    if !self.type_reference_table.contains_type_reference(current)
                        || seen.contains(&current)
                    {
                        return Err("machine value type query has a stale or cyclic type");
                    }
                    seen.push(current);
                    match self.type_reference_table.type_reference(current) {
                        TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                        TypeReferenceNode::Callable { contract } => break *contract,
                        _ => {
                            return Err("machine value type query has no structural callable type");
                        }
                    }
                };
                self.declared_callable_contract(contract)?;
                value.type_reference
            }
            ExpressionNode::Call(call) => {
                let Some(binding) = call.runtime_call else {
                    return Ok(None);
                };
                if call.target_symbol.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.receiver.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.authored_machine_application.is_valid()
                    || call.requirement_application.is_valid()
                    || !call.machine_arguments.is_empty()
                {
                    return Err("callable type query retains conflicting static call metadata");
                }
                if !self.expression_table.expression_is_valid(binding.callee) {
                    return Err("callable type query lost its exact evaluated callee");
                }
                self.declared_callable_contract(binding.contract)?
                    .signature
                    .return_type
            }
            _ => return Ok(None),
        };
        if reference.is_valid() && !self.type_reference_table.contains_type_reference(reference) {
            return Err("callable declaration has a stale result type");
        }
        Ok(Some(reference))
    }
}
