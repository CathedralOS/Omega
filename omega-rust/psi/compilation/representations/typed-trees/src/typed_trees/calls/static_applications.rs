//! Authored static applications retained independently of executable calls.
//!
//! Specialization consumes static operands and retargets execution to a concrete
//! state. That rewrite cannot consume the source edge checking must admit. This
//! carrier holds declaration and operand identities, not an admitted selection
//! or a trust receipt. Both expression and statement calls refer to these rows
//! in the shared expression payload arenas; their zero handle means no retained
//! application. Copies own fresh rows before lexical and type substitution.

use crate::expression::StaticMachineArgument;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

pub type StaticMachineApplicationHandle = Handle<StaticMachineApplication>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineApplication {
    /// The generic source declaration, never its private executable instance.
    pub registration_operation: SymbolHandle,
    /// The declaration whose type/lifetime scope authored these operands.
    /// Folding its type root does not transfer that scope to another caller.
    pub lexical_owner: SymbolHandle,
    /// Diagnostic text only; the symbol above owns declaration identity.
    pub target: Identifier,
    pub arguments: HandleSpan<StaticMachineArgument>,
    /// Exact inferred operands, including calls with no spelled static list.
    /// These are derivation data; their source bounds still owe admission.
    pub bindings: HandleSpan<StaticMachineBinding>,
}

/// One known substitution for a source declaration's binder. Missing bindings
/// are absent rows, not claims that an open tuple satisfies its requirements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineBinding {
    /// Always the original callee binder, even when a caller body is cloned.
    pub parameter: SymbolHandle,
    pub value: StaticMachineBindingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticMachineBindingValue {
    Type(TypeReferenceHandle),
    Const(TypeReferenceHandle),
    /// The carrier only: ordinary call operands retain and check the subject.
    RuntimeValue(TypeReferenceHandle),
    Machine(StaticMachineArgument),
    Conformance(StaticMachineArgument),
}

impl Default for StaticMachineBindingValue {
    fn default() -> Self {
        Self::Type(TypeReferenceHandle::invalid())
    }
}

impl StaticMachineBinding {
    pub fn remap_symbols(&mut self, symbols: &[(SymbolHandle, SymbolHandle)]) {
        // The binder belongs to registration_operation, not lexical_owner.
        // Remapping it would turn recursive calls into bindings for a private
        // caller's erased telescope. Actual operands do follow caller cloning.
        match &mut self.value {
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => argument.remap_symbols(symbols),
            StaticMachineBindingValue::Type(_)
            | StaticMachineBindingValue::Const(_)
            | StaticMachineBindingValue::RuntimeValue(_) => {}
        }
    }
}
