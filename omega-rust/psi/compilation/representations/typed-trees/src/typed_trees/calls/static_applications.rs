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
}
