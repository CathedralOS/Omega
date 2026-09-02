//! Immutable inputs and complete output of one scalar family builder.

use crate::selection::shared::*;

pub(super) struct ScalarConstructionContext<'a> {
    pub(super) function: usize,
    pub(super) source: &'a SourceFunction,
    pub(super) constraints: &'a SelectedSelectionConstraints,
    pub(super) physical: &'a ValidatedPhysicalRegisterModel,
    pub(super) catalog: &'a ValidatedRegisterConstraintCatalog,
    pub(super) condition_inputs: Vec<ConditionInputContext>,
    pub(super) u64_type: ScalarType,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ConditionInputContext {
    pub(super) source_value: psi_core::ValueId,
    pub(super) parameter_index: usize,
    pub(super) definition_site: ValueDefinitionSite,
    pub(super) scalar_type: ScalarType,
    pub(super) class: RegisterClassId,
    pub(super) view: RegisterViewId,
}

pub(super) struct ConstructedScalarBody {
    pub(super) virtual_registers: Vec<VirtualRegister>,
    pub(super) blocks: Vec<SelectedBlock>,
}
