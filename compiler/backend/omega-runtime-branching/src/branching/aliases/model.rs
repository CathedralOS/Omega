use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeBranchAlias {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: ProgramName,
    pub(super) expression: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct BranchParameterBinding {
    pub(crate) parameter_symbol: SymbolHandle,
    pub(crate) parameter_name: ProgramName,
    pub(crate) expression: Expression,
}
