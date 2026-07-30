use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: TypeReferenceHandle,
    pub service_reaches: HandleSpan<Identifier>,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: omega_core::semantics::ServiceReachRowId,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    /// TPR4 (decision 23): the bodyless requirement's authored PUBLIC
    /// guarantee (bare `terminates;`); the conformance check propagates it
    /// to implementations by INHERITANCE. Copied from the resolved record,
    /// never re-derived.
    pub terminates_guarantee: bool,
}

impl Default for StateSignature {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            is_default: false,
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            service_reaches: HandleSpan::empty(),
            service_reach_row: omega_core::semantics::ServiceReachRowId::NULL,
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            terminates_guarantee: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

impl Default for StateParameter {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureContract {
    pub kind: SignatureContractKind,
    pub facts: omega_core::arena::HandleSpan<crate::domain::ProofFact>,
    pub token_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignatureContractKind {
    #[default]
    Requires,
    Ensures,
    Boundary,
}
