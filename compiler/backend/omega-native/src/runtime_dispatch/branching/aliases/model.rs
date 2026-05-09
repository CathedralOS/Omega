use crate::control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::runtime_dispatch::branching) struct RuntimeBranchAlias {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: ProgramName,
    pub(super) expression: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::runtime_dispatch::branching) struct BranchParameterBinding {
    pub(in crate::runtime_dispatch::branching) parameter_symbol: SymbolHandle,
    pub(in crate::runtime_dispatch::branching) parameter_name: ProgramName,
    pub(in crate::runtime_dispatch::branching) expression: Expression,
}
